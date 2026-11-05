//! Local JSON backup snapshots of a [`KudzuLibrary`].
//!
//! Files are named `kudzu-backup-<yyyy-MM-dd'T'HHmmss>-<reason>[.N].json` in local time, exactly
//! like the Swift app, so either app can list the other's backups.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{
    DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc,
};
use serde::{Deserialize, Serialize};

use super::{kudzu_json, InterchangeError};
use crate::types::{iso8601, KudzuLibrary};

const PREFIX: &str = "kudzu-backup-";
const SUFFIX: &str = ".json";

/// Why a backup snapshot was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupReason {
    Daily,
    PreImport,
    /// Safety copy of the current library taken before a restore replaces it.
    PreRestore,
    Manual,
    /// Taken on the first launch of a new app version, before anything migrates.
    PreUpdate,
}

impl BackupReason {
    /// The name used in filenames and JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::PreImport => "preImport",
            Self::PreRestore => "preRestore",
            Self::Manual => "manual",
            Self::PreUpdate => "preUpdate",
        }
    }

    /// Parses a filename/JSON name.
    pub fn parse(s: &str) -> Option<Self> {
        [Self::Daily, Self::PreImport, Self::PreRestore, Self::Manual, Self::PreUpdate]
            .into_iter()
            .find(|r| r.as_str() == s)
    }
}

/// A stored backup file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: PathBuf,
    pub filename: String,
    #[serde(with = "iso8601")]
    pub created_at: DateTime<Utc>,
    pub reason: BackupReason,
}

/// The time zone used for backup filenames and the "same day" check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackupTimeZone {
    /// The system's local time zone (what the app uses).
    #[default]
    Local,
    /// A fixed offset, for deterministic tests.
    Fixed(FixedOffset),
}

impl BackupTimeZone {
    fn utc_to_local(self, date: DateTime<Utc>) -> NaiveDateTime {
        match self {
            Self::Local => date.with_timezone(&Local).naive_local(),
            Self::Fixed(offset) => date.with_timezone(&offset).naive_local(),
        }
    }

    fn local_to_utc(self, naive: NaiveDateTime) -> Option<DateTime<Utc>> {
        match self {
            Self::Local => Local
                .from_local_datetime(&naive)
                .earliest()
                .map(|d| d.with_timezone(&Utc)),
            Self::Fixed(offset) => offset
                .from_local_datetime(&naive)
                .earliest()
                .map(|d| d.with_timezone(&Utc)),
        }
    }
}

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
        let json = kudzu_json::encode(library)?;
        let local = self.time_zone.utc_to_local(now);
        let timestamp = format!(
            "{:04}-{:02}-{:02}T{:02}{:02}{:02}",
            local.year(),
            local.month(),
            local.day(),
            local.hour(),
            local.minute(),
            local.second()
        );
        let mut sequence = 1;
        loop {
            let suffix = if sequence == 1 {
                String::new()
            } else {
                format!(".{sequence}")
            };
            let filename = format!("{PREFIX}{timestamp}-{}{suffix}{SUFFIX}", reason.as_str());
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
            .filter_map(|entry| self.parse_info(&entry.path()))
            .collect();
        infos.sort_by(|l, r| {
            r.created_at
                .cmp(&l.created_at)
                .then_with(|| sequence_of(&r.filename).cmp(&sequence_of(&l.filename)))
                .then_with(|| r.filename.cmp(&l.filename))
        });
        Ok(infos)
    }

    /// Loads and decodes a backup file.
    pub fn load(&self, path: &Path) -> Result<KudzuLibrary, InterchangeError> {
        let json = fs::read_to_string(path)
            .map_err(|e| InterchangeError::io(format!("Couldn't read {}", path.display()), e))?;
        kudzu_json::decode(&json)
    }

    /// Deletes older backups so at most `keep` (the app uses 14) newest remain; returns those deleted.
    pub fn prune(&self, keep: usize) -> Result<Vec<BackupInfo>, InterchangeError> {
        let all = self.list()?;
        let to_delete: Vec<BackupInfo> = all.into_iter().skip(keep).collect();
        for info in &to_delete {
            fs::remove_file(&info.path).map_err(|e| {
                InterchangeError::io(format!("Couldn't delete {}", info.path.display()), e)
            })?;
        }
        Ok(to_delete)
    }

    /// True if no `daily` backup exists yet on the local calendar day of `now`.
    pub fn needs_daily_backup(&self, now: DateTime<Utc>) -> bool {
        let today = self.local_day(now);
        match self.list() {
            Ok(all) => !all
                .iter()
                .any(|i| i.reason == BackupReason::Daily && self.local_day(i.created_at) == today),
            Err(_) => true,
        }
    }

    fn local_day(&self, date: DateTime<Utc>) -> NaiveDate {
        self.time_zone.utc_to_local(date).date()
    }

    fn parse_info(&self, path: &Path) -> Option<BackupInfo> {
        let filename = path.file_name()?.to_str()?.to_string();
        let body = filename.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
        let (timestamp, reason_part) = body.rsplit_once('-')?;
        // Drop an optional same-second sequence suffix (`daily.2`).
        let reason = BackupReason::parse(reason_part.split('.').next()?)?;
        let naive = NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H%M%S").ok()?;
        if timestamp.len() != 17 {
            return None;
        }
        let created_at = self.time_zone.local_to_utc(naive)?;
        Some(BackupInfo {
            path: path.to_path_buf(),
            filename,
            created_at,
            reason,
        })
    }
}

/// The same-second sequence number in a backup filename (1 when there's no suffix).
fn sequence_of(filename: &str) -> u32 {
    let stem = filename.strip_suffix(SUFFIX).unwrap_or(filename);
    stem.rsplit_once('.')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(1)
}
