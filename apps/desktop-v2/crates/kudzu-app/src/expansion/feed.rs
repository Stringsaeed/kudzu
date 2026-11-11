//! State-based engine feed. Wakers only set a flag; reloads run on this worker.
use super::ExpansionError;
use crate::{
    ports::{EngineLibrary, ExpansionEngine},
    settings::Settings,
    watch::{Publisher, Revision, Subscription, Watch},
};
use kudzu_core::{library::model::KudzuLibrary, system::EngineStatus};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Condvar, Mutex, PoisonError},
    thread::JoinHandle,
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct FeedState {
    pub generation: u64,
    pub library: Revision,
    pub settings: Revision,
    pub error: Option<ExpansionError>,
}
struct Pending {
    generation: u64,
    dirty: bool,
    stopped: bool,
}
struct Shared {
    pending: Mutex<Pending>,
    changed: Condvar,
    output: Publisher<FeedState>,
}
pub(super) struct Feed {
    shared: Arc<Shared>,
    _subscriptions: [Subscription; 2],
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl Feed {
    pub(super) fn new(
        library: Watch<KudzuLibrary>,
        settings: Watch<Settings>,
        engine: Arc<dyn ExpansionEngine>,
        status: Arc<Mutex<Publisher<EngineStatus>>>,
        locale: String,
    ) -> Result<Self, ExpansionError> {
        let shared = Arc::new(Shared {
            pending: Mutex::new(Pending {
                generation: 0,
                dirty: true,
                stopped: false,
            }),
            changed: Condvar::new(),
            output: Publisher::new(FeedState::default()),
        });
        let wake = || {
            let weak = Arc::downgrade(&shared);
            move || {
                if let Some(shared) = weak.upgrade() {
                    shared
                        .pending
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .dirty = true;
                    shared.changed.notify_one();
                }
            }
        };
        let subscriptions = [library.subscribe(wake()), settings.subscribe(wake())];
        let state = shared.clone();
        let worker = std::thread::Builder::new()
            .name("kudzu-engine-feed".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut previous = None;
                    let mut paused = None;
                    let mut applied = 0;
                    loop {
                        let generation = {
                            let mut pending =
                                state.pending.lock().unwrap_or_else(PoisonError::into_inner);
                            while !pending.dirty && !pending.stopped {
                                pending = state
                                    .changed
                                    .wait(pending)
                                    .unwrap_or_else(PoisonError::into_inner);
                            }
                            if pending.stopped {
                                return;
                            }
                            pending.dirty = false;
                            pending.generation
                        };
                        if generation != applied {
                            previous = None;
                            paused = None;
                            applied = generation;
                        }
                        let (library_revision, library) = library.snapshot();
                        let (settings_revision, settings) = settings.snapshot();
                        let next = EngineLibrary {
                            snippets: library.snippets.clone(),
                            groups: library.groups.clone(),
                            config: settings.matching_config(),
                            locale: locale.clone(),
                        };
                        if previous.as_ref() != Some(&next) {
                            engine.reload(next.clone());
                            previous = Some(next);
                        }
                        if paused != Some(settings.paused) {
                            engine.set_paused(settings.paused);
                            paused = Some(settings.paused);
                            let current = engine.status();
                            let committed = {
                                status
                                    .lock()
                                    .unwrap_or_else(PoisonError::into_inner)
                                    .replace(Arc::new(current))
                            };
                            committed.notification.notify();
                        }
                        state
                            .output
                            .replace(Arc::new(FeedState {
                                generation,
                                library: library_revision,
                                settings: settings_revision,
                                error: None,
                            }))
                            .notification
                            .notify();
                    }
                }));
                if result.is_err() {
                    let mut next = *state.output.watch().get();
                    next.error = Some(ExpansionError::WorkerStopped);
                    state.output.replace(Arc::new(next)).notification.notify();
                }
            })
            .map_err(crate::error::IoFailure::from)?;
        Ok(Self {
            shared,
            _subscriptions: subscriptions,
            worker: Mutex::new(Some(worker)),
        })
    }
    pub(super) fn watch(&self) -> Watch<FeedState> {
        self.shared.output.watch()
    }
    pub(super) fn resync(&self) -> u64 {
        let mut pending = self
            .shared
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        pending.generation = pending
            .generation
            .checked_add(1)
            .expect("feed generation exhausted");
        pending.dirty = true;
        self.shared.changed.notify_one();
        pending.generation
    }
    pub(super) fn stop(&self) {
        self.shared
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .stopped = true;
        self.shared.changed.notify_one();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = worker.join();
        }
    }
}
impl Drop for Feed {
    fn drop(&mut self) {
        self.stop();
    }
}
