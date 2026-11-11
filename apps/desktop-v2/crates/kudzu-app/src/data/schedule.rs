//! The backups nobody asks for: the daily one, and the one before a new version first runs.

use kudzu_core::interchange::backup::BackupReason;

use super::{DataError, Inner, KEEP_BACKUPS};

impl Inner {
    /// Writes a daily backup if none exists for today (local calendar), keeping the last 14.
    /// A failure is logged; the next check tries again. Skipped while an import or restore runs.
    pub(super) fn daily_backup(self: &std::sync::Arc<Self>) {
        let Ok(_busy) = Self::busy(self) else {
            return;
        };
        let now = self.now();
        if !self.store.needs_daily_backup(now) {
            return;
        }
        let (_, snapshot) = self.library.snapshot();
        if let Err(error) = self.store.write(&snapshot, BackupReason::Daily, now) {
            log::error!("daily backup failed: {error}");
        }
        self.prune();
        self.refresh_backups();
    }

    /// On the first launch of a new version, keeps a copy of the library from before it ran
    /// (so a bad release can always be undone), then records the version. A failed copy is
    /// logged and the launch goes on, as it always has.
    pub(super) fn note_app_version(&self, current: &str) -> Result<(), DataError> {
        let previous = self.settings.watch().get().last_run_version.clone();
        if previous.as_deref() == Some(current) {
            return Ok(());
        }
        if previous.is_some() {
            log::info!("updated to {current}");
            let (_, snapshot) = self.library.snapshot();
            if let Err(error) = self
                .store
                .write(&snapshot, BackupReason::PreUpdate, self.now())
            {
                log::error!("pre-update backup failed: {error}");
            }
            self.refresh_backups();
        }
        self.settings
            .update(|settings| settings.last_run_version = Some(current.to_owned()))?;
        Ok(())
    }

    /// Daily and manual backups prune to the newest 14; imports, restores and updates do not.
    pub(super) fn prune(&self) {
        if let Err(error) = self.store.prune(KEEP_BACKUPS) {
            log::warn!("pruning backups failed: {error}");
        }
    }
}
