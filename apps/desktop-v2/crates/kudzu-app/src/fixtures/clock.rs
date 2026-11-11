//! Manual time with wakeups; no sleeps or process-global locale/time-zone mutation.
use crate::ports::clock::Clock;
use crate::watch::{Publisher, Subscription};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub struct FakeClock {
    epoch: SystemTime,
    elapsed: Mutex<Duration>,
    changes: Publisher<Duration>,
}
impl FakeClock {
    pub fn new(epoch: SystemTime) -> Self {
        Self {
            epoch,
            elapsed: Mutex::new(Duration::ZERO),
            changes: Publisher::new(Duration::ZERO),
        }
    }
    pub fn advance(&self, duration: Duration) {
        let notification = {
            let mut elapsed = self.elapsed.lock().unwrap();
            *elapsed += duration;
            self.changes.replace(*elapsed)
        };
        notification.notify();
    }
}
impl Default for FakeClock {
    fn default() -> Self {
        Self::new(SystemTime::UNIX_EPOCH + Duration::from_secs(1_794_398_400))
    }
}
impl Clock for FakeClock {
    fn now(&self) -> SystemTime {
        self.epoch + self.monotonic()
    }
    fn monotonic(&self) -> Duration {
        *self.elapsed.lock().unwrap()
    }
    fn subscribe(&self, wake: Arc<dyn Fn() + Send + Sync>) -> Subscription {
        self.changes.watch().subscribe(move || wake())
    }
}
