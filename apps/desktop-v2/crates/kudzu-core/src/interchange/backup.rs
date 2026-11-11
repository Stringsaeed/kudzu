//! Pure backup naming, calendar-day and rotation rules.
//!
//! The backup folder itself (writing, listing, pruning) is `kudzu_app::data::BackupStore`; this
//! module is the rules it follows.
//!
//! Files are named `kudzu-backup-<yyyy-MM-dd'T'HHmmss>-<reason>[.N].json` in local time, exactly
//! like the Swift app, so either app can list the other's backups.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, FixedOffset, Local, NaiveDateTime, TimeZone, Timelike, Utc};

const PREFIX: &str = "kudzu-backup-";
const SUFFIX: &str = ".json";

/// Why a backup snapshot was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
        [
            Self::Daily,
            Self::PreImport,
            Self::PreRestore,
            Self::Manual,
            Self::PreUpdate,
        ]
        .into_iter()
        .find(|r| r.as_str() == s)
    }
}

/// A stored backup file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub filename: String,
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
    /// The wall-clock time in this zone.
    pub fn utc_to_local(self, date: DateTime<Utc>) -> NaiveDateTime {
        match self {
            Self::Local => date.with_timezone(&Local).naive_local(),
            Self::Fixed(offset) => date.with_timezone(&offset).naive_local(),
        }
    }

    /// The instant a wall-clock time in this zone names (the earlier one when it repeats).
    pub fn local_to_utc(self, naive: NaiveDateTime) -> Option<DateTime<Utc>> {
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

/// The same-second sequence number in a backup filename (1 when there's no suffix).
fn sequence_of(filename: &str) -> u32 {
    let stem = filename.strip_suffix(SUFFIX).unwrap_or(filename);
    stem.rsplit_once('.')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(1)
}

/// A backup filename for a local timestamp and a same-second sequence (starting at 1).
pub fn filename(local: NaiveDateTime, reason: BackupReason, sequence: u32) -> String {
    let timestamp = format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}",
        local.year(),
        local.month(),
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    );
    let suffix = if sequence == 1 {
        String::new()
    } else {
        format!(".{sequence}")
    };
    let filename = format!("{PREFIX}{timestamp}-{}{suffix}{SUFFIX}", reason.as_str());
    filename
}

/// Orders backups by time, same-second sequence, then filename, newest first.
pub fn sort_newest(infos: &mut [BackupInfo]) {
    infos.sort_by(|l, r| {
        r.created_at
            .cmp(&l.created_at)
            .then_with(|| sequence_of(&r.filename).cmp(&sequence_of(&l.filename)))
            .then_with(|| r.filename.cmp(&l.filename))
    });
}

/// Older backups to remove after keeping the newest `keep` snapshots.
pub fn rotation_candidates(mut backups: Vec<BackupInfo>, keep: usize) -> Vec<BackupInfo> {
    sort_newest(&mut backups);
    backups.into_iter().skip(keep).collect()
}

/// True if no daily snapshot exists on the local calendar day of `now`.
pub fn needs_daily_backup(
    backups: &[BackupInfo],
    now: DateTime<Utc>,
    time_zone: BackupTimeZone,
) -> bool {
    let today = time_zone.utc_to_local(now).date();
    !backups.iter().any(|i| {
        i.reason == BackupReason::Daily && time_zone.utc_to_local(i.created_at).date() == today
    })
}

/// Parses a backup name without touching the filesystem.
pub fn parse_info(path: &Path, time_zone: BackupTimeZone) -> Option<BackupInfo> {
    let filename = path.file_name()?.to_str()?.to_string();
    let body = filename.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    let (timestamp, reason_part) = body.rsplit_once('-')?;
    // Drop an optional same-second sequence suffix (`daily.2`).
    let reason = BackupReason::parse(reason_part.split('.').next()?)?;
    let naive = NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H%M%S").ok()?;
    if timestamp.len() != 17 {
        return None;
    }
    let created_at = time_zone.local_to_utc(naive)?;
    Some(BackupInfo {
        path: path.to_path_buf(),
        filename,
        created_at,
        reason,
    })
}
