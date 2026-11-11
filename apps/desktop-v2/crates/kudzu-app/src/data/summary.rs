//! What an import tells the user, and what detection finds.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kudzu_core::interchange::backup::BackupInfo;
use kudzu_core::interchange::detect::ImportSource;
use kudzu_core::interchange::espanso::ImportWarning;
use kudzu_core::library::dedupe::DedupeReport;

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

/// A completed import, with its safety backup and dedupe details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSummary {
    pub source: ImportSource,
    /// Snippets in the source, before deduplication.
    pub imported_snippets: usize,
    /// Groups in the source, before deduplication.
    pub imported_groups: usize,
    /// Snippets the library gained once duplicates merged. Onboarding continues only on a gain.
    pub added_snippets: usize,
    pub added_groups: usize,
    /// The `preImport` backup of the library as it was before merging.
    pub backup: BackupInfo,
    pub dedupe_report: DedupeReport,
    pub warnings: Vec<ImportWarning>,
    pub grouped_warnings: Vec<WarningGroup>,
}

/// An Espanso config folder that has snippets in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspansoDetection {
    pub path: PathBuf,
    pub snippet_count: usize,
    pub group_count: usize,
}
