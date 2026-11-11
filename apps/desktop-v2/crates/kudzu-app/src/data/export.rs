//! Writing the library out: Kudzu JSON and Espanso match files.

use std::fs;
use std::path::Path;

use kudzu_core::interchange::espanso;
use kudzu_core::library::json;
use kudzu_core::library::model::KudzuLibrary;

use super::DataError;

pub(super) fn write_json(library: &KudzuLibrary, path: &Path) -> Result<(), DataError> {
    let text = json::encode(library).map_err(|_| DataError::Encode)?;
    fs::write(path, text).map_err(|e| DataError::Write(e.into()))
}

/// Writes `library` as Espanso match files under `dir`; returns how many files were written.
pub(super) fn write_espanso(library: &KudzuLibrary, dir: &Path) -> Result<usize, DataError> {
    let mut written = 0;
    for (relative, yaml) in espanso::export(library) {
        let path = dir.join(&relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| DataError::Write(e.into()))?;
        }
        fs::write(&path, yaml).map_err(|e| DataError::Write(e.into()))?;
        written += 1;
    }
    Ok(written)
}
