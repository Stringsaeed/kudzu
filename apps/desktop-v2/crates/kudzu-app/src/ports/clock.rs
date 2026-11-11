//! Injected wall and monotonic clocks. Timers use only monotonic time.
use crate::watch::Subscription;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
    fn monotonic(&self) -> Duration;
    /// A fake wakes sleepers after advancing. Real clocks use the sleeper's timeout.
    fn subscribe(&self, wake: Arc<dyn Fn() + Send + Sync>) -> Subscription;
}

pub struct SystemClock(Instant);
impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
    fn monotonic(&self) -> Duration {
        self.0.elapsed()
    }
    fn subscribe(&self, _: Arc<dyn Fn() + Send + Sync>) -> Subscription {
        Subscription::default()
    }
}
