//! Time bounds around the app's lifetime: the startup wait for the engine feed, the guard
//! that stops the engine when startup fails, and quit work that must not stall a quit.
use super::AssemblyError;
use crate::expansion::FeedState;
use crate::ports::ExpansionEngine;
use crate::watch::Watch;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long startup waits for the engine feed and shutdown for the writer drain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timeouts {
    pub feed: Duration,
    pub drain: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            feed: Duration::from_secs(5),
            drain: Duration::from_secs(5),
        }
    }
}

/// Wait for the feed to apply `generation` to the new engine. A feed error or a timeout is
/// returned: the caller refuses to start rather than admit insertion against a stale index.
pub(super) fn wait_for_feed(
    feed: &Watch<FeedState>,
    generation: u64,
    timeout: Duration,
) -> Result<(), AssemblyError> {
    let deadline = Instant::now() + timeout;
    loop {
        let state = feed.get();
        if state.generation >= generation {
            return Ok(());
        }
        if let Some(error) = state.error {
            return Err(error.into());
        }
        if Instant::now() >= deadline {
            return Err(AssemblyError::FeedTimeout);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Stops the engine when startup fails after `start`, unless disarmed.
pub(super) struct StopOnError(Option<Arc<dyn ExpansionEngine>>);

impl StopOnError {
    pub(super) fn armed(engine: Arc<dyn ExpansionEngine>) -> Self {
        Self(Some(engine))
    }

    pub(super) fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for StopOnError {
    fn drop(&mut self) {
        if let Some(engine) = self.0.take() {
            engine.stop();
        }
    }
}

/// How long a quit waits for the clipboard's final collection. Collection deletes only
/// unreferenced blobs, so an unfinished one is safe and runs again on the next launch.
pub(super) const COLLECTION_DEADLINE: Duration = Duration::from_millis(1500);

/// Run `work` on its own thread and wait for it at most `deadline`. `None` leaves it running
/// detached, so a Dock or logout quit can't stall on it.
pub(crate) fn bounded<T: Send + 'static>(
    deadline: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("kudzu-quit-work".into())
        .spawn(move || {
            let _ = sender.send(work());
        });
    match spawned {
        Ok(_) => receiver.recv_timeout(deadline).ok(),
        Err(_) => None,
    }
}
