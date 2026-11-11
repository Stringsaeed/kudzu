//! Import and restore: parse first, back up, then merge into the library as it is now.

use std::path::Path;

use kudzu_core::interchange::backup::{BackupInfo, BackupReason};
use kudzu_core::interchange::merge::merge_library;

use super::source::{detect_import_source, load_as};
use super::summary::{group_warnings, EspansoDetection, ImportSummary};
use super::{DataError, Inner};
use kudzu_core::interchange::espanso;

impl Inner {
    /// Imports a file or folder from any supported app. Nothing is touched until it has parsed
    /// and the `preImport` backup is on disk.
    pub(super) fn import(&self, path: &Path) -> Result<ImportSummary, DataError> {
        let source = detect_import_source(path)?;
        let parsed = load_as(path, source)?;
        let incoming = &parsed.library;
        let (backup, (dedupe_report, added_snippets, added_groups)) = self.run_backed_up(
            BackupReason::PreImport,
            |_| {},
            |library| {
                let (snippets, groups) = (library.snippets.len(), library.groups.len());
                let (merged, report) = merge_library(library, incoming);
                let added = (
                    merged.snippets.len().saturating_sub(snippets),
                    merged.groups.len().saturating_sub(groups),
                );
                *library = merged;
                (report, added.0, added.1)
            },
        )?;
        self.refresh_backups();
        Ok(ImportSummary {
            source,
            imported_snippets: incoming.snippets.len(),
            imported_groups: incoming.groups.len(),
            added_snippets,
            added_groups,
            backup,
            dedupe_report,
            grouped_warnings: group_warnings(&parsed.warnings),
            warnings: parsed.warnings,
        })
    }

    /// Replaces the library with a backup the store lists. The backup is read before anything
    /// else happens, then the current library is saved as a `preRestore` backup, so a restore
    /// can always be undone. Returns that safety backup.
    pub(super) fn restore(&self, requested: &BackupInfo) -> Result<BackupInfo, DataError> {
        // Only files the store lists can be restored, whatever path the caller holds.
        let info = self
            .store
            .list()?
            .into_iter()
            .find(|candidate| candidate.filename == requested.filename)
            .ok_or(DataError::BackupMissing)?;
        let restored = self.store.load(&info.path)?;
        let (safety, ()) = self.run_backed_up(
            BackupReason::PreRestore,
            |_| {},
            |library| *library = restored.clone(),
        )?;
        self.refresh_backups();
        Ok(safety)
    }

    /// Espanso config folders under the home folder that have at least one snippet.
    pub(super) fn detect_espanso(&self) -> Vec<EspansoDetection> {
        espanso::default_config_dirs(&self.home)
            .into_iter()
            .filter(|dir| dir.is_dir())
            .filter_map(|dir| {
                let result = espanso::import_dir(&dir).ok()?;
                Some(EspansoDetection {
                    path: dir,
                    snippet_count: result.library.snippets.len(),
                    group_count: result.library.groups.len(),
                })
            })
            .filter(|found| found.snippet_count > 0)
            .collect()
    }
}
