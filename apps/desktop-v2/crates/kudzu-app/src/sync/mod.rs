//! Sync lifecycle over [`CloudStore`](crate::ports::CloudStore). The adapter only translates.
//!
//! Merges run on this worker, inside `library.transact`. The sync index and engine state are
//! written on this same thread only after `library.saved().durable` covers that revision, and
//! nothing is sent while that hold is outstanding. Clipboard history, settings and form memory
//! are never inputs. Kudzu Next (`production: false`) never calls `start`.
//!
//! Sign-in does not drop server knowledge. Sign-out drops it, restarts, and shows
//! [`SIGNED_OUT`] until availability is `Available`. An account switch drops it and keeps
//! syncing. Shutdown stops this worker, runs the caller's drain, then writes a checkpoint only
//! if the library revision is already durable.

mod apply;
mod files;
mod job;
mod status;

pub use kudzu_core::sync::CONTAINER_ID;
pub use status::{
    dev_gate, SyncStatus, DEV_UNAVAILABLE, FETCH_INTERVAL, NO_ACCOUNT, PERMANENT_FAILURE, QUOTA,
    RESTRICTED, SAVE_FAILED, SIGNED_OUT, START_FAILED, TEMPORARILY_UNAVAILABLE, UNENTITLED,
    USAGE_INTERVAL, WORKER_STOPPED,
};

use crate::error::{IoFailure, PortError};
use crate::library::LibraryService;
use crate::persist::PersistError;
use crate::ports::{
    AccountChange, Clock, CloudAvailability, CloudBatch, CloudEvents, CloudSend, CloudStore,
};
use crate::schedule::{Scheduler, Timer};
use crate::settings::Settings;
use crate::watch::{Publisher, Subscription, Watch};
use job::Job;
use kudzu_core::sync::SyncIndex;
use std::collections::VecDeque;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError, Weak};
use std::thread::JoinHandle;
#[cfg(test)]
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("The sync folder could not be opened.")]
    Storage(#[source] IoFailure),
    #[error("The sync index could not be read.")]
    Index(#[from] PersistError),
    #[error("Sync could not start.")]
    Worker(#[source] IoFailure),
}

/// `production` is true only for bundle id `sh.saeed.kudzu`. `device_id` is the library's id.
pub struct SyncConfig {
    pub production: bool,
    pub device_id: String,
    pub directory: PathBuf,
}

struct Inbox {
    stopped: bool,
    working: bool,
    enabled: Option<bool>,
    library_dirty: bool,
    sync_now: bool,
    events: VecDeque<Inbound>,
}

struct Work {
    enabled: Option<bool>,
    sync_now: bool,
    events: VecDeque<Inbound>,
}

enum Inbound {
    Fetched(u64, CloudBatch),
    Sent(u64, CloudSend),
    Account(u64, AccountChange),
    Availability(u64, CloudAvailability),
    Failed(u64, PortError),
    ZoneDeleted(u64),
    State(u64, Vec<u8>),
    Tick(u64),
    /// `library.saved()` moved. No generation: a durable library unblocks the hold.
    Durable,
}

impl Inbound {
    fn generation(&self) -> Option<u64> {
        match self {
            Self::Fetched(generation, _)
            | Self::Sent(generation, _)
            | Self::Account(generation, _)
            | Self::Availability(generation, _)
            | Self::Failed(generation, _)
            | Self::ZoneDeleted(generation)
            | Self::State(generation, _)
            | Self::Tick(generation) => Some(*generation),
            Self::Durable => None,
        }
    }
}

struct Shared {
    inbox: Mutex<Inbox>,
    changed: Condvar,
    idle: Condvar,
    status: Publisher<SyncStatus>,
    generation: AtomicU64,
    production: bool,
    device_id: String,
    library: Arc<LibraryService>,
    cloud: Arc<dyn CloudStore>,
    clock: Arc<dyn Clock>,
    scheduler: Arc<Scheduler>,
    timer: Mutex<Option<Timer>>,
    files: Mutex<files::SyncFiles>,
    disk_index: Mutex<SyncIndex>,
    hold: Mutex<Option<apply::Hold>>,
    /// Set only after `start` returns `Ok`. `stop` is skipped otherwise, including Dev and
    /// an unentitled `Unavailable`.
    port_started: AtomicBool,
    panic_turn: AtomicBool,
    watched_sync: Mutex<bool>,
}

fn bump(shared: &Shared) -> u64 {
    shared.generation.fetch_add(1, Ordering::SeqCst) + 1
}

fn enqueue(shared: &Shared, event: Inbound) {
    let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
    if inbox.stopped {
        return;
    }
    inbox.events.push_back(event);
    shared.changed.notify_one();
}

/// Stops the port once. A session that never started, including Dev and unentitled, is left alone.
fn stop_port(shared: &Shared) {
    if shared.port_started.swap(false, Ordering::SeqCst) {
        shared.cloud.stop();
    }
}

fn request_enabled(shared: &Shared, enabled: bool) {
    let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
    if inbox.stopped {
        return;
    }
    inbox.enabled = Some(enabled);
    shared.changed.notify_one();
}

fn request_sync(shared: &Shared) {
    let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
    if inbox.stopped {
        return;
    }
    inbox.sync_now = true;
    shared.changed.notify_one();
}

struct Bridge(Weak<Shared>);

impl CloudEvents for Bridge {
    fn fetched(&self, generation: u64, batch: CloudBatch) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::Fetched(generation, batch));
        }
    }
    fn sent(&self, generation: u64, report: CloudSend) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::Sent(generation, report));
        }
    }
    fn account_changed(&self, generation: u64, change: AccountChange) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::Account(generation, change));
        }
    }
    fn availability(&self, generation: u64, availability: CloudAvailability) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::Availability(generation, availability));
        }
    }
    fn failed(&self, generation: u64, error: PortError) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::Failed(generation, error));
        }
    }
    fn zone_deleted(&self, generation: u64) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::ZoneDeleted(generation));
        }
    }
    fn state(&self, generation: u64, bytes: Vec<u8>) {
        if bytes.is_empty() {
            return;
        }
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::State(generation, bytes));
        }
    }
}

fn wait(shared: &Shared) -> Option<Work> {
    let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if inbox.stopped {
            inbox.working = false;
            shared.idle.notify_all();
            return None;
        }
        let ready = inbox.enabled.is_some()
            || inbox.library_dirty
            || inbox.sync_now
            || !inbox.events.is_empty();
        if ready {
            inbox.working = true;
            return Some(Work {
                enabled: inbox.enabled.take(),
                sync_now: std::mem::take(&mut inbox.sync_now),
                events: std::mem::take(&mut inbox.events),
            });
        }
        inbox.working = false;
        shared.idle.notify_all();
        inbox = shared
            .changed
            .wait(inbox)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

fn run(shared: Arc<Shared>, mut job: Job) {
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        while let Some(work) = wait(&shared) {
            job.turn(&shared, work);
        }
    }));
    // The port stops before a shutdown drain. Bytes still to persist already sit in the hold.
    stop_port(&shared);
    if panicked.is_err() {
        shared
            .status
            .replace(Arc::new(SyncStatus::Error {
                message: WORKER_STOPPED.into(),
            }))
            .notification
            .notify();
        let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
        inbox.stopped = true;
        inbox.working = false;
        shared.idle.notify_all();
    }
}

/// Lifecycle and status. Windows call [`status`](Self::status), [`set_enabled`](Self::set_enabled)
/// and [`sync_now`](Self::sync_now).
pub struct SyncService {
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
    _subscriptions: [Subscription; 3],
}

impl SyncService {
    pub fn open(
        config: SyncConfig,
        library: Arc<LibraryService>,
        settings: Watch<Settings>,
        cloud: Arc<dyn CloudStore>,
        clock: Arc<dyn Clock>,
        scheduler: Arc<Scheduler>,
    ) -> Result<Self, SyncError> {
        let (sync_files, index) = files::SyncFiles::open(&config.directory)?;
        let enabled = settings.get().icloud_sync;
        let initial = match (config.production, enabled) {
            (_, false) => SyncStatus::Off,
            (false, true) => SyncStatus::Unavailable {
                reason: DEV_UNAVAILABLE.into(),
            },
            (true, true) => SyncStatus::Starting,
        };
        let shared = Arc::new(Shared {
            inbox: Mutex::new(Inbox {
                stopped: false,
                working: false,
                enabled: None,
                library_dirty: false,
                sync_now: false,
                events: VecDeque::new(),
            }),
            changed: Condvar::new(),
            idle: Condvar::new(),
            status: Publisher::new(initial),
            generation: AtomicU64::new(0),
            production: config.production,
            device_id: config.device_id,
            library: library.clone(),
            cloud,
            clock,
            scheduler,
            timer: Mutex::new(None),
            files: Mutex::new(sync_files),
            disk_index: Mutex::new(index.clone()),
            hold: Mutex::new(None),
            port_started: AtomicBool::new(false),
            panic_turn: AtomicBool::new(false),
            watched_sync: Mutex::new(enabled),
        });
        let job = Job::new(index, Arc::new(Bridge(Arc::downgrade(&shared))));
        let worker = std::thread::Builder::new()
            .name("kudzu-sync".into())
            .spawn({
                let shared = Arc::clone(&shared);
                move || run(shared, job)
            })
            .map_err(|error| SyncError::Worker(error.into()))?;
        let subscriptions = subscribe(&shared, &library, settings);
        Ok(Self {
            shared,
            worker: Mutex::new(Some(worker)),
            _subscriptions: subscriptions,
        })
    }

    pub fn status(&self) -> Watch<SyncStatus> {
        self.shared.status.watch()
    }

    /// Starts or stops. The settings subscription calls this too, and only when `icloud_sync` flips.
    pub fn set_enabled(&self, enabled: bool) {
        request_enabled(&self.shared, enabled);
    }

    /// Fetches now, and sends once the first full fetch has finished.
    pub fn sync_now(&self) {
        request_sync(&self.shared);
    }

    /// Stops the worker, runs `drain` (S40 passes the shared writer), then checkpoints if durable.
    pub fn shutdown_with(&self, drain: impl FnOnce()) {
        {
            let mut inbox = self
                .shared
                .inbox
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            inbox.stopped = true;
            self.shared.changed.notify_all();
        }
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            let _ = worker.join();
        }
        *self
            .shared
            .timer
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
        drain();
        let _ = apply::flush(&self.shared);
    }

    /// Idempotent. Does not drain the shared writer; [`Drop`] uses this path.
    pub fn shutdown(&self) {
        self.shutdown_with(|| {});
    }

    /// Blocks until the worker has applied every event queued so far and the durability hold is clear.
    #[cfg(test)]
    pub(crate) fn settle(&self) {
        self.wait_idle(true);
    }

    /// Blocks until the inbox is idle. A hold that is still waiting on the library does not count.
    #[cfg(test)]
    pub(crate) fn settle_pending(&self) {
        self.wait_idle(false);
    }

    #[cfg(test)]
    fn wait_idle(&self, durable: bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut inbox = self
            .shared
            .inbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            let stopped = inbox.stopped;
            let idle = !inbox.working
                && (stopped
                    || (inbox.enabled.is_none()
                        && !inbox.library_dirty
                        && !inbox.sync_now
                        && inbox.events.is_empty()));
            let held = durable
                && !stopped
                && self
                    .shared
                    .hold
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .is_some();
            if idle && !held {
                return;
            }
            let now = Instant::now();
            if now >= deadline {
                panic!("sync did not settle: {:?}", self.status().get());
            }
            let (guard, timeout) = self
                .shared
                .idle
                .wait_timeout(inbox, deadline - now)
                .unwrap_or_else(PoisonError::into_inner);
            inbox = guard;
            if timeout.timed_out() && inbox.working && Instant::now() >= deadline {
                panic!("sync did not settle: {:?}", self.status().get());
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn enqueue_tick(&self, generation: u64) {
        enqueue(&self.shared, Inbound::Tick(generation));
    }

    #[cfg(test)]
    pub(crate) fn panic_for_test(&self) {
        self.shared.panic_turn.store(true, Ordering::SeqCst);
        enqueue(&self.shared, Inbound::Tick(0));
    }
}

impl Drop for SyncService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn subscribe(
    shared: &Arc<Shared>,
    library: &LibraryService,
    settings: Watch<Settings>,
) -> [Subscription; 3] {
    let library_sub = library.watch().subscribe({
        let shared = Arc::downgrade(shared);
        move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let mut inbox = shared.inbox.lock().unwrap_or_else(PoisonError::into_inner);
            if inbox.stopped {
                return;
            }
            inbox.library_dirty = true;
            shared.changed.notify_one();
        }
    });
    let saved_sub = library.saved().subscribe({
        let shared = Arc::downgrade(shared);
        move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            enqueue(&shared, Inbound::Durable);
        }
    });
    let settings_watch = settings.clone();
    let settings_sub = settings.subscribe({
        let shared = Arc::downgrade(shared);
        move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let enabled = settings_watch.get().icloud_sync;
            let mut watched = shared
                .watched_sync
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if *watched == enabled {
                return;
            }
            *watched = enabled;
            drop(watched);
            request_enabled(&shared, enabled);
        }
    });
    request_enabled(shared, settings.get().icloud_sync);
    [library_sub, settings_sub, saved_sub]
}

#[cfg(test)]
mod tests;
