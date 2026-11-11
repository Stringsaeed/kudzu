//! Update checks, signed downloads, and install-on-quit / restart-to-update.
mod error;
mod feed;
mod stage;
mod state;
mod verify;

#[cfg(test)]
mod tests;

use crate::error::PortError;
use crate::ports::{
    Clock, Fetch, FetchRequest, InstallOutcome, Relauncher, UpdateInstaller, UpdatePackage,
};
use crate::schedule::{Scheduler, Timer};
use crate::tasks::Task;
use crate::watch::{Publisher, Watch};
use minisign_verify::PublicKey;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub use error::UpdateError;
pub use state::{display_percent, UpdateState, UNKNOWN_TOTAL_PERCENT};
pub use verify::{PRODUCTION_PUBKEY, PRODUCTION_PUBKEY_TAURI_B64};

pub const PRODUCTION_BUNDLE_ID: &str = "sh.saeed.kudzu";
pub const STABLE_FEED: &str =
    "https://github.com/Stringsaeed/kudzu/releases/latest/download/latest.json";
pub const BETA_FEED: &str =
    "https://github.com/Stringsaeed/kudzu/releases/download/update-feed/beta.json";
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
pub const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
pub const RESTART_DELAY: Duration = Duration::from_millis(400);
pub const FEED_TIMEOUT: Duration = Duration::from_secs(30);
pub const FEED_MAX_BYTES: usize = 64 * 1024;
pub const ARCHIVE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
pub const ARCHIVE_MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    User,
    Scheduled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prefs {
    pub auto_check: bool,
    pub channel: Channel,
}

pub struct Config {
    pub current_version: String,
    pub bundle_id: String,
    pub updates_allowed: bool,
    pub release_build: bool,
    /// Directory beside the running bundle; the staged archive is created here.
    pub stage_dir: PathBuf,
    pub(crate) public_key: PublicKey,
}

impl Config {
    pub fn for_identity(
        bundle_id: impl Into<String>,
        cloud_and_updates: bool,
        version: impl Into<String>,
    ) -> Self {
        let bundle_id = bundle_id.into();
        let updates_allowed = cloud_and_updates && bundle_id == PRODUCTION_BUNDLE_ID;
        Self {
            current_version: version.into(),
            bundle_id,
            updates_allowed,
            release_build: !cfg!(debug_assertions),
            stage_dir: PathBuf::new(),
            public_key: PublicKey::decode(PRODUCTION_PUBKEY)
                .expect("compiled-in updater public key"),
        }
    }
}

pub struct UpdatePorts {
    pub fetch: Arc<dyn Fetch>,
    pub installer: Arc<dyn UpdateInstaller>,
    pub relauncher: Arc<dyn Relauncher>,
    pub clock: Arc<dyn Clock>,
    pub scheduler: Arc<Scheduler>,
}

struct Pending {
    version: String,
    notes: Option<String>,
    date: Option<String>,
    bytes: Vec<u8>,
}

struct Inner {
    config: Config,
    prefs: Watch<Prefs>,
    fetch: Arc<dyn Fetch>,
    installer: Arc<dyn UpdateInstaller>,
    relauncher: Arc<dyn Relauncher>,
    clock: Arc<dyn Clock>,
    scheduler: Arc<Scheduler>,
    publisher: Publisher<UpdateState>,
    pending: Mutex<Option<Pending>>,
    busy: AtomicBool,
    cancel: Arc<AtomicBool>,
    check_timer: Mutex<Option<Timer>>,
    restart_timer: Mutex<Option<Timer>>,
}

#[derive(Clone)]
pub struct Updates {
    inner: Arc<Inner>,
}

impl Updates {
    pub fn new(
        config: Config,
        prefs: Watch<Prefs>,
        ports: UpdatePorts,
    ) -> Result<Self, UpdateError> {
        let start_schedule = config.release_build && config.updates_allowed;
        let inner = Inner {
            config,
            prefs,
            fetch: ports.fetch,
            installer: ports.installer,
            relauncher: ports.relauncher,
            clock: ports.clock,
            scheduler: ports.scheduler,
            publisher: Publisher::new(UpdateState::Idle),
            pending: Mutex::new(None),
            busy: AtomicBool::new(false),
            cancel: Arc::new(AtomicBool::new(false)),
            check_timer: Mutex::new(None),
            restart_timer: Mutex::new(None),
        };
        let updates = Self {
            inner: Arc::new(inner),
        };
        if start_schedule {
            let this = updates.clone();
            let timer = updates
                .inner
                .scheduler
                .every(FIRST_CHECK_DELAY, CHECK_INTERVAL, move || {
                    this.check(Trigger::Scheduled);
                })
                .map_err(|_| UpdateError::Schedule)?;
            *updates.inner.check_timer.lock().unwrap() = Some(timer);
        }
        Ok(updates)
    }

    pub fn state(&self) -> Watch<UpdateState> {
        self.inner.publisher.watch()
    }

    pub fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Release);
    }

    pub fn check(&self, trigger: Trigger) -> Task<UpdateState> {
        let this = self.clone();
        match Task::spawn("kudzu-update-check", move || this.run_check(trigger)) {
            Ok(task) => task,
            Err(_) => Task::done(self.fail_start()),
        }
    }

    pub fn restart_to_update(&self) -> Task<()> {
        let this = self.clone();
        match Task::spawn("kudzu-update-restart", move || this.run_restart()) {
            Ok(task) => task,
            Err(_) => {
                self.fail_start();
                Task::done(())
            }
        }
    }

    pub fn install_on_quit(&self) {
        let _ = self.install_pending();
    }

    fn publish(&self, next: UpdateState) {
        self.inner
            .publisher
            .replace(Arc::new(next))
            .notification
            .notify();
    }

    fn current(&self) -> UpdateState {
        (*self.inner.publisher.watch().get()).clone()
    }

    fn fail_start(&self) -> UpdateState {
        let next = UpdateState::Error {
            message: UpdateError::Check.to_string(),
            needs_move: false,
        };
        self.publish(next.clone());
        next
    }

    fn run_check(&self, trigger: Trigger) -> UpdateState {
        let quiet = matches!(trigger, Trigger::Scheduled);
        if !self.inner.config.updates_allowed {
            let next = UpdateState::Error {
                message: UpdateError::Dev.to_string(),
                needs_move: false,
            };
            self.publish(next.clone());
            return next;
        }
        if quiet && !self.inner.prefs.get().auto_check {
            return self.current();
        }
        let Some(_busy) = BusyGuard::try_acquire(&self.inner.busy) else {
            return self.current();
        };
        match self.current() {
            UpdateState::Ready { .. } | UpdateState::Restarting { .. } => {
                return self.current();
            }
            _ => {}
        }
        match self.inner.installer.probe() {
            Ok(probe) if probe.needs_move => {
                let next = UpdateState::Error {
                    message: UpdateError::NeedsMove.to_string(),
                    needs_move: true,
                };
                self.publish(next.clone());
                return next;
            }
            Ok(_) => {}
            Err(_) => return self.fail_check(quiet, UpdateError::Check),
        }
        if !quiet {
            self.publish(UpdateState::Checking);
        }
        self.inner.cancel.store(false, Ordering::Release);
        match self.download_and_verify() {
            Ok(Some(pending)) => {
                let next = UpdateState::Ready {
                    version: pending.version.clone(),
                    notes: pending.notes.clone(),
                    date: pending.date.clone(),
                };
                {
                    let mut slot = self.inner.pending.lock().unwrap();
                    *slot = Some(pending);
                    self.publish(next.clone());
                }
                next
            }
            Ok(None) => {
                let next = UpdateState::UpToDate {
                    checked_at: rfc3339(self.inner.clock.now()),
                };
                self.publish(next.clone());
                next
            }
            Err(CheckFail::Cancelled) => self.fail_cancel(quiet),
            Err(CheckFail::Check(error)) => self.fail_check(quiet, error),
            Err(CheckFail::Download(error)) => {
                if quiet {
                    let next = UpdateState::Idle;
                    self.publish(next.clone());
                    next
                } else {
                    self.fail_check(false, error)
                }
            }
        }
    }

    fn fail_cancel(&self, quiet: bool) -> UpdateState {
        if quiet {
            return self.current();
        }
        let next = UpdateState::Idle;
        self.publish(next.clone());
        next
    }

    fn fail_check(&self, quiet: bool, error: UpdateError) -> UpdateState {
        if quiet {
            return self.current();
        }
        let next = UpdateState::Error {
            message: error.to_string(),
            needs_move: false,
        };
        self.publish(next.clone());
        next
    }

    fn download_and_verify(&self) -> Result<Option<Pending>, CheckFail> {
        let channel = self.inner.prefs.get().channel;
        let feed_req = FetchRequest {
            url: feed::url(channel).into(),
            timeout: FEED_TIMEOUT,
            max_bytes: FEED_MAX_BYTES,
            cancel: self.inner.cancel.clone(),
        };
        let feed_bytes = self
            .inner
            .fetch
            .get(&feed_req, &|_, _| {})
            .map_err(|error| map_fetch(error, UpdateError::Check))?;
        if feed_bytes.len() > FEED_MAX_BYTES {
            return Err(CheckFail::Check(UpdateError::Check));
        }
        let parsed = feed::parse(&feed_bytes).map_err(CheckFail::Check)?;
        if !feed::is_newer(&parsed.version, &self.inner.config.current_version)
            .map_err(CheckFail::Check)?
        {
            return Ok(None);
        }
        let platform = feed::platform(&parsed).map_err(CheckFail::Check)?;
        feed::check_archive_url(&platform.url, &parsed.version).map_err(CheckFail::Check)?;
        let signature = verify::decode_signature(&platform.signature)
            .map_err(|_| CheckFail::Check(UpdateError::Signature))?;
        verify::bind_archive(signature.trusted_comment(), &parsed.version, &platform.url)
            .map_err(CheckFail::Check)?;
        let feed_version = parsed.version.clone();
        self.publish(UpdateState::Downloading {
            version: feed_version.clone(),
            downloaded: 0,
            total: None,
        });
        let last_percent = Mutex::new(u64::MAX);
        let archive_req = FetchRequest {
            url: platform.url.clone(),
            timeout: ARCHIVE_TIMEOUT,
            max_bytes: ARCHIVE_MAX_BYTES,
            cancel: self.inner.cancel.clone(),
        };
        let this = self.clone();
        let progress_version = feed_version.clone();
        let bytes = this
            .inner
            .fetch
            .get(&archive_req, &|downloaded, total| {
                let percent = display_percent(downloaded, total);
                let mut last = last_percent.lock().unwrap();
                if percent != *last {
                    *last = percent;
                    drop(last);
                    this.publish(UpdateState::Downloading {
                        version: progress_version.clone(),
                        downloaded,
                        total,
                    });
                }
            })
            .map_err(|error| map_fetch(error, UpdateError::Download))?;
        if bytes.len() > ARCHIVE_MAX_BYTES {
            return Err(CheckFail::Download(UpdateError::Download));
        }
        let version = verify::verify(
            &self.inner.config.public_key,
            &bytes,
            &platform.signature,
            &feed_version,
            &platform.url,
        )
        .map_err(CheckFail::Download)?;
        Ok(Some(Pending {
            version,
            notes: parsed.notes,
            date: parsed.pub_date,
            bytes,
        }))
    }

    fn run_restart(&self) {
        if !self.install_pending() {
            return;
        }
        let this = self.clone();
        match self.inner.scheduler.after(RESTART_DELAY, move || {
            let _ = this.inner.relauncher.relaunch();
        }) {
            Ok(timer) => *self.inner.restart_timer.lock().unwrap() = Some(timer),
            Err(_) => {
                let _ = self.inner.relauncher.relaunch();
            }
        }
    }

    fn install_pending(&self) -> bool {
        if !self.inner.config.updates_allowed {
            return false;
        }
        let Some(_busy) = BusyGuard::try_acquire(&self.inner.busy) else {
            return false;
        };
        self.cancel();
        let Some(pending) = self.inner.pending.lock().unwrap().take() else {
            return false;
        };
        self.publish(UpdateState::Restarting {
            version: pending.version.clone(),
        });
        let staged = match stage::archive(&self.inner.config.stage_dir, &pending.bytes) {
            Ok(staged) => staged,
            Err(_) => {
                self.restore_pending(pending, false);
                return false;
            }
        };
        let package = UpdatePackage {
            archive: staged.path.clone(),
            bundle_id: self.inner.config.bundle_id.clone(),
            version: pending.version.clone(),
            digest: staged.digest,
        };
        match self.inner.installer.install(&package) {
            Ok(InstallOutcome::Installed) => true,
            Ok(InstallOutcome::Skipped { installed_version }) => {
                match (
                    semver::Version::parse(&installed_version),
                    semver::Version::parse(&self.inner.config.current_version),
                ) {
                    (Ok(installed), Ok(running)) if installed.cmp_precedence(&running).is_gt() => {
                        true
                    }
                    (Ok(_), Ok(_)) => {
                        self.publish(UpdateState::UpToDate {
                            checked_at: rfc3339(self.inner.clock.now()),
                        });
                        false
                    }
                    _ => {
                        self.restore_pending(pending, false);
                        false
                    }
                }
            }
            Err(_) => {
                let needs_move = self
                    .inner
                    .installer
                    .probe()
                    .map(|probe| probe.needs_move)
                    .unwrap_or(false);
                self.restore_pending(pending, needs_move);
                false
            }
        }
    }

    fn restore_pending(&self, pending: Pending, needs_move: bool) {
        let message = if needs_move {
            UpdateError::NeedsMove.to_string()
        } else {
            UpdateError::Install.to_string()
        };
        *self.inner.pending.lock().unwrap() = Some(pending);
        self.publish(UpdateState::Error {
            message,
            needs_move,
        });
    }
}

enum CheckFail {
    Check(UpdateError),
    Download(UpdateError),
    Cancelled,
}

fn map_fetch(error: PortError, fallback: UpdateError) -> CheckFail {
    match error {
        PortError::Cancelled => CheckFail::Cancelled,
        PortError::InvalidData => CheckFail::Check(UpdateError::Feed),
        _ => {
            if matches!(fallback, UpdateError::Download) {
                CheckFail::Download(fallback)
            } else {
                CheckFail::Check(fallback)
            }
        }
    }
}

fn rfc3339(time: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339()
}

struct BusyGuard<'a>(&'a AtomicBool);

impl<'a> BusyGuard<'a> {
    fn try_acquire(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self(flag))
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
