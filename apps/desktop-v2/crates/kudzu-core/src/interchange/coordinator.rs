//! Import orchestration over files: previews, safety backups, merge + dedupe, restore and
//! export. Works on plain [`KudzuLibrary`] values; the caller persists the result.
//!
//! Marked for S29: everything here that reads or writes files moves to `kudzu-app::data`
//! together with [`BackupStore`]. The pure parts already live in [`super::merge`],
//! [`super::detect`] and [`crate::library::starter`].

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::backup::{BackupInfo, BackupReason, BackupStore};
use super::detect::{self, ImportSource};
use super::espanso::{self, EspansoImportResult, ImportWarning};
use super::merge::merge_library;
use super::{raycast, textexpander, ImportResult, InterchangeError};
use crate::library::dedupe::DedupeReport;
use crate::library::json;
use crate::library::model::KudzuLibrary;

/// Import warnings sharing one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningGroup {
    pub message: String,
    pub count: usize,
    pub warnings: Vec<ImportWarning>,
}

/// Groups warnings by message, most frequent first, then by message.
pub fn group_warnings(warnings: &[ImportWarning]) -> Vec<WarningGroup> {
    let mut buckets: BTreeMap<&str, Vec<ImportWarning>> = BTreeMap::new();
    for w in warnings {
        buckets.entry(&w.message).or_default().push(w.clone());
    }
    let mut groups: Vec<WarningGroup> = buckets
        .into_iter()
        .map(|(message, warnings)| WarningGroup {
            message: message.to_string(),
            count: warnings.len(),
            warnings,
        })
        .collect();
    groups.sort_by(|l, r| {
        r.count
            .cmp(&l.count)
            .then_with(|| l.message.cmp(&r.message))
    });
    groups
}

/// An Espanso directory parsed but not yet merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspansoDirectoryPreview {
    pub path: PathBuf,
    pub snippet_count: usize,
    pub group_count: usize,
    pub grouped_warnings: Vec<WarningGroup>,
    pub result: EspansoImportResult,
}

/// Imports `dir` without touching any library, for a confirmation screen.
pub fn preview_espanso_dir(dir: &Path) -> Result<EspansoDirectoryPreview, InterchangeError> {
    let result = espanso::import_dir(dir)?;
    Ok(EspansoDirectoryPreview {
        path: dir.to_path_buf(),
        snippet_count: result.library.snippets.len(),
        group_count: result.library.groups.len(),
        grouped_warnings: group_warnings(&result.warnings),
        result,
    })
}

/// Previews every default Espanso config directory that exists under `home`.
pub fn detect_espanso_dirs(home: &Path) -> Vec<EspansoDirectoryPreview> {
    espanso::default_config_dirs(home)
        .into_iter()
        .filter(|d| d.is_dir())
        .filter_map(|d| preview_espanso_dir(&d).ok())
        .collect()
}

/// A completed import (Espanso or Kudzu JSON), with its safety backup and dedupe details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    /// Snippets in the source, before deduplication.
    pub imported_snippets: usize,
    /// Groups in the source, before deduplication.
    pub imported_groups: usize,
    /// The `preImport` backup of the library as it was before merging.
    pub backup: BackupInfo,
    pub dedupe_report: DedupeReport,
    pub warnings: Vec<ImportWarning>,
    pub grouped_warnings: Vec<WarningGroup>,
}

/// Writes a `preImport` backup of `existing`, then merges `incoming` into it.
pub fn import_library(
    existing: &KudzuLibrary,
    incoming: &KudzuLibrary,
    warnings: Vec<ImportWarning>,
    backups: &BackupStore,
    now: DateTime<Utc>,
) -> Result<(KudzuLibrary, ImportSummary), InterchangeError> {
    let backup = backups.write(existing, BackupReason::PreImport, now)?;
    let (merged, dedupe_report) = merge_library(existing, incoming);
    let summary = ImportSummary {
        imported_snippets: incoming.snippets.len(),
        imported_groups: incoming.groups.len(),
        backup,
        dedupe_report,
        grouped_warnings: group_warnings(&warnings),
        warnings,
    };
    Ok((merged, summary))
}

/// Merges an Espanso import result into `existing` after a `preImport` backup.
pub fn import_espanso(
    existing: &KudzuLibrary,
    result: &EspansoImportResult,
    backups: &BackupStore,
    now: DateTime<Utc>,
) -> Result<(KudzuLibrary, ImportSummary), InterchangeError> {
    import_result(existing, result, backups, now)
}

/// Merges any converted import (Espanso, Raycast, TextExpander, Kudzu JSON) into `existing`
/// after a `preImport` backup.
pub fn import_result(
    existing: &KudzuLibrary,
    result: &ImportResult,
    backups: &BackupStore,
    now: DateTime<Utc>,
) -> Result<(KudzuLibrary, ImportSummary), InterchangeError> {
    import_library(
        existing,
        &result.library,
        result.warnings.clone(),
        backups,
        now,
    )
}

/// Works out what `path` is from its extension and, for JSON, its contents.
/// Folders are Espanso config directories. S29 moves this file access to the data service.
pub fn detect_import_source(path: &Path) -> Result<ImportSource, InterchangeError> {
    if path.is_dir() {
        return Ok(ImportSource::Espanso);
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    let contents = if extension == "json" {
        fs::read_to_string(path)
            .map_err(|e| InterchangeError::io(format!("Couldn't read {}", path.display()), e))?
    } else {
        String::new()
    };
    detect::file_format(&extension, &contents)
}

/// Reads and converts the import file at `path`, whichever app wrote it. Nothing is merged yet.
pub fn load_import(path: &Path) -> Result<ImportResult, InterchangeError> {
    match detect_import_source(path)? {
        ImportSource::Kudzu => {
            let json = fs::read_to_string(path).map_err(|e| {
                InterchangeError::io(format!("Couldn't read {}", path.display()), e)
            })?;
            Ok(ImportResult {
                library: json::decode(&json)?,
                warnings: vec![],
            })
        }
        ImportSource::Espanso if path.is_dir() => espanso::import_dir(path),
        ImportSource::Espanso => espanso::import_files(&[path.to_path_buf()]),
        ImportSource::Raycast => raycast::import_file(path),
        ImportSource::TextExpander => textexpander::import_file(path),
    }
}

/// Decodes Kudzu JSON and merges it into `existing` after a `preImport` backup.
pub fn import_kudzu_json(
    existing: &KudzuLibrary,
    json: &str,
    backups: &BackupStore,
    now: DateTime<Utc>,
) -> Result<(KudzuLibrary, ImportSummary), InterchangeError> {
    let incoming = json::decode(json)?;
    import_library(existing, &incoming, vec![], backups, now)
}

/// Loads the backup at `path` to replace `current`, first saving `current` as a `preRestore`
/// backup. Returns the restored library and the safety backup.
pub fn restore_backup(
    current: &KudzuLibrary,
    path: &Path,
    backups: &BackupStore,
    now: DateTime<Utc>,
) -> Result<(KudzuLibrary, BackupInfo), InterchangeError> {
    let restored = backups.load(path)?;
    let safety = backups.write(current, BackupReason::PreRestore, now)?;
    Ok((restored, safety))
}

/// Writes `library` as Espanso match files under `dir`; returns the written paths, sorted.
pub fn export_espanso_to_dir(
    library: &KudzuLibrary,
    dir: &Path,
) -> Result<Vec<PathBuf>, InterchangeError> {
    let mut written = Vec::new();
    for (relative, yaml) in espanso::export(library) {
        let path = dir.join(&relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                InterchangeError::io(format!("Couldn't create {}", parent.display()), e)
            })?;
        }
        fs::write(&path, yaml)
            .map_err(|e| InterchangeError::io(format!("Couldn't write {}", path.display()), e))?;
        written.push(path);
    }
    Ok(written)
}
