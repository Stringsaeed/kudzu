//! Backup file operations retained until S29 moves them to `kudzu-app::data`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Timelike, Utc};

use super::{
    filename, needs_daily_backup, parse_info, rotation_candidates, sort_newest, BackupInfo,
    BackupReason, BackupTimeZone,
};
use crate::interchange::InterchangeError;
use crate::library::{json, model::KudzuLibrary};

/// Manages backup snapshots in one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupStore {
    pub dir: PathBuf,
    pub time_zone: BackupTimeZone,
}

impl BackupStore {
    /// A store rooted at `dir` using the local time zone.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            time_zone: BackupTimeZone::Local,
        }
    }

    /// A store rooted at `dir` using `time_zone` for names and day boundaries.
    pub fn with_time_zone(dir: impl Into<PathBuf>, time_zone: BackupTimeZone) -> Self {
        Self {
            dir: dir.into(),
            time_zone,
        }
    }

    /// Writes a snapshot of `library`. Never overwrites: a second backup with the same reason in
    /// the same second gets a `.2`, `.3`, … suffix, so a restore's safety copy can't be replaced.
    pub fn write(
        &self,
        library: &KudzuLibrary,
        reason: BackupReason,
        now: DateTime<Utc>,
    ) -> Result<BackupInfo, InterchangeError> {
        fs::create_dir_all(&self.dir).map_err(|e| {
            InterchangeError::io(format!("Couldn't create {}", self.dir.display()), e)
        })?;
        let json = json::encode(library)?;
        let local = self.time_zone.utc_to_local(now);
        let mut sequence = 1;
        loop {
            let filename = filename(local, reason, sequence);
            let path = self.dir.join(&filename);
            // `create_new` makes the existence check and the claim one atomic step.
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    file.write_all(json.as_bytes())
                        .and_then(|_| file.sync_all())
                        .map_err(|e| {
                            InterchangeError::io(format!("Couldn't write {}", path.display()), e)
                        })?;
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
                Err(e) => {
                    return Err(InterchangeError::io(
                        format!("Couldn't create {}", path.display()),
                        e,
                    ))
                }
            }
        }
    }

    /// All valid backups in the directory, newest first.
    pub fn list(&self) -> Result<Vec<BackupInfo>, InterchangeError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => {
                return Err(InterchangeError::io(
                    format!("Couldn't list {}", self.dir.display()),
                    e,
                ))
            }
        };
        let mut infos: Vec<BackupInfo> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| parse_info(&entry.path(), self.time_zone))
            .collect();
        sort_newest(&mut infos);
        Ok(infos)
    }

    /// Loads and decodes a backup file.
    pub fn load(&self, path: &Path) -> Result<KudzuLibrary, InterchangeError> {
        let json = fs::read_to_string(path)
            .map_err(|e| InterchangeError::io(format!("Couldn't read {}", path.display()), e))?;
        json::decode(&json)
    }

    /// Deletes older backups so at most `keep` (the app uses 14) newest remain; returns those deleted.
    pub fn prune(&self, keep: usize) -> Result<Vec<BackupInfo>, InterchangeError> {
        let all = self.list()?;
        let to_delete = rotation_candidates(all, keep);
        for info in &to_delete {
            fs::remove_file(&info.path).map_err(|e| {
                InterchangeError::io(format!("Couldn't delete {}", info.path.display()), e)
            })?;
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
}
