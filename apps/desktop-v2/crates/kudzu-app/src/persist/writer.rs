//! Shared write queue, debounce, retry and bounded shutdown.
use super::{PersistError, Revision, SaveState, WriteTarget};
use crate::{
    ports::clock::Clock,
    watch::{Publisher, Subscription, Watch},
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{mpsc, Arc, Condvar, Mutex, PoisonError, Weak},
    thread::JoinHandle,
    time::Duration,
};

struct Pending<T> {
    latest: Revision,
    value: Option<(Revision, Arc<T>)>,
    due: Duration,
    first_pending_at: Option<Duration>,
}
struct Slot<T> {
    pending: Mutex<Pending<T>>,
    target: Box<dyn WriteTarget<T>>,
    saved: Publisher<SaveState>,
}
trait PendingWrite: Send + Sync {
    fn due(&self) -> Option<Duration>;
    fn attempt(&self, clock: &dyn Clock, retry: Duration) -> Result<(), PersistError>;
}
impl<T: Send + Sync> PendingWrite for Slot<T> {
    fn due(&self) -> Option<Duration> {
        let pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
        pending.value.as_ref().map(|_| pending.due)
    }
    fn attempt(&self, clock: &dyn Clock, retry: Duration) -> Result<(), PersistError> {
        let Some((revision, value)) = ({
            let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            pending.first_pending_at = None;
            pending.value.clone()
        }) else {
            return Ok(());
        };
        let result = catch_unwind(AssertUnwindSafe(|| self.target.write(&value)))
            .unwrap_or(Err(PersistError::Panicked));
        let now = clock.monotonic();
        let notification = {
            let mut pending = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            let current = self.saved.watch().get();
            let state = match result {
                Ok(()) => {
                    if pending.latest == revision {
                        pending.value = None;
                        pending.first_pending_at = None;
                    }
                    SaveState {
                        durable: revision.max(current.durable),
                        error: None,
                    }
                }
                Err(error) => {
                    if pending.latest == revision {
                        pending.due = now.saturating_add(retry);
                        pending.first_pending_at = Some(now);
                    }
                    SaveState {
                        durable: current.durable,
                        error: Some(error),
                    }
                }
            };
            self.saved.replace(Arc::new(state)).notification
        };
        notification.notify();
        result
    }
}
#[derive(Default)]
struct Queue {
    stopped: bool,
    slots: Vec<Arc<dyn PendingWrite>>,
}
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
    clock: Arc<dyn Clock>,
    debounce: Duration,
}

/// Owned by a persisted service. Submit under its transaction lock; no callbacks or I/O run here.
pub struct Persisted<T> {
    slot: Arc<Slot<T>>,
    writer: Weak<Shared>,
}
impl<T> Persisted<T> {
    pub fn saved(&self) -> Watch<SaveState> {
        self.slot.saved.watch()
    }
    /// Ignore stale/out-of-order revisions. A snapshot remains retained until a successful save.
    pub fn submit(&self, revision: Revision, value: Arc<T>) -> Result<(), PersistError> {
        let writer = self.writer.upgrade().ok_or(PersistError::Stopped)?;
        let queue = writer.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if queue.stopped {
            return Err(PersistError::Stopped);
        }
        let mut pending = self
            .slot
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if revision > pending.latest {
            let now = writer.clock.monotonic();
            let first = *pending.first_pending_at.get_or_insert(now);
            pending.latest = revision;
            pending.value = Some((revision, value));
            pending.due = now
                .saturating_add(writer.debounce)
                .min(first.saturating_add(Duration::from_secs(5)));
            writer.changed.notify_one();
        }
        Ok(())
    }
}
struct Worker {
    join: Option<JoinHandle<()>>,
    completed: mpsc::Receiver<Result<(), PersistError>>,
    result: Result<(), PersistError>,
}

/// Composition creates one writer for all domain files and owns its shutdown.
/// Registered targets live until shutdown even when their service handle is dropped.
pub struct Writer {
    shared: Arc<Shared>,
    worker: Mutex<Worker>,
    _clock_wake: Subscription,
}
impl Writer {
    pub fn new(
        clock: Arc<dyn Clock>,
        debounce: Duration,
        retry: Duration,
    ) -> Result<Self, PersistError> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            changed: Condvar::new(),
            clock: clock.clone(),
            debounce,
        });
        let wake = Arc::downgrade(&shared);
        let subscription = clock.subscribe(Arc::new(move || {
            if let Some(shared) = wake.upgrade() {
                let _queue = shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
                shared.changed.notify_one();
            }
        }));
        let thread_state = shared.clone();
        let (done, completed) = mpsc::channel();
        let join = std::thread::Builder::new()
            .name("kudzu-persist".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    writer_loop(thread_state.clone(), retry.max(Duration::from_millis(1)))
                }))
                .unwrap_or(Err(PersistError::WorkerStopped));
                {
                    let mut queue = thread_state
                        .queue
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    queue.stopped = true;
                    thread_state.changed.notify_all();
                }
                let _ = done.send(result);
            })?;
        Ok(Self {
            shared,
            worker: Mutex::new(Worker {
                join: Some(join),
                completed,
                result: Ok(()),
            }),
            _clock_wake: subscription,
        })
    }
    /// A file has one registration and one owning service. Use its loaded snapshot's revision.
    pub fn register<T: Send + Sync + 'static>(
        &self,
        target: impl WriteTarget<T> + 'static,
        durable: Revision,
    ) -> Result<Persisted<T>, PersistError> {
        let mut queue = self
            .shared
            .queue
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if queue.stopped {
            return Err(PersistError::Stopped);
        }
        let slot = Arc::new(Slot {
            pending: Mutex::new(Pending {
                latest: durable,
                value: None,
                due: Duration::ZERO,
                first_pending_at: None,
            }),
            target: Box::new(target),
            saved: Publisher::new(SaveState {
                durable,
                error: None,
            }),
        });
        queue.slots.push(slot.clone());
        Ok(Persisted {
            slot,
            writer: Arc::downgrade(&self.shared),
        })
    }
    /// Stop admissions, bypass debounce, attempt every pending file, and wait up to five seconds.
    /// S40 must stop producers first and handle an error before treating shutdown as durable.
    /// No target may wait on the UI. On Timeout the worker remains alive and retains its data;
    /// S40 must report the failure, not claim a durable quit. A process exit can lose that data.
    /// Repeated calls return the same result, including Timeout, without another wait.
    pub fn shutdown(&self) -> Result<(), PersistError> {
        self.shutdown_with_timeout(Duration::from_secs(5))
    }
    pub fn shutdown_with_timeout(&self, timeout: Duration) -> Result<(), PersistError> {
        let mut worker = self.worker.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(join) = worker.join.take() {
            {
                let mut queue = self
                    .shared
                    .queue
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                queue.stopped = true;
                self.shared.changed.notify_one();
            }
            worker.result = match worker.completed.recv_timeout(timeout) {
                Ok(result) => {
                    let _ = join.join();
                    result
                }
                Err(mpsc::RecvTimeoutError::Timeout) => Err(PersistError::Timeout),
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(PersistError::WorkerStopped),
            };
        }
        worker.result
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
fn writer_loop(shared: Arc<Shared>, retry: Duration) -> Result<(), PersistError> {
    let mut queue = shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if queue.stopped {
            let slots = queue.slots.clone();
            drop(queue);
            let mut result = Ok(());
            for slot in slots {
                if let Err(error) = slot.attempt(shared.clock.as_ref(), retry) {
                    result = Err(error);
                }
            }
            return result;
        }
        let next = queue
            .slots
            .iter()
            .filter_map(|slot| slot.due().map(|due| (due, slot.clone())))
            .min_by_key(|(due, _)| *due);
        let Some((due, slot)) = next else {
            queue = shared
                .changed
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
            continue;
        };
        let now = shared.clock.monotonic();
        if due > now {
            queue = shared
                .changed
                .wait_timeout(queue, due - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
            continue;
        }
        drop(queue);
        let _ = slot.attempt(shared.clock.as_ref(), retry);
        queue = shared.queue.lock().unwrap_or_else(PoisonError::into_inner);
    }
}
