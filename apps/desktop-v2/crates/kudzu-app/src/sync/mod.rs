//! Sync lifecycle over [`CloudStore`](crate::ports::CloudStore). The adapter only translates.
//!
//! Merges run on this worker, inside `library.transact`, and the sync index is written in that
//! same turn before anything is sent. Clipboard history, settings and form memory are never
//! inputs. Kudzu Next (`production: false`) never calls `start`.
//!
//! `account_changed` always drops server knowledge and restarts with a new generation. Sign-in
//! is `availability(Available)` and does not reset. [`SIGNED_OUT`] is the sign-out sentence;
//! the port has one account callback, so a switch (which must keep syncing) cannot show it.
//! S37 calls `account_changed` for sign-out and account switch only.
//!
//! Shutdown stops this worker before the process drains writers. The index is not on the writer.

mod files;
mod job;
mod status;

pub use status::{
    dev_gate, SyncStatus, CONTAINER_ID, DEV_UNAVAILABLE, FETCH_INTERVAL, NO_ACCOUNT,
    PERMANENT_FAILURE, QUOTA, RESTRICTED, SAVE_FAILED, SIGNED_OUT, START_FAILED,
    TEMPORARILY_UNAVAILABLE, UNENTITLED, USAGE_INTERVAL, WORKER_STOPPED,
};

use crate::error::{IoFailure, PortError};
use crate::library::LibraryService;
use crate::persist::PersistError;
use crate::ports::{Clock, CloudAvailability, CloudBatch, CloudEvents, CloudSend, CloudStore};
use crate::schedule::{Scheduler, Timer};
use crate::settings::Settings;
use crate::watch::{Publisher, Subscription, Watch};
use job::Job;
use std::collections::VecDeque;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
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

pub(super) struct Work {
    enabled: Option<bool>,
    sync_now: bool,
    events: VecDeque<Inbound>,
}

enum Inbound {
    Fetched(u64, CloudBatch),
    Sent(u64, CloudSend),
    Account(u64),
    Availability(u64, CloudAvailability),
    Failed(u64, PortError),
    ZoneDeleted(u64),
    Tick(u64),
}

impl Inbound {
    fn generation(&self) -> u64 {
        match self {
            Self::Fetched(generation, _)
            | Self::Sent(generation, _)
            | Self::Account(generation)
            | Self::Availability(generation, _)
            | Self::Failed(generation, _)
            | Self::ZoneDeleted(generation)
            | Self::Tick(generation) => *generation,
        }
    }
}

pub(super) struct Shared {
    inbox: Mutex<Inbox>,
    changed: Condvar,
    idle: Condvar,
    status: Publisher<SyncStatus>,
    generation: AtomicU64,
    pub(super) production: bool,
    pub(super) device_id: String,
    pub(super) library: Arc<LibraryService>,
    pub(super) cloud: Arc<dyn CloudStore>,
    pub(super) clock: Arc<dyn Clock>,
    pub(super) scheduler: Arc<Scheduler>,
    pub(super) timer: Mutex<Option<Timer>>,
}

pub(super) fn bump(shared: &Shared) -> u64 {
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
    fn account_changed(&self, generation: u64) {
        if let Some(shared) = self.0.upgrade() {
            enqueue(&shared, Inbound::Account(generation));
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
    checkpoint: PathBuf,
    _subscriptions: [Subscription; 2],
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
        let checkpoint = sync_files.checkpoint.clone();
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
        });
        let job = Job::new(index, sync_files, Arc::new(Bridge(Arc::downgrade(&shared))));
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
            checkpoint,
            _subscriptions: subscriptions,
        })
    }

    pub fn status(&self) -> Watch<SyncStatus> {
        self.shared.status.watch()
    }

    /// Starts or stops. The settings subscription calls this too.
    pub fn set_enabled(&self, enabled: bool) {
        request_enabled(&self.shared, enabled);
    }

    /// Fetches now, and sends once the first full fetch has finished.
    pub fn sync_now(&self) {
        request_sync(&self.shared);
    }

    /// Idempotent. Stops the worker, then checkpoints, then stops the port.
    pub fn shutdown(&self) {
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
        bump(&self.shared);
        if let Ok(bytes) = self.shared.cloud.checkpoint() {
            let _ = files::write_checkpoint(&self.checkpoint, &bytes);
        }
        self.shared.cloud.stop();
    }

    /// Blocks until the worker has applied every event queued so far.
    #[cfg(test)]
    pub(crate) fn settle(&self) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut inbox = self
            .shared
            .inbox
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        loop {
            let idle = !inbox.working
                && (inbox.stopped
                    || (inbox.enabled.is_none()
                        && !inbox.library_dirty
                        && !inbox.sync_now
                        && inbox.events.is_empty()));
            if idle {
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
) -> [Subscription; 2] {
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
    let settings_watch = settings.clone();
    let settings_sub = settings.subscribe({
        let shared = Arc::downgrade(shared);
        move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            request_enabled(&shared, settings_watch.get().icloud_sync);
        }
    });
    request_enabled(shared, settings.get().icloud_sync);
    [library_sub, settings_sub]
}

#[cfg(test)]
mod tests;
