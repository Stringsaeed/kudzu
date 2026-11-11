//! One state-based reactor. Timers only enqueue; matching runs on its own joined worker.
use super::{KudzuLibrary, LibraryError};
use crate::{
    error::IoFailure,
    schedule::{Scheduler, Timer},
    watch::{Publisher, Revision, Subscription, Watch},
};
use kudzu_core::library::conflicts::{find_conflicts, TriggerConflict};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Condvar, Mutex, PoisonError},
    thread::JoinHandle,
    time::Duration,
};

#[derive(Clone, PartialEq, Eq, Default)]
pub struct ConflictSnapshot {
    /// Zero until the initial computation. Otherwise the exact library snapshot checked.
    pub revision: Revision,
    pub items: Vec<TriggerConflict>,
    pub error: Option<LibraryError>,
}

#[derive(Default)]
struct Pending {
    requested: Option<Revision>,
    ready: bool,
    stopped: bool,
    timer: Option<Timer>,
}
struct Shared {
    state: Arc<Mutex<Publisher<KudzuLibrary>>>,
    library: Watch<KudzuLibrary>,
    output: Publisher<ConflictSnapshot>,
    pending: Mutex<Pending>,
    changed: Condvar,
    scheduler: Arc<Scheduler>,
}

pub(super) struct ConflictWorker {
    shared: Arc<Shared>,
    _subscription: Subscription,
    worker: Option<JoinHandle<()>>,
}
impl ConflictWorker {
    pub(super) fn new(
        state: Arc<Mutex<Publisher<KudzuLibrary>>>,
        scheduler: Arc<Scheduler>,
    ) -> Result<Self, LibraryError> {
        let library = state.lock().unwrap_or_else(PoisonError::into_inner).watch();
        let shared = Arc::new(Shared {
            state,
            library: library.clone(),
            output: Publisher::new(ConflictSnapshot::default()),
            pending: Mutex::new(Pending::default()),
            changed: Condvar::new(),
            scheduler,
        });
        let weak = Arc::downgrade(&shared);
        let subscription = library.subscribe(move || {
            if let Some(shared) = weak.upgrade() {
                if let Err(error) = arm(&shared) {
                    report(&shared, error);
                }
            }
        });
        arm(&shared)?;
        let thread_state = shared.clone();
        let worker = std::thread::Builder::new()
            .name("kudzu-conflicts".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| run(&thread_state)));
                let timer = {
                    let mut pending = thread_state
                        .pending
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    pending.stopped = true;
                    pending.timer.take()
                };
                drop(timer);
                if result.is_err() {
                    report(&thread_state, LibraryError::WorkerStopped);
                }
            })
            .map_err(IoFailure::from)?;
        Ok(Self {
            shared,
            _subscription: subscription,
            worker: Some(worker),
        })
    }
    pub(super) fn watch(&self) -> Watch<ConflictSnapshot> {
        self.shared.output.watch()
    }
}
impl Drop for ConflictWorker {
    fn drop(&mut self) {
        let timer = {
            let mut pending = self
                .shared
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            pending.stopped = true;
            self.shared.changed.notify_one();
            pending.timer.take()
        };
        drop(timer);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn arm(shared: &Arc<Shared>) -> Result<(), LibraryError> {
    let revision = shared.library.revision();
    let mut pending = shared
        .pending
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if pending.stopped || pending.requested.is_some_and(|old| old >= revision) {
        return Ok(());
    }
    pending.requested = Some(revision);
    pending.ready = false;
    let weak = Arc::downgrade(shared);
    let timer = shared
        .scheduler
        .after(Duration::from_millis(250), move || {
            if let Some(shared) = weak.upgrade() {
                let mut pending = shared
                    .pending
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if !pending.stopped && pending.requested == Some(revision) {
                    pending.ready = true;
                    shared.changed.notify_one();
                }
            }
        })?;
    pending.timer = Some(timer);
    Ok(())
}

fn run(shared: &Shared) {
    loop {
        let requested = {
            let mut pending = shared
                .pending
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            while !pending.ready && !pending.stopped {
                pending = shared
                    .changed
                    .wait(pending)
                    .unwrap_or_else(PoisonError::into_inner);
            }
            if pending.stopped {
                return;
            }
            pending.ready = false;
            pending.requested
        };
        let (revision, library) = shared.library.snapshot();
        if requested != Some(revision) {
            continue;
        }
        let items = find_conflicts(&library.snippets, &library.groups);
        let notification = {
            let state = shared.state.lock().unwrap_or_else(PoisonError::into_inner);
            if state.watch().revision() != revision {
                continue;
            }
            shared
                .output
                .replace(Arc::new(ConflictSnapshot {
                    revision,
                    items,
                    error: None,
                }))
                .notification
        };
        notification.notify();
    }
}

fn report(shared: &Shared, error: LibraryError) {
    let notification = {
        let _state = shared.state.lock().unwrap_or_else(PoisonError::into_inner);
        let current = shared.output.watch().get();
        shared
            .output
            .replace(Arc::new(ConflictSnapshot {
                error: Some(error),
                ..(*current).clone()
            }))
            .notification
    };
    notification.notify();
}
