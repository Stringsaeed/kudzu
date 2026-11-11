//! The adapter passes the engine contract twice: forwarding through a recording native fake,
//! and on the real `kudzu_engine::Engine` for what needs no permission. The real engine is
//! never started here, so no tap is installed and no event is posted.
use super::*;
use kudzu_app::fixtures::{
    contract::engine::{self as contract, EngineHarness, NativeLog, NativeSide},
    engine::FakeEngineEvents,
};
use kudzu_app::ports::{Expansion, TriggerLost};
use kudzu_core::activity::ActivityEvent;
use kudzu_engine::EngineHost;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
struct Recorded {
    log: NativeLog,
    paused: bool,
    starts: usize,
    /// Native form tokens requested and not yet submitted or cancelled, as the engine keeps.
    pending: std::collections::HashSet<u64>,
}

/// Records commands in port terms and reports through the host, as the native engine would.
struct FakeNative {
    host: Arc<dyn EngineHost>,
    state: Mutex<Recorded>,
}

impl FakeNative {
    fn state(&self) -> std::sync::MutexGuard<'_, Recorded> {
        self.state.lock().unwrap()
    }
}

fn port_event(event: kudzu_engine::InputEvent) -> InputEvent {
    InputEvent {
        stamp: kudzu_app::ports::EventStamp::from_cg_event_timestamp(event.stamp.0),
        key_code: event.key_code,
    }
}

impl Native for FakeNative {
    fn start(&self) {
        self.state().starts += 1;
    }
    fn stop(&self) {
        let status = self.status();
        self.host.status_changed(status);
    }
    fn reload(&self, library: kudzu_engine::EngineLibrary) {
        self.state()
            .log
            .reloads
            .push((library.snippets.len(), library.locale));
    }
    fn status(&self) -> EngineStatus {
        let mut status = kudzu_app::fixtures::engine::EngineState::default().status;
        status.paused = self.state().paused;
        status
    }
    fn set_paused(&self, paused: bool) {
        self.state().paused = paused;
    }
    fn set_suppressed(&self, _: bool) {}
    fn insert(&self, snippet: Uuid, target: Option<AppInfo>, selection: Option<String>) {
        self.state().log.inserts.push((snippet, target, selection));
    }
    fn submit_form(
        &self,
        token: u64,
        values: HashMap<String, String>,
        s: kudzu_engine::Submission,
    ) -> bool {
        let submission = FormSubmission {
            event: s.event.map(port_event),
            focus_seq: s.focus_seq,
        };
        let mut state = self.state();
        if !state.pending.remove(&token) {
            return false;
        }
        state.log.submissions.push((token, values, submission));
        true
    }
    fn cancel_form(&self, token: u64) {
        let mut state = self.state();
        state.pending.remove(&token);
        state.log.cancels.push(token);
    }
    fn form_panel_focused(&self, token: u64, focus: kudzu_engine::PanelFocus) {
        let focus = PanelFocus {
            key: focus.key,
            seq: focus.seq,
            after: focus.after.map(port_event),
        };
        self.state().log.focus.push((token, focus));
    }
    fn set_kudzu_windows(&self, windows: &[kudzu_engine::KudzuWindow]) {
        let windows = windows
            .iter()
            .map(|w| KudzuWindow {
                number: w.number,
                passes_clicks_through: w.passes_clicks_through,
            })
            .collect();
        self.state().log.windows.push(windows);
    }
}

struct Forwarding {
    adapter: EngineAdapter,
    native: Arc<FakeNative>,
    instances: Arc<EngineInstances>,
}

impl Forwarding {
    fn new(instances: Arc<EngineInstances>) -> Self {
        let slot = OnceLock::new();
        let adapter = EngineAdapter::with_native(instances.clone(), |host| {
            let native = Arc::new(FakeNative {
                host,
                state: Mutex::default(),
            });
            slot.set(native.clone()).ok();
            native
        });
        Self {
            adapter,
            native: slot.into_inner().unwrap(),
            instances,
        }
    }
    fn host(&self) -> &dyn EngineHost {
        self.native.host.as_ref()
    }
}

impl NativeSide for Forwarding {
    fn request_form(&self, native: u64, snippet: Uuid, target: Option<AppInfo>) {
        self.native.state().pending.insert(native);
        self.host().form_requested(kudzu_engine::FormRequest {
            token: native,
            snippet_id: snippet,
            snippet_name: "Form".into(),
            fields: vec![],
            caret: None,
            target_app: target,
            selection: None,
        });
    }
    fn report_status(&self, status: EngineStatus) {
        self.host().status_changed(status);
    }
    fn report_expansion(&self, expansion: Expansion) {
        self.host().expanded(kudzu_engine::ExpansionEvent {
            snippet_id: expansion.snippet_id,
            chars_saved: expansion.chars_saved,
        });
    }
    fn report_activity(&self, event: ActivityEvent) {
        self.host().activity(event);
    }
    fn report_trigger_lost(&self, lost: TriggerLost) {
        self.host().trigger_lost(kudzu_engine::TriggerLost {
            snippet_id: lost.snippet_id,
            trigger: lost.trigger,
        });
    }
    fn ask_history(&self, index: usize, current: Option<&str>) -> Option<String> {
        self.host().clipboard_history(index, current)
    }
    fn log(&self) -> NativeLog {
        self.native.state().log.clone()
    }
}

/// A token from a later instance than `instance`, which the allocator has not reached yet.
fn later_token(instance: EngineInstance, native: u64) -> FormToken {
    let probe = EngineInstances::default();
    loop {
        let candidate = probe.next_engine();
        if candidate > instance {
            return candidate.form_token(native);
        }
    }
}

impl EngineHarness for Forwarding {
    fn engine(&self) -> &dyn ExpansionEngine {
        &self.adapter
    }
    fn forms(&self) -> &dyn FormChannel {
        &self.adapter
    }
    fn windows(&self) -> &dyn KudzuWindows {
        &self.adapter
    }
    fn native(&self) -> Option<&dyn NativeSide> {
        Some(self)
    }
    fn foreign_token(&self, native: u64) -> FormToken {
        later_token(self.adapter.instance, native)
    }
    fn own_token(&self, native: u64) -> FormToken {
        self.adapter.instance.form_token(native)
    }
}

#[test]
fn forwarding_adapter_passes_the_engine_contract() {
    let instances = Arc::new(EngineInstances::default());
    contract::without_native(&Forwarding::new(instances.clone()));
    contract::with_native(&Forwarding::new(instances));
}

struct Real(EngineAdapter);
impl EngineHarness for Real {
    fn engine(&self) -> &dyn ExpansionEngine {
        &self.0
    }
    fn forms(&self) -> &dyn FormChannel {
        &self.0
    }
    fn windows(&self) -> &dyn KudzuWindows {
        &self.0
    }
    fn native(&self) -> Option<&dyn NativeSide> {
        None
    }
    fn foreign_token(&self, native: u64) -> FormToken {
        later_token(self.0.instance, native)
    }
    fn own_token(&self, native: u64) -> FormToken {
        self.0.instance.form_token(native)
    }
}

#[test]
fn real_engine_adapter_passes_the_contract_cases_that_need_no_permission() {
    let real = Real(EngineAdapter::new(Arc::new(EngineInstances::default())));
    contract::without_native(&real);
    drop(real);
}

#[test]
fn a_newer_instance_retires_the_older_adapter_and_fences_its_callbacks() {
    let instances = Arc::new(EngineInstances::default());
    let old = Forwarding::new(instances.clone());
    let events = Arc::new(FakeEngineEvents::default());
    old.adapter.start(events.clone()).unwrap();
    old.request_form(1, Uuid::nil(), None);
    let old_token = events.forms.lock().unwrap()[0].token;
    old.adapter.stop();

    let new = Forwarding::new(instances.clone());
    assert!(!instances.is_current(old.adapter.instance));
    assert_eq!(
        old.adapter.start(events.clone()),
        Err(PortError::Unavailable),
        "a retired adapter cannot come back"
    );
    new.adapter.start(events.clone()).unwrap();
    // The old engine's late report goes nowhere; its token never reaches the new engine.
    old.request_form(1, Uuid::nil(), None);
    assert_eq!(events.forms.lock().unwrap().len(), 1);
    assert_eq!(
        new.adapter
            .submit_form(old_token, HashMap::new(), FormSubmission::default()),
        Err(AdmissionError::StaleToken)
    );
    new.adapter.cancel_form(old_token);
    new.adapter.panel_focused(
        old_token,
        PanelFocus {
            key: true,
            seq: 1,
            after: None,
        },
    );
    assert!(new.log().cancels.is_empty() && new.log().focus.is_empty());
    new.request_form(1, Uuid::nil(), None);
    let fresh = events.forms.lock().unwrap()[1].token;
    assert_ne!(fresh, old_token, "same native number, different namespace");
    assert_eq!(
        new.adapter
            .submit_form(fresh, HashMap::new(), FormSubmission::default()),
        Ok(())
    );
    new.adapter.stop();
}

#[test]
fn stop_delivers_the_final_status_before_detaching() {
    let harness = Forwarding::new(Arc::new(EngineInstances::default()));
    let events = Arc::new(FakeEngineEvents::default());
    harness.adapter.start(events.clone()).unwrap();
    harness.adapter.stop();
    assert_eq!(events.statuses.lock().unwrap().len(), 1);
    harness.report_status(harness.adapter.status());
    assert_eq!(events.statuses.lock().unwrap().len(), 1);
    assert_eq!(harness.native.state().starts, 1);
}

#[test]
fn an_empty_locale_becomes_the_system_locale_and_a_given_one_is_kept() {
    let harness = Forwarding::new(Arc::new(EngineInstances::default()));
    harness.adapter.reload(EngineLibrary::default());
    harness.adapter.reload(EngineLibrary {
        locale: "fr_FR".into(),
        ..Default::default()
    });
    let reloads = harness.log().reloads;
    assert_eq!(reloads[0].1, crate::locale::current());
    assert!(!reloads[0].1.is_empty());
    assert_eq!(reloads[1].1, "fr_FR");
    assert!(harness.instances.is_current(harness.adapter.instance));
}

#[test]
fn input_events_keep_their_stamp_and_key_code() {
    let event = InputEvent {
        stamp: kudzu_app::ports::EventStamp::from_cg_event_timestamp(u64::MAX - 3),
        key_code: Some(36),
    };
    let native = input_event(event);
    assert_eq!(native.stamp, kudzu_engine::EventStamp(u64::MAX - 3));
    assert_eq!(native.key_code, Some(36));
    assert_eq!(port_event(native), event);
}
