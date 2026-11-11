//! The only form-content sink. Pruning consumes current library snapshots on its own thread.
use super::FormError;
use crate::{
    persist::{PersistError, Persisted, Revision, SaveState},
    watch::{Publisher, Subscription, Watch},
};
use kudzu_core::{
    library::model::KudzuLibrary,
    template::{FormFieldSpec, FormMemory},
};
use std::{
    collections::{HashMap, HashSet},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, PoisonError,
    },
    thread::JoinHandle,
};
use uuid::Uuid;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct PruneState {
    pub library: Revision,
    pub error: Option<FormError>,
}
struct State {
    values: Publisher<FormMemory>,
    previous_ids: Option<HashSet<Uuid>>,
}
struct Shared {
    state: Mutex<State>,
    persisted: Persisted<FormMemory>,
    library: Watch<KudzuLibrary>,
    pruned: Publisher<PruneState>,
    stopped: AtomicBool,
}
pub(super) struct Memory {
    shared: Arc<Shared>,
    wake: mpsc::SyncSender<()>,
    _subscription: Subscription,
    worker: Mutex<Option<JoinHandle<()>>>,
}
impl Memory {
    pub(super) fn new(
        initial: FormMemory,
        persisted: Persisted<FormMemory>,
        library: Watch<KudzuLibrary>,
    ) -> Result<Self, FormError> {
        let initial = Arc::new(initial);
        persisted.submit(Revision(1), initial.clone())?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                values: Publisher::with_revision(initial, Revision(1)),
                previous_ids: None,
            }),
            persisted,
            library: library.clone(),
            pruned: Publisher::new(PruneState::default()),
            stopped: AtomicBool::new(false),
        });
        let (wake, receive) = mpsc::sync_channel(1);
        let enqueue = wake.clone();
        let subscription = library.subscribe(move || {
            let _ = enqueue.try_send(());
        });
        wake.try_send(()).expect("new prune queue");
        let state = shared.clone();
        let worker = std::thread::Builder::new()
            .name("kudzu-form-prune".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    while receive.recv().is_ok() {
                        if state.stopped.load(Ordering::Acquire) {
                            break;
                        }
                        let (_, error, library) = state.edit(|_| {});
                        state
                            .pruned
                            .replace(Arc::new(PruneState {
                                library,
                                error: error.map(FormError::from),
                            }))
                            .notification
                            .notify();
                    }
                }));
                if result.is_err() {
                    let mut next = *state.pruned.watch().get();
                    next.error = Some(FormError::WorkerStopped);
                    state.pruned.replace(Arc::new(next)).notification.notify();
                }
            })
            .map_err(crate::error::IoFailure::from)?;
        Ok(Self {
            shared,
            wake,
            _subscription: subscription,
            worker: Mutex::new(Some(worker)),
        })
    }
    pub(super) fn initial_values(
        &self,
        snippet: Uuid,
        fields: &[FormFieldSpec],
    ) -> HashMap<String, String> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values
            .watch()
            .get()
            .initial_values(snippet, fields)
    }
    pub(super) fn record(
        &self,
        snippet: Uuid,
        values: HashMap<String, String>,
    ) -> (Revision, Option<PersistError>) {
        let (revision, error, _) = self.shared.edit(|memory| memory.record(values, snippet));
        (revision, error)
    }
    pub(super) fn saved(&self) -> Watch<SaveState> {
        self.shared.persisted.saved()
    }
    pub(super) fn pruned(&self) -> Watch<PruneState> {
        self.shared.pruned.watch()
    }
    pub(super) fn stop(&self) {
        self.shared.stopped.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
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
impl Shared {
    fn edit(
        &self,
        edit: impl FnOnce(&mut FormMemory),
    ) -> (Revision, Option<PersistError>, Revision) {
        let (committed, error, library_revision) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let mut next = (*state.values.watch().get()).clone();
            edit(&mut next);
            // An empty recovered library is not evidence that the user deleted snippets.
            // The first observation is a baseline; only subsequent disappearances prune.
            let (revision, library) = self.library.snapshot();
            let current_ids: HashSet<_> = library.snippets.iter().map(|s| s.id).collect();
            if let Some(previous) = state.previous_ids.replace(current_ids.clone()) {
                for removed in previous.difference(&current_ids) {
                    next.values.remove(removed);
                }
            }
            while next.values.len() > FormMemory::CAPACITY {
                let victim = *next.values.keys().min().expect("over capacity");
                next.values.remove(&victim);
            }
            let committed = state.values.replace(Arc::new(next));
            let error = self
                .persisted
                .submit(committed.revision, committed.value.clone())
                .err();
            (committed, error, revision)
        };
        let revision = committed.revision;
        committed.notification.notify();
        (revision, error, library_revision)
    }
}
impl Drop for Memory {
    fn drop(&mut self) {
        self.stop();
    }
}
