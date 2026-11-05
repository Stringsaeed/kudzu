//! Import orchestration: previews, safety backups, merge + dedupe, restore, export and starter
//! snippets. Works on plain [`KudzuLibrary`] values; the caller persists the result.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::backup::{BackupInfo, BackupReason, BackupStore};
use super::dedupe::{self, DedupeReport};
use super::espanso::{self, EspansoImportResult, ImportWarning};
use super::stats::latest;
use super::{kudzu_json, InterchangeError};
use crate::types::{GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};

/// Starter trigger used in onboarding.
pub const SAMPLE_TRIGGER: &str = ";kudzu";
/// Starter expansion used in onboarding.
pub const SAMPLE_EXPANSION: &str = "Kudzu is working!";
/// Name of the group that holds the starter snippets.
pub const GETTING_STARTED_GROUP: &str = "Getting Started";

/// Import warnings sharing one message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// Adds `incoming` to `existing`, then dedupes.
///
/// Records whose IDs already exist are kept unless the incoming snippet has a newer `updated_at`.
/// Usage rows for an existing (snippet, device) take the max of each counter: they are two
/// snapshots of the same counter (e.g. this device's own backup), so summing would double count.
/// Groups whose parent is missing, or whose parent link would form a cycle, become root groups.
pub fn merge_library(
    existing: &KudzuLibrary,
    incoming: &KudzuLibrary,
) -> (KudzuLibrary, DedupeReport) {
    let mut merged = existing.clone();

    // Groups: insert new ones without parents, then link parents that exist and don't loop.
    let mut parents: HashMap<Uuid, Option<Uuid>> =
        merged.groups.iter().map(|g| (g.id, g.parent_id)).collect();
    let mut inserted = Vec::new();
    for g in &incoming.groups {
        if parents.contains_key(&g.id) {
            continue;
        }
        parents.insert(g.id, None);
        inserted.push(merged.groups.len());
        merged.groups.push(GroupRecord {
            parent_id: None,
            ..g.clone()
        });
    }
    for index in inserted {
        let id = merged.groups[index].id;
        let wanted = incoming
            .groups
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| g.parent_id);
        let Some(parent) = wanted.filter(|p| parents.contains_key(p)) else {
            continue;
        };
        if !is_descendant_or_self(parent, id, &parents) {
            parents.insert(id, Some(parent));
            merged.groups[index].parent_id = Some(parent);
        }
    }

    // Snippets: newer incoming edits win; group links must point at a known group.
    let known_group = |gid: Option<Uuid>| gid.filter(|g| parents.contains_key(g));
    let mut positions: HashMap<Uuid, usize> = HashMap::new();
    for (i, s) in merged.snippets.iter().enumerate() {
        positions.entry(s.id).or_insert(i);
    }
    for record in &incoming.snippets {
        let record = SnippetRecord {
            group_id: known_group(record.group_id),
            ..record.clone()
        };
        match positions.get(&record.id) {
            Some(&i) => {
                if record.updated_at > merged.snippets[i].updated_at {
                    merged.snippets[i] = record;
                }
            }
            None => {
                positions.insert(record.id, merged.snippets.len());
                merged.snippets.push(record);
            }
        }
    }

    // Usage: max of each counter for a known (snippet, device).
    let mut rows: HashMap<(Uuid, String), usize> = HashMap::new();
    for (i, u) in merged.usage.iter().enumerate() {
        rows.entry((u.snippet_id, u.device_id.clone())).or_insert(i);
    }
    for record in &incoming.usage {
        match rows.get(&(record.snippet_id, record.device_id.clone())) {
            Some(&i) => {
                let row = &mut merged.usage[i];
                *row = UsageRecord {
                    count: row.count.max(record.count),
                    chars_saved: row.chars_saved.max(record.chars_saved),
                    last_used_at: latest(row.last_used_at, record.last_used_at),
                    ..row.clone()
                };
            }
            None => {
                rows.insert(
                    (record.snippet_id, record.device_id.clone()),
                    merged.usage.len(),
                );
                merged.usage.push(record.clone());
            }
        }
    }

    dedupe::dedupe(&merged, &HashMap::new())
}

/// True if `candidate` is `group` or sits anywhere below it.
fn is_descendant_or_self(
    candidate: Uuid,
    group: Uuid,
    parents: &HashMap<Uuid, Option<Uuid>>,
) -> bool {
    let mut current = Some(candidate);
    let mut seen = HashSet::new();
    while let Some(node) = current.filter(|n| seen.insert(*n)) {
        if node == group {
            return true;
        }
        current = parents.get(&node).copied().flatten();
    }
    false
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
    import_library(
        existing,
        &result.library,
        result.warnings.clone(),
        backups,
        now,
    )
}

/// Decodes Kudzu JSON and merges it into `existing` after a `preImport` backup.
pub fn import_kudzu_json(
    existing: &KudzuLibrary,
    json: &str,
    backups: &BackupStore,
    now: DateTime<Utc>,
) -> Result<(KudzuLibrary, ImportSummary), InterchangeError> {
    let incoming = kudzu_json::decode(json)?;
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

/// Adds the "Getting Started" group and starter snippets (`;kudzu`, `;date`, `;sig`) unless a
/// snippet already uses `;kudzu`. Returns whether anything was added.
pub fn ensure_getting_started_snippets(library: &mut KudzuLibrary, now: DateTime<Utc>) -> bool {
    let has_sample = library.snippets.iter().any(|s| {
        s.triggers
            .iter()
            .any(|t| t.to_lowercase() == SAMPLE_TRIGGER)
    });
    if has_sample {
        return false;
    }
    let existing = library.groups.iter().find(|g| {
        g.parent_id.is_none() && g.name.to_lowercase() == GETTING_STARTED_GROUP.to_lowercase()
    });
    let group_id = match existing {
        Some(g) => g.id,
        None => {
            let next = library
                .groups
                .iter()
                .filter(|g| g.parent_id.is_none())
                .map(|g| g.sort_index)
                .max()
                .map_or(0, |m| m + 1);
            let group = GroupRecord {
                sort_index: next,
                ..GroupRecord::new(GETTING_STARTED_GROUP)
            };
            let id = group.id;
            library.groups.push(group);
            id
        }
    };
    let starters = [
        ("Kudzu Test", SAMPLE_TRIGGER, SAMPLE_EXPANSION),
        (
            "Today's Date",
            ";date",
            r#"{{date format="EEEE, d MMMM yyyy"}}"#,
        ),
        ("Signature", ";sig", "Best regards,\nYour Name"),
    ];
    for (name, trigger, content) in starters {
        library.snippets.push(SnippetRecord {
            group_id: Some(group_id),
            created_at: now,
            updated_at: now,
            ..SnippetRecord::new(name, vec![trigger.to_string()], content)
        });
    }
    true
}
