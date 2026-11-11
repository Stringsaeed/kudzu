//! A single timer thread. Callbacks enqueue work; they never perform long operations.

use crate::error::IoFailure;
use crate::ports::clock::Clock;
use crate::watch::Subscription;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
    #[error("The timer could not start.")]
    Start(#[from] IoFailure),
    #[error("The timer has stopped.")]
    Stopped,
    #[error("The repeat interval must be positive.")]
    ZeroInterval,
}
struct Entry {
    due: Duration,
    id: u64,
    interval: Option<Duration>,
    active: Arc<AtomicBool>,
    run: Arc<dyn Fn() + Send + Sync>,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        (self.due, self.id) == (other.due, other.id)
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (other.due, other.id).cmp(&(self.due, self.id))
    }
}
#[derive(Default)]
struct Queue {
    stopped: bool,
    next: u64,
    entries: BinaryHeap<Entry>,
}
#[derive(Default)]
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
}

/// Keep each timer handle in its service; dropping it cancels future executions.
pub struct Timer {
    active: Arc<AtomicBool>,
    shared: Weak<Shared>,
}
impl Drop for Timer {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
        if let Some(shared) = self.shared.upgrade() {
            let _queue = shared.queue.lock().unwrap();
            shared.changed.notify_one();
        }
    }
}

pub struct Scheduler {
    shared: Arc<Shared>,
    clock: Arc<dyn Clock>,
    worker: Mutex<Option<JoinHandle<()>>>,
    _clock_wake: Subscription,
}
impl Scheduler {
    pub fn new(clock: Arc<dyn Clock>) -> Result<Self, ScheduleError> {
        let shared = Arc::new(Shared::default());
        let wake = Arc::downgrade(&shared);
        let subscription = clock.subscribe(Arc::new(move || {
            if let Some(shared) = wake.upgrade() {
                let _queue = shared.queue.lock().unwrap();
                shared.changed.notify_one();
            }
        }));
        let thread_state = shared.clone();
        let thread_clock = clock.clone();
        let worker = std::thread::Builder::new()
            .name("kudzu-schedule".into())
            .spawn(move || run(thread_state, thread_clock))
            .map_err(IoFailure::from)?;
        Ok(Self {
            shared,
            clock,
            worker: Mutex::new(Some(worker)),
            _clock_wake: subscription,
        })
    }
    pub fn after(
        &self,
        delay: Duration,
        run: impl Fn() + Send + Sync + 'static,
    ) -> Result<Timer, ScheduleError> {
        self.add(delay, None, Arc::new(run))
    }
    /// Missed ticks coalesce to one call; the next deadline is one interval after that call.
    pub fn every(
        &self,
        delay: Duration,
        interval: Duration,
        run: impl Fn() + Send + Sync + 'static,
    ) -> Result<Timer, ScheduleError> {
        if interval.is_zero() {
            return Err(ScheduleError::ZeroInterval);
        }
        self.add(delay, Some(interval), Arc::new(run))
    }
    fn add(
        &self,
        delay: Duration,
        interval: Option<Duration>,
        run: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Timer, ScheduleError> {
        let mut queue = self.shared.queue.lock().unwrap();
        if queue.stopped {
            return Err(ScheduleError::Stopped);
        }
        let id = queue.next;
        queue.next += 1;
        let active = Arc::new(AtomicBool::new(true));
        queue.entries.push(Entry {
            due: self.clock.monotonic().saturating_add(delay),
            id,
            interval,
            active: active.clone(),
            run,
        });
        self.shared.changed.notify_one();
        Ok(Timer {
            active,
            shared: Arc::downgrade(&self.shared),
        })
    }
    /// Synchronous, idempotent. Callbacks must not wait for a UI loop or call shutdown.
    pub fn shutdown(&self) {
        let mut worker = self.worker.lock().unwrap();
        {
            let mut queue = self.shared.queue.lock().unwrap();
            queue.stopped = true;
            self.shared.changed.notify_one();
        }
        if let Some(worker) = worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(shared: Arc<Shared>, clock: Arc<dyn Clock>) {
    let mut queue = shared.queue.lock().unwrap();
    loop {
        if queue.stopped {
            let entries = std::mem::take(&mut queue.entries);
            drop(queue);
            drop(entries);
            return;
        }
        if queue
            .entries
            .peek()
            .is_some_and(|entry| !entry.active.load(Ordering::Acquire))
        {
            let cancelled = queue.entries.pop();
            drop(queue);
            drop(cancelled);
            queue = shared.queue.lock().unwrap();
            continue;
        }
        let Some(entry) = queue.entries.peek() else {
            queue = shared.changed.wait(queue).unwrap();
            continue;
        };
        let now = clock.monotonic();
        if entry.due > now {
            let wait = entry.due - now;
            queue = shared.changed.wait_timeout(queue, wait).unwrap().0;
            continue;
        }
        let mut entry = queue.entries.pop().unwrap();
        // Requeue before calling out, so an advance from the callback cannot lose the next tick.
        if let Some(interval) = entry.interval {
            entry.due = now.saturating_add(interval);
            queue.entries.push(Entry {
                due: entry.due,
                id: entry.id,
                interval: entry.interval,
                active: entry.active.clone(),
                run: entry.run.clone(),
            });
        }
        drop(queue);
        if entry.active.load(Ordering::Acquire) {
            (entry.run)();
        }
        drop(entry);
        queue = shared.queue.lock().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::clock::FakeClock;
    use std::sync::mpsc;
    #[test]
    fn shutdown_drops_callback_captures_without_holding_the_timer_lock() {
        let scheduler = Scheduler::new(Arc::new(FakeClock::default())).unwrap();
        let inner = scheduler.after(Duration::from_secs(60), || {}).unwrap();
        let outer = scheduler
            .after(Duration::from_secs(30), move || {
                let _ = &inner;
            })
            .unwrap();
        let (done, rx) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let _outer = outer;
            scheduler.shutdown();
            done.send(()).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(2))
            .expect("callback destruction deadlocked shutdown");
        thread.join().unwrap();
    }
    #[test]
    fn fake_clock_drives_order_repeat_coalescing_and_cancellation() {
        let clock = Arc::new(FakeClock::default());
        let scheduler = Scheduler::new(clock.clone()).unwrap();
        let (tx, rx) = mpsc::channel();
        let first = tx.clone();
        let repeating = scheduler
            .every(
                Duration::from_secs(20),
                Duration::from_secs(60),
                move || {
                    first.send(20).unwrap();
                },
            )
            .unwrap();
        let once = scheduler
            .after(Duration::from_secs(10), move || {
                tx.send(10).unwrap();
            })
            .unwrap();
        clock.advance(Duration::from_secs(9));
        assert!(rx.try_recv().is_err());
        clock.advance(Duration::from_secs(1));
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 10);
        clock.advance(Duration::from_secs(10));
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 20);
        clock.advance(Duration::from_secs(600));
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), 20);
        assert!(rx.try_recv().is_err());
        drop(repeating);
        drop(once);
        clock.advance(Duration::from_secs(100));
        scheduler.shutdown();
        assert!(rx.try_recv().is_err());
        assert!(scheduler.after(Duration::ZERO, || {}).is_err());
    }
}
