//! Espanso match-file import and export.
//!
//! See the Espanso mapping table in docs/TEMPLATE_SYNTAX.md for what converts and what becomes a
//! warning.

mod exporter;
mod importer;
pub(crate) mod tags;
mod yaml;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(test)]
use serde::{Deserialize, Serialize};

use super::InterchangeError;
use crate::library::model::KudzuLibrary;

/// A warning emitted during an Espanso import for an unsupported or degraded feature.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub struct ImportWarning {
    /// Path of the match file relative to the imported directory.
    pub file: String,
    /// 1-based line in `file`, when known.
    #[cfg_attr(test, serde(default, skip_serializing_if = "Option::is_none"))]
    pub line: Option<usize>,
    /// Index of the match in the file's `matches:` list, when the warning is about one match.
    #[cfg_attr(test, serde(default, skip_serializing_if = "Option::is_none"))]
    pub match_index: Option<usize>,
    #[cfg_attr(test, serde(default, skip_serializing_if = "Option::is_none"))]
    pub trigger: Option<String>,
    pub message: String,
}

/// The library built from Espanso files plus everything that didn't convert cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EspansoImportResult {
    pub library: KudzuLibrary,
    pub warnings: Vec<ImportWarning>,
}

/// Default Espanso config locations under `home`: macOS Application Support first, then
/// `~/.config/espanso`, plus `%APPDATA%\espanso` on Windows.
pub fn default_config_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![
        home.join("Library/Application Support/espanso"),
        home.join(".config/espanso"),
    ];
    if cfg!(windows) {
        let app_data =
            std::env::var_os("APPDATA").map_or_else(|| home.join("AppData/Roaming"), PathBuf::from);
        dirs.push(app_data.join("espanso"));
    }
    dirs
}

/// Imports an Espanso directory: either the config root containing `match/`, or a `match/` directory.
///
/// Every auto-loaded file (recursively, skipping `_`-prefixed and hidden entries) is read, and
/// `imports:` are followed. Each file becomes a group; subdirectories become parent groups.
pub fn import_dir(dir: &Path) -> Result<EspansoImportResult, InterchangeError> {
    let root = importer::resolve_match_root(dir);
    let files = importer::discover_match_files(&root)?;
    Ok(importer::perform_import(&files, Some(&root)))
}

/// Imports an explicit list of Espanso match files (and whatever they import).
pub fn import_files(files: &[PathBuf]) -> Result<EspansoImportResult, InterchangeError> {
    let base = importer::common_parent(files);
    Ok(importer::perform_import(files, base.as_deref()))
}

/// Converts `library` into Espanso match files: relative path (`base.yml`, `work/emails.yml`) → YAML.
///
/// Each group becomes a file nested by its parent groups; ungrouped snippets go to `base.yml`.
pub fn export(library: &KudzuLibrary) -> BTreeMap<String, String> {
    exporter::export(library)
}
