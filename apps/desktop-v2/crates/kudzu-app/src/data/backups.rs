//! The backup folder: snapshots of the library as Kudzu JSON, named by local time and reason.
//! The naming, daily and rotation rules are pure and live in `kudzu_core::interchange::backup`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Timelike, Utc};
use kudzu_core::interchange::backup::{
    filename, needs_daily_backup, parse_info, rotation_candidates, sort_newest, BackupInfo,
    BackupReason, BackupTimeZone,
};
use kudzu_core::library::json;
use kudzu_core::library::model::KudzuLibrary;

use super::DataError;

/// Manages backup snapshots in one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupStore {
    dir: PathBuf,
    time_zone: BackupTimeZone,
}

impl BackupStore {
    /// A store rooted at `dir` using the local time zone.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_time_zone(dir, BackupTimeZone::Local)
    }

    /// A store rooted at `dir` using `time_zone` for names and day boundaries.
    pub fn with_time_zone(dir: impl Into<PathBuf>, time_zone: BackupTimeZone) -> Self {
        Self {
            dir: dir.into(),
            time_zone,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub(super) fn time_zone(&self) -> BackupTimeZone {
        self.time_zone
    }

    /// Writes a snapshot of `library`. Never overwrites: a second backup with the same reason in
    /// the same second gets a `.2`, `.3`, … suffix, so a restore's safety copy can't be replaced.
    pub fn write(
        &self,
        library: &KudzuLibrary,
        reason: BackupReason,
        now: DateTime<Utc>,
    ) -> Result<BackupInfo, DataError> {
        let json = json::encode(library).map_err(|_| DataError::Encode)?;
        fs::create_dir_all(&self.dir).map_err(|e| DataError::BackupWrite(e.into()))?;
        let local = self.time_zone.utc_to_local(now);
        let mut sequence = 1;
        loop {
            let filename = filename(local, reason, sequence);
            let path = self.dir.join(&filename);
            // `create_new` makes the existence check and the claim one atomic step.
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file
                        .write_all(json.as_bytes())
                        .and_then(|_| file.sync_all())
                    {
                        // Never leave a half-written snapshot that would list as a backup.
                        drop(file);
                        let _ = fs::remove_file(&path);
                        return Err(DataError::BackupWrite(error.into()));
                    }
                    let created_at = self
                        .time_zone
                        .local_to_utc(local.with_nanosecond(0).unwrap_or(local))
                        .unwrap_or(now);
                    return Ok(BackupInfo {
                        path,
                        filename,
                        created_at,
                        reason,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => sequence += 1,
                Err(e) => return Err(DataError::BackupWrite(e.into())),
            }
        }
    }

    /// All valid backups in the directory, newest first.
    pub fn list(&self) -> Result<Vec<BackupInfo>, DataError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(DataError::BackupList(e.into())),
        };
        let mut infos: Vec<BackupInfo> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| parse_info(&entry.path(), self.time_zone))
            .collect();
        sort_newest(&mut infos);
        Ok(infos)
    }

    /// Loads and decodes a backup file.
    pub fn load(&self, path: &Path) -> Result<KudzuLibrary, DataError> {
        let json = fs::read_to_string(path).map_err(|e| DataError::Read(e.into()))?;
        json::decode(&json).map_err(|_| DataError::BackupUnreadable)
    }

    /// Deletes older backups so at most `keep` (the app uses 14) newest remain; returns those deleted.
    pub fn prune(&self, keep: usize) -> Result<Vec<BackupInfo>, DataError> {
        let to_delete = rotation_candidates(self.list()?, keep);
        for info in &to_delete {
            fs::remove_file(&info.path).map_err(|e| DataError::BackupWrite(e.into()))?;
        }
        Ok(to_delete)
    }

    /// True if no `daily` backup exists yet on the local calendar day of `now`.
    pub fn needs_daily_backup(&self, now: DateTime<Utc>) -> bool {
        match self.list() {
            Ok(all) => needs_daily_backup(&all, now, self.time_zone),
            Err(_) => true,
        }
    }

    /// Removes one backup this store wrote, by name. Used to drop a backup made for an attempt
    /// that had to be abandoned.
    pub(super) fn discard(&self, info: &BackupInfo) {
        let _ = fs::remove_file(self.dir.join(&info.filename));
    }
}
