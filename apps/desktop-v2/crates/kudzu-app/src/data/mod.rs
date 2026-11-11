//! Import, export and backups: one place that orchestrates core's interchange code with the
//! library and owns the backup folder.
//!
//! - Imports parse before they change anything, take the `preImport` backup (which must succeed),
//!   then merge inside `library.transact`, so they land on the library as it is now.
//! - Restores load the backup, take a `preRestore` backup, then replace the library the same way.
//! - Daily backups run at startup and every six hours on the scheduler. Daily and manual backups
//!   prune to the newest 14; imports, restores and updates leave the folder alone.
//!
//! Long operations return a [`Task`]; the library publishes the result through its own watch.
//! Logs and errors never contain paths or file contents (see [`DataError`]).

mod backups;
mod error;
mod export;
mod import;
mod schedule;
mod source;
mod summary;
mod transaction;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use kudzu_core::interchange::backup::{BackupInfo, BackupReason, BackupTimeZone};

pub use backups::BackupStore;
pub use error::DataError;
pub use source::{detect_import_source, load_import};
pub use summary::{group_warnings, EspansoDetection, ImportSummary, WarningGroup};

use crate::library::LibraryService;
use crate::ports::Clock;
use crate::schedule::{Scheduler, Timer};
use crate::settings::SettingsService;
use crate::tasks::Task;
use crate::watch::{Publisher, Watch};

/// Backups kept by the daily and manual prune.
pub const KEEP_BACKUPS: usize = 14;
/// How often the daily-backup check runs after the one at startup.
pub const DAILY_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Where the service works, supplied by composition.
#[derive(Debug, Clone)]
pub struct DataConfig {
    /// The `backups/` folder.
    pub backups: PathBuf,
    /// The home folder, where Espanso's config lives.
    pub home: PathBuf,
    /// The time zone for backup names and "today". `Local` in the app.
    pub time_zone: BackupTimeZone,
}

struct Inner {
    library: Arc<LibraryService>,
    settings: Arc<SettingsService>,
    store: BackupStore,
    clock: Arc<dyn Clock>,
    home: PathBuf,
    busy: AtomicBool,
    list: Publisher<Vec<BackupInfo>>,
}

/// Held while an import, restore or backup runs; only one runs at a time.
struct Busy(Arc<Inner>);
impl Drop for Busy {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::Release);
    }
}

impl Inner {
    fn busy(this: &Arc<Self>) -> Result<Busy, DataError> {
        this.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| DataError::Busy)?;
        Ok(Busy(this.clone()))
    }

    fn now(&self) -> DateTime<Utc> {
        self.clock.now().into()
    }

    /// Publishes the backup folder's current listing when it differs from the last one.
    fn refresh_backups(&self) {
        let list = match self.store.list() {
            Ok(list) => list,
            Err(error) => {
                log::warn!("listing backups failed: {error}");
                return;
            }
        };
        if *self.list.watch().get() != list {
            self.list.replace(Arc::new(list)).notification.notify();
        }
    }
}

/// Import, export, backups and restore.
#[derive(Clone)]
pub struct DataService {
    inner: Arc<Inner>,
}

impl DataService {
    /// Startup: reads the backup folder once for [`backups`](Self::backups).
    pub fn new(
        config: DataConfig,
        library: Arc<LibraryService>,
        settings: Arc<SettingsService>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let store = BackupStore::with_time_zone(config.backups, config.time_zone);
        let initial = store.list().unwrap_or_default();
        Self {
            inner: Arc::new(Inner {
                library,
                settings,
                store,
                clock,
                home: config.home,
                busy: AtomicBool::new(false),
                list: Publisher::new(initial),
            }),
        }
    }

    /// The backups on disk, newest first. Updated after every operation here.
    pub fn backups(&self) -> Watch<Vec<BackupInfo>> {
        self.inner.list.watch()
    }

    /// Espanso config folders in the home folder that have snippets.
    pub fn detect_espanso(&self) -> Result<Task<Vec<EspansoDetection>>, DataError> {
        let inner = self.inner.clone();
        Ok(Task::spawn("kudzu-detect-espanso", move || {
            inner.detect_espanso()
        })?)
    }

    /// Imports a file or folder from any supported app (Kudzu or Raycast `.json`, Espanso
    /// `.yml` or a folder, TextExpander `.textexpander` or `.csv`), detected from its extension
    /// and contents.
    pub fn import(
        &self,
        source: PathBuf,
    ) -> Result<Task<Result<ImportSummary, DataError>>, DataError> {
        let busy = Inner::busy(&self.inner)?;
        let inner = self.inner.clone();
        Ok(Task::spawn("kudzu-import", move || {
            let _busy = busy;
            inner.import(&source)
        })?)
    }

    /// The suggested name for a Kudzu JSON export: `kudzu-YYYY-MM-DD.json`, by local date.
    pub fn export_json_name(&self) -> String {
        let local = self
            .inner
            .store
            .time_zone()
            .utc_to_local(self.inner.now())
            .date();
        format!("kudzu-{}.json", local.format("%Y-%m-%d"))
    }

    /// Writes the whole library as Kudzu JSON to `destination`.
    pub fn export_json(
        &self,
        destination: PathBuf,
    ) -> Result<Task<Result<(), DataError>>, DataError> {
        let inner = self.inner.clone();
        Ok(Task::spawn("kudzu-export-json", move || {
            let (_, library) = inner.library.snapshot();
            export::write_json(&library, &destination)
        })?)
    }

    /// Writes the library as Espanso match files under `folder`; resolves to the file count.
    pub fn export_espanso(
        &self,
        folder: PathBuf,
    ) -> Result<Task<Result<usize, DataError>>, DataError> {
        let inner = self.inner.clone();
        Ok(Task::spawn("kudzu-export-espanso", move || {
            let (_, library) = inner.library.snapshot();
            export::write_espanso(&library, &folder)
        })?)
    }

    /// Takes a `manual` backup, then prunes to the newest 14.
    pub fn backup_now(&self) -> Result<Task<Result<BackupInfo, DataError>>, DataError> {
        let busy = Inner::busy(&self.inner)?;
        let inner = self.inner.clone();
        Ok(Task::spawn("kudzu-backup", move || {
            let _busy = busy;
            let (_, library) = inner.library.snapshot();
            let result = inner
                .store
                .write(&library, BackupReason::Manual, inner.now());
            if result.is_ok() {
                inner.prune();
            }
            inner.refresh_backups();
            result
        })?)
    }

    /// Replaces the library with `backup` after taking a `preRestore` backup of the current one.
    /// Resolves to that safety backup.
    pub fn restore(
        &self,
        backup: BackupInfo,
    ) -> Result<Task<Result<BackupInfo, DataError>>, DataError> {
        let busy = Inner::busy(&self.inner)?;
        let inner = self.inner.clone();
        Ok(Task::spawn("kudzu-restore", move || {
            let _busy = busy;
            inner.restore(&backup)
        })?)
    }

    /// Startup, before the first window: on the first launch of a new version, backs the library
    /// up as `preUpdate`, then records the version in settings.
    pub fn note_app_version(&self, current: &str) -> Result<(), DataError> {
        self.inner.note_app_version(current)
    }

    /// Runs the daily-backup check now and every six hours. Keep the timer; dropping it stops
    /// the checks.
    pub fn start(&self, scheduler: &Scheduler) -> Result<Timer, DataError> {
        let weak = Arc::downgrade(&self.inner);
        Ok(
            scheduler.every(Duration::ZERO, DAILY_CHECK_INTERVAL, move || {
                // The timer thread only hands the work to a thread of its own.
                if let Some(inner) = weak.upgrade() {
                    let _ = Task::spawn("kudzu-daily-backup", move || inner.daily_backup());
                }
            })?,
        )
    }
}

#[cfg(test)]
mod tests;
