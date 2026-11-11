//! One named thread per operation; dropping a result owner never joins a worker.
//! S40: Tasks are untracked. Stop task admission and fence/join any service-owned producers
//! before draining Writer. Dropping a Task is not cancellation or a shutdown barrier.

use crate::error::IoFailure;
use crate::watch::{Publisher, Watch};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TaskError {
    #[error("The operation could not start.")]
    Start(#[from] IoFailure),
    #[error("The operation stopped unexpectedly.")]
    Panicked,
}

struct Slot<T> {
    result: Mutex<Option<Result<T, TaskError>>>,
    ready: Publisher<bool>,
}

/// A nonblocking result slot. Service work must commit before returning its result.
/// Dropping abandons delivery, not the operation; use a Weak service in cancellable work.
pub struct Task<T>(Arc<Slot<T>>);
impl<T: Send + 'static> Task<T> {
    pub fn spawn(
        name: &str,
        operation: impl FnOnce() -> T + Send + 'static,
    ) -> Result<Self, TaskError> {
        let slot = Arc::new(Slot {
            result: Mutex::new(None),
            ready: Publisher::new(false),
        });
        let owner = Arc::downgrade(&slot);
        std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
                    .map_err(|_| TaskError::Panicked);
                if let Some(slot) = owner.upgrade() {
                    *slot.result.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
                    slot.ready.replace(Arc::new(true)).notification.notify();
                }
            })
            .map_err(IoFailure::from)?;
        Ok(Self(slot))
    }
    /// A task that is already complete. Used when the worker thread cannot start.
    pub fn done(value: T) -> Self {
        Self(Arc::new(Slot {
            result: Mutex::new(Some(Ok(value))),
            ready: Publisher::new(true),
        }))
    }
    pub fn ready(&self) -> Watch<bool> {
        self.0.ready.watch()
    }
    pub fn take(&self) -> Option<Result<T, TaskError>> {
        self.0
            .result
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn operation_panic_is_delivered_as_a_safe_error() {
        let (release, wait) = mpsc::channel();
        let task = Task::spawn("fixture-panic", move || {
            wait.recv().unwrap();
            panic!("operation failed");
        })
        .unwrap();
        let (wake, woke) = mpsc::channel();
        let _sub = task.ready().subscribe(move || {
            wake.send(()).unwrap();
        });
        release.send(()).unwrap();
        woke.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(task.take(), Some(Err(TaskError::Panicked)));
    }

    #[test]
    fn completion_is_pull_based_and_taken_once() {
        let (release, wait) = mpsc::channel();
        let task = Task::spawn("fixture-import", move || {
            wait.recv().unwrap();
            42
        })
        .unwrap();
        let (wake, woke) = mpsc::channel();
        let _subscription = task.ready().subscribe(move || {
            wake.send(()).unwrap();
        });
        assert!(task.take().is_none());
        release.send(()).unwrap();
        woke.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(task.take(), Some(Ok(42)));
        assert!(task.take().is_none());
    }

    #[test]
    fn dropped_owner_does_not_wait_or_receive_completion() {
        let (release, wait) = mpsc::channel();
        let (done, dropped) = mpsc::channel();
        struct ResultDrop(mpsc::Sender<()>);
        impl Drop for ResultDrop {
            fn drop(&mut self) {
                self.0.send(()).unwrap();
            }
        }
        let task = Task::spawn("fixture-owner", move || {
            wait.recv().unwrap();
            ResultDrop(done)
        })
        .unwrap();
        let ready = task.ready();
        drop(task);
        release.send(()).unwrap();
        dropped.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(!*ready.get());
    }
}
