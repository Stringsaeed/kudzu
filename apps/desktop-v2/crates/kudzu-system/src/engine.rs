//! `ExpansionEngine`, `FormChannel` and `KudzuWindows` over `kudzu_engine::Engine`.
//!
//! The native engine takes its host at construction, but the port's `start(events)` comes
//! later, so the engine reports to a [`ForwardingHost`] whose sink `start` installs and `stop`
//! removes. The host belongs to one [`EngineInstance`] from the composition's shared
//! [`EngineInstances`]: it qualifies native form tokens with it, and goes silent once a newer
//! engine instance exists. Commands check the same instance before a token reaches the engine.
//!
//! Threading: every method returns without waiting on the target app. `reload` builds the
//! trigger index on the calling thread (keep it off the UI thread); `stop` restores a pending
//! paste's clipboard and so can wait briefly for an in-flight paste. Sink callbacks arrive on
//! engine threads, or on the caller's thread for `start`, `stop`, `set_paused` and
//! `cancel_form`, with no adapter or engine lock held. Call `start` and `stop` from one thread
//! at a time (the composition root).
mod host;
mod native;
#[cfg(test)]
mod tests;

use host::ForwardingHost;
use kudzu_app::{
    error::PortError,
    ports::{
        engine::AdmissionError, EngineEvents, EngineInstance, EngineInstances, EngineLibrary,
        ExpansionEngine, FormChannel, FormSubmission, FormToken, InputEvent, KudzuWindow,
        KudzuWindows, PanelFocus,
    },
};
use kudzu_core::system::{AppInfo, EngineStatus};
use native::Native;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, PoisonError, Weak,
    },
};
use uuid::Uuid;

/// The macOS expansion engine behind the application's engine ports.
pub struct EngineAdapter {
    native: Arc<dyn Native>,
    host: Arc<ForwardingHost>,
    instance: EngineInstance,
    instances: Arc<EngineInstances>,
    running: AtomicBool,
}

/// Each composition's newest native engine. The native engine owns a session event tap, so
/// only the newest adapter on an `EngineInstances` may keep one running.
type Live = Vec<(Weak<EngineInstances>, Weak<dyn Native>)>;
static LIVE: Mutex<Live> = Mutex::new(Vec::new());

/// Stops the native engines of earlier adapters on `instances`, which retiring them alone
/// would leave matching and inserting. No lock is held while they stop.
fn stop_previous(instances: &Arc<EngineInstances>) {
    let previous: Vec<Arc<dyn Native>> = {
        let mut live = LIVE.lock().unwrap_or_else(PoisonError::into_inner);
        live.retain(|(owner, native)| owner.strong_count() > 0 && native.strong_count() > 0);
        let (same, rest): (Live, Live) = live.drain(..).partition(|(owner, _)| {
            owner
                .upgrade()
                .is_some_and(|owner| Arc::ptr_eq(&owner, instances))
        });
        *live = rest;
        same.into_iter()
            .filter_map(|(_, native)| native.upgrade())
            .collect()
    };
    for native in previous {
        native.stop();
    }
}

impl EngineAdapter {
    /// A stopped engine on the real system, with the next instance from `instances`. Creating
    /// it retires the previous adapter on the same `instances` (its late callbacks are dropped)
    /// and stops that adapter's native engine, so only one event tap runs. S40 should still
    /// `stop` the previous adapter first, which also detaches its sink.
    pub fn new(instances: Arc<EngineInstances>) -> Self {
        Self::with_native(instances, |host| {
            Arc::new(native::System(kudzu_engine::Engine::new(host)))
        })
    }

    fn with_native(
        instances: Arc<EngineInstances>,
        build: impl FnOnce(Arc<dyn kudzu_engine::EngineHost>) -> Arc<dyn Native>,
    ) -> Self {
        let instance = instances.next_engine();
        stop_previous(&instances);
        let host = Arc::new(ForwardingHost::new(instance, instances.clone()));
        let native = build(host.clone());
        LIVE.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((Arc::downgrade(&instances), Arc::downgrade(&native)));
        Self {
            native,
            host,
            instance,
            instances,
            running: AtomicBool::new(false),
        }
    }

    fn admitted(&self) -> Result<(), AdmissionError> {
        let live = self.running.load(Ordering::SeqCst) && self.instances.is_current(self.instance);
        live.then_some(()).ok_or(AdmissionError::NoEngine)
    }
}

impl ExpansionEngine for EngineAdapter {
    /// Fails with `Unavailable` once a newer engine instance exists. Nothing is admitted until
    /// the native engine has started (its listener attaches once permissions allow).
    fn start(&self, events: Arc<dyn EngineEvents>) -> Result<(), PortError> {
        if !self.instances.is_current(self.instance) {
            return Err(PortError::Unavailable);
        }
        self.host.attach(events);
        self.native.start();
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Refuses new admissions, stops the native engine (its last status still reaches the
    /// sink), then detaches the sink.
    fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.native.stop();
        self.host.detach();
    }

    fn reload(&self, library: EngineLibrary) {
        self.native.reload(engine_library(library));
    }

    fn status(&self) -> EngineStatus {
        self.native.status()
    }

    fn set_paused(&self, paused: bool) {
        self.native.set_paused(paused);
    }

    fn set_suppressed(&self, suppressed: bool) {
        self.native.set_suppressed(suppressed);
    }

    /// Never `Busy`: the native engine queues insertions on its worker.
    fn insert(
        &self,
        snippet: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<(), AdmissionError> {
        self.admitted()?;
        self.native.insert(snippet, target, selection);
        Ok(())
    }
}

impl FormChannel for EngineAdapter {
    /// `StaleToken` for another instance's token, and for one the native engine no longer has
    /// as its pending form (already submitted, cancelled or replaced), which would otherwise
    /// produce no outcome: the forms service cancels on it.
    fn submit_form(
        &self,
        token: FormToken,
        values: HashMap<String, String>,
        submission: FormSubmission,
    ) -> Result<(), AdmissionError> {
        self.admitted()?;
        let native = self
            .instance
            .native_token(token)
            .ok_or(AdmissionError::StaleToken)?;
        let accepted = self.native.submit_form(
            native,
            values,
            kudzu_engine::Submission {
                event: submission.event.map(input_event),
                focus_seq: submission.focus_seq,
            },
        );
        accepted.then_some(()).ok_or(AdmissionError::StaleToken)
    }

    fn cancel_form(&self, token: FormToken) {
        if let Some(native) = self.instance.native_token(token) {
            self.native.cancel_form(native);
        }
    }

    fn panel_focused(&self, token: FormToken, focus: PanelFocus) {
        if let Some(native) = self.instance.native_token(token) {
            self.native.form_panel_focused(
                native,
                kudzu_engine::PanelFocus {
                    key: focus.key,
                    seq: focus.seq,
                    after: focus.after.map(input_event),
                },
            );
        }
    }
}

impl KudzuWindows for EngineAdapter {
    /// Holds for this engine object across stop and start; a new adapter starts empty.
    fn set_kudzu_windows(&self, windows: &[KudzuWindow]) {
        let windows: Vec<_> = windows
            .iter()
            .map(|w| kudzu_engine::KudzuWindow {
                number: w.number,
                passes_clicks_through: w.passes_clicks_through,
            })
            .collect();
        self.native.set_kudzu_windows(&windows);
    }
}

fn input_event(event: InputEvent) -> kudzu_engine::InputEvent {
    kudzu_engine::InputEvent {
        stamp: kudzu_engine::EventStamp(event.stamp.cg_event_timestamp()),
        key_code: event.key_code,
    }
}

/// The host always passes a locale: an empty one becomes the system locale here, so the
/// engine never reads it from the OS.
fn engine_library(library: EngineLibrary) -> kudzu_engine::EngineLibrary {
    let locale = if library.locale.trim().is_empty() {
        crate::locale::current()
    } else {
        library.locale
    };
    kudzu_engine::EngineLibrary {
        snippets: library.snippets,
        groups: library.groups,
        config: library.config,
        locale,
    }
}
