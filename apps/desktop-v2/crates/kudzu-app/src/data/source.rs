//! Working out what an import file is and reading it. Nothing here touches the library.

use std::fs;
use std::path::Path;

use kudzu_core::interchange::detect::{self, ImportSource};
use kudzu_core::interchange::{espanso, raycast, textexpander, ImportResult};
use kudzu_core::library::json;

use super::DataError;

/// Works out what `path` is from its extension and, for JSON, its contents.
/// Folders are Espanso config directories.
pub fn detect_import_source(path: &Path) -> Result<ImportSource, DataError> {
    if path.is_dir() {
        return Ok(ImportSource::Espanso);
    }
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    let contents = if extension == "json" {
        fs::read_to_string(path).map_err(|e| DataError::Read(e.into()))?
    } else {
        String::new()
    };
    detect::file_format(&extension, &contents).map_err(|_| DataError::UnsupportedFile)
}

/// Reads and converts the import file at `path`, whichever app wrote it. Nothing is merged yet.
pub fn load_import(path: &Path) -> Result<ImportResult, DataError> {
    load_as(path, detect_import_source(path)?)
}

pub(super) fn load_as(path: &Path, source: ImportSource) -> Result<ImportResult, DataError> {
    let converted = match source {
        ImportSource::Kudzu => {
            let text = fs::read_to_string(path).map_err(|e| DataError::Read(e.into()))?;
            json::decode(&text).map(|library| ImportResult {
                library,
                warnings: vec![],
            })
        }
        ImportSource::Espanso if path.is_dir() => espanso::import_dir(path),
        ImportSource::Espanso => espanso::import_files(&[path.to_path_buf()]),
        ImportSource::Raycast => raycast::import_file(path),
        ImportSource::TextExpander => textexpander::import_file(path),
    };
    converted.map_err(|error| DataError::from_core(error, format_name(source)))
}

pub(super) fn format_name(source: ImportSource) -> &'static str {
    match source {
        ImportSource::Kudzu => "Kudzu library",
        ImportSource::Espanso => "Espanso",
        ImportSource::Raycast => "Raycast snippet export",
        ImportSource::TextExpander => "TextExpander",
    }
}
