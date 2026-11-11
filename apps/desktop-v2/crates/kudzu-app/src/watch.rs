//! Immutable snapshots; notifications are explicitly delivered after the service unlocks.
//!
//! Poison policy for services and infrastructure: recover plain-data locks with
//! `unwrap_or_else(PoisonError::into_inner)`. Mutate a working copy and commit only on success;
//! never expose partially changed data if a caller panics. Run external callbacks after unlock
//! and isolate each with `catch_unwind`. This policy does not repair external side effects.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Revision(pub u64);

struct Listener {
    active: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}

struct State<T> {
    value: Arc<T>,
    revision: Revision,
    listeners: Vec<Weak<Listener>>,
}

/// Read-only handle. Subscribe before reading; coalesced wakes always pull the latest state.
pub struct Watch<T>(Arc<Mutex<State<T>>>);

impl<T> Clone for Watch<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Watch<T> {
    pub fn get(&self) -> Arc<T> {
        self.snapshot().1
    }
    pub fn revision(&self) -> Revision {
        self.snapshot().0
    }
    /// Read the revision and its snapshot atomically, avoiding a revision/read race in `poll`.
    pub fn snapshot(&self) -> (Revision, Arc<T>) {
        let state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        (state.revision, state.value.clone())
    }
    pub fn subscribe(&self, wake: impl Fn() + Send + Sync + 'static) -> Subscription {
        let listener = Arc::new(Listener {
            active: AtomicBool::new(true),
            wake: Box::new(wake),
        });
        let mut state = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        state
            .listeners
            .retain(|listener| listener.strong_count() != 0);
        state.listeners.push(Arc::downgrade(&listener));
        Subscription(Some(listener))
    }
}

/// The service keeps this handle private; callers receive only `watch()`.
pub struct Publisher<T>(Watch<T>);

impl<T> Publisher<T> {
    pub fn new(value: T) -> Self {
        Self::with_revision(Arc::new(value), Revision(0))
    }
    /// Persisted services start at 1 (durable 0 until their initial write succeeds).
    pub fn with_revision(value: Arc<T>, revision: Revision) -> Self {
        Self(Watch(Arc::new(Mutex::new(State {
            value,
            revision,
            listeners: Vec::new(),
        }))))
    }
    pub fn watch(&self) -> Watch<T> {
        self.0.clone()
    }
}

impl<T: PartialEq> Publisher<T> {
    /// Commit while the owning service is locked. Call `notify()` only after releasing it.
    pub fn replace(&self, value: Arc<T>) -> Committed<T> {
        let mut state = self.0 .0.lock().unwrap_or_else(PoisonError::into_inner);
        if *state.value == *value {
            return Committed {
                revision: state.revision,
                value: state.value.clone(),
                changed: false,
                notification: Notification(Vec::new()),
            };
        }
        state.revision = Revision(
            state
                .revision
                .0
                .checked_add(1)
                .expect("snapshot revision exhausted"),
        );
        state.value = value.clone();
        let listeners = state.listeners.iter().filter_map(Weak::upgrade).collect();
        state
            .listeners
            .retain(|listener| listener.strong_count() != 0);
        Committed {
            revision: state.revision,
            value,
            changed: true,
            notification: Notification(listeners),
        }
    }
}

/// One snapshot is shared by the service, subscribers and persistence.
#[must_use = "submit this revision and value, release the service lock, then notify"]
pub struct Committed<T> {
    pub revision: Revision,
    pub value: Arc<T>,
    pub changed: bool,
    pub notification: Notification,
}

/// Drop unsubscribes, including notifications captured but not yet delivered.
/// Dropping never waits: a callback may begin just after drop returns if delivery already
/// passed its active check. Use Weak owners or generation checks when teardown matters.
#[derive(Default)]
pub struct Subscription(Option<Arc<Listener>>);
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(listener) = &self.0 {
            listener.active.store(false, Ordering::Release);
        }
    }
}

/// No payload and no implicit callback on drop. Wakers must only enqueue, flag, or wake.
#[must_use = "release the service lock, then call notify()"]
pub struct Notification(Vec<Arc<Listener>>);
impl Notification {
    pub fn notify(self) {
        for listener in self.0 {
            if listener.active.load(Ordering::Acquire) {
                let _ = catch_unwind(AssertUnwindSafe(|| (listener.wake)()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn concurrent_subscribe_then_get_never_misses_the_final_snapshot() {
        let publisher = Arc::new(Publisher::new(0));
        let producer = publisher.clone();
        let writer = std::thread::spawn(move || {
            for value in 1..=500 {
                producer.replace(Arc::new(value)).notification.notify();
                std::thread::yield_now();
            }
        });
        let readers: Vec<_> = (0..8)
            .map(|_| {
                let watch = publisher.watch();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        let (tx, rx) = mpsc::channel();
                        let _sub = watch.subscribe(move || {
                            let _ = tx.send(());
                        });
                        let mut seen = Revision(0);
                        loop {
                            let (revision, value) = watch.snapshot();
                            assert!(revision >= seen);
                            assert_eq!(revision.0, *value);
                            seen = revision;
                            if *value == 500 {
                                break;
                            }
                            rx.recv_timeout(Duration::from_secs(2)).unwrap();
                        }
                    }
                })
            })
            .collect();
        writer.join().unwrap();
        for reader in readers {
            reader.join().unwrap();
        }
    }

    #[test]
    fn commit_returns_the_published_arc_and_revision_without_copying() {
        let publisher = Publisher::with_revision(Arc::new(10), Revision(4));
        let value = Arc::new(11);
        let committed = publisher.replace(value.clone());
        assert!(committed.changed);
        assert_eq!(committed.revision, Revision(5));
        assert!(Arc::ptr_eq(&value, &committed.value));
        assert!(Arc::ptr_eq(&value, &publisher.watch().get()));
        committed.notification.notify();
        let same = publisher.replace(Arc::new(11));
        assert!(!same.changed);
        assert_eq!(same.revision, Revision(5));
        assert!(Arc::ptr_eq(&value, &same.value));
        same.notification.notify();
    }

    #[test]
    fn panicking_waker_does_not_skip_other_subscribers_or_escape_the_commit() {
        let publisher = Publisher::new(0);
        let _bad = publisher.watch().subscribe(|| panic!("waker failed"));
        let (tx, rx) = mpsc::channel();
        let _good = publisher.watch().subscribe(move || {
            tx.send(()).unwrap();
        });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            publisher.replace(Arc::new(1)).notification.notify()
        }));
        assert!(
            result.is_ok(),
            "a waker panic escaped notification delivery"
        );
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(*publisher.watch().get(), 1);
    }

    #[test]
    fn poisoned_plain_snapshot_lock_retains_the_last_committed_value() {
        #[derive(Debug)]
        struct Value(u8);
        impl PartialEq for Value {
            fn eq(&self, other: &Self) -> bool {
                assert_ne!(other.0, 2, "comparison failed");
                self.0 == other.0
            }
        }
        let publisher = Publisher::new(Value(1));
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || publisher.replace(Arc::new(Value(2)))
        ))
        .is_err());
        assert_eq!(publisher.watch().get().0, 1);
        publisher.replace(Arc::new(Value(3))).notification.notify();
        assert_eq!(publisher.watch().get().0, 3);
    }

    #[test]
    fn revisions_snapshots_and_subscription_lifetime() {
        let publisher = Publisher::new(10);
        let watch = publisher.watch();
        let (tx, rx) = mpsc::channel();
        let subscription = watch.subscribe(move || {
            tx.send(()).unwrap();
        });
        publisher.replace(Arc::new(10)).notification.notify();
        assert_eq!(watch.revision(), Revision(0));
        assert!(rx.try_recv().is_err());
        // Subscribe before the first read: a concurrent commit is observable by both paths.
        publisher.replace(Arc::new(11)).notification.notify();
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let (revision, snapshot) = watch.snapshot();
        assert_eq!((revision, *snapshot), (Revision(1), 11));
        let pending = publisher.replace(Arc::new(12));
        drop(subscription);
        pending.notification.notify();
        assert!(rx.try_recv().is_err());
        assert_eq!(*snapshot, 11);
    }

    #[test]
    fn waker_can_reenter_service_and_watch_after_commit_unlocks() {
        let service = Arc::new(Mutex::new(Publisher::new(0)));
        let watch = service.lock().unwrap().watch();
        let (tx, rx) = mpsc::channel();
        let reader = service.clone();
        let _subscription = watch.subscribe(move || {
            let value = reader.lock().unwrap().watch().get();
            tx.send(*value).unwrap();
        });
        std::thread::spawn(move || {
            let notification = { service.lock().unwrap().replace(Arc::new(1)).notification };
            notification.notify();
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
    }
}
