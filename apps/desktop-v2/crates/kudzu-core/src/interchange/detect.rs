//! Which app wrote an import file, from its extension and, for JSON, its shape.

use super::InterchangeError;

/// Which app wrote an import file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSource {
    Kudzu,
    Espanso,
    Raycast,
    TextExpander,
}

/// Detects a file format from its extension (without the dot) and text contents.
/// Only JSON needs contents: a leading array is a Raycast export, otherwise a Kudzu library.
/// The caller handles directories and reading files.
pub fn file_format(extension: &str, contents: &str) -> Result<ImportSource, InterchangeError> {
    let extension = extension.to_lowercase();
    match extension.as_str() {
        "yml" | "yaml" => Ok(ImportSource::Espanso),
        "textexpander" | "csv" | "plist" | "xml" => Ok(ImportSource::TextExpander),
        "json" => {
            let first = contents
                .trim_start_matches('\u{feff}')
                .trim_start()
                .chars()
                .next();
            Ok(if first == Some('[') {
                ImportSource::Raycast
            } else {
                ImportSource::Kudzu
            })
        }
        _ => Err(InterchangeError::InvalidSource(
            "Kudzu can import .json (Kudzu or Raycast), .yml (Espanso), and .textexpander or .csv (TextExpander) files."
                .into(),
        )),
    }
}
