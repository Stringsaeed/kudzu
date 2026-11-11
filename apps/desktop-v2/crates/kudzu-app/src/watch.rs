//! Immutable snapshots; notifications are explicitly delivered after the service unlocks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};

struct Listener {
    active: AtomicBool,
    wake: Box<dyn Fn() + Send + Sync>,
}

struct State<T> {
    value: Arc<T>,
    revision: u64,
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
    pub fn revision(&self) -> u64 {
        self.snapshot().0
    }
    /// Read the revision and its snapshot atomically, avoiding a revision/read race in `poll`.
    pub fn snapshot(&self) -> (u64, Arc<T>) {
        let state = self.0.lock().unwrap();
        (state.revision, state.value.clone())
    }
    pub fn subscribe(&self, wake: impl Fn() + Send + Sync + 'static) -> Subscription {
        let listener = Arc::new(Listener {
            active: AtomicBool::new(true),
            wake: Box::new(wake),
        });
        let mut state = self.0.lock().unwrap();
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
        Self(Watch(Arc::new(Mutex::new(State {
            value: Arc::new(value),
            revision: 0,
            listeners: Vec::new(),
        }))))
    }
    pub fn watch(&self) -> Watch<T> {
        self.0.clone()
    }
}

impl<T: PartialEq> Publisher<T> {
    /// Commit while the owning service is locked. Call `notify()` only after releasing it.
    pub fn replace(&self, value: T) -> Notification {
        let mut state = self.0 .0.lock().unwrap();
        if *state.value == value {
            return Notification(Vec::new());
        }
        state.revision = state
            .revision
            .checked_add(1)
            .expect("snapshot revision exhausted");
        state.value = Arc::new(value);
        let listeners = state.listeners.iter().filter_map(Weak::upgrade).collect();
        state
            .listeners
            .retain(|listener| listener.strong_count() != 0);
        Notification(listeners)
    }
}

/// Drop unsubscribes, including notifications captured but not yet delivered.
/// A callback already executing may finish; dropping never waits for a callback.
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
                (listener.wake)();
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
    fn revisions_snapshots_and_subscription_lifetime() {
        let publisher = Publisher::new(10);
        let watch = publisher.watch();
        let (tx, rx) = mpsc::channel();
        let subscription = watch.subscribe(move || {
            tx.send(()).unwrap();
        });
        publisher.replace(10).notify();
        assert_eq!(watch.revision(), 0);
        assert!(rx.try_recv().is_err());
        // Subscribe before the first read: a concurrent commit is observable by both paths.
        publisher.replace(11).notify();
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let (revision, snapshot) = watch.snapshot();
        assert_eq!((revision, *snapshot), (1, 11));
        let pending = publisher.replace(12);
        drop(subscription);
        pending.notify();
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
            let notification = { service.lock().unwrap().replace(1) };
            notification.notify();
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 1);
    }
}
