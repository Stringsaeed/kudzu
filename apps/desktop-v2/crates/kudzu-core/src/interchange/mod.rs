//! Moving libraries between apps and formats.
//!
//! - [`espanso`]: Espanso match-file import and export.
//! - [`raycast`]: Raycast snippet JSON import.
//! - [`textexpander`]: TextExpander (`.textexpander` property list and CSV) import.
//! - `rich`: Markdown, HTML and RTF readers that turn rich sources into formatting spans.
//! - [`strftime`]: strftime ⇄ ICU pattern and offset conversion used by the Espanso bridge.
//! - [`detect`]: which app wrote an import file.
//! - [`merge`]: adding an imported library to an existing one.
//! - [`backup`]: backup file names, reasons, the daily rule and rotation. The backup folder and
//!   the import, restore and export orchestration are `kudzu_app::data`.
//!
//! The Kudzu JSON codec lives in `library::json`, dedupe in `library::dedupe` and usage
//! statistics in `library::usage`: they are rules of the library, not of a foreign format.
//!
//! The readers are the one place in this crate that touches the file system: walking an
//! Espanso directory is part of that format.

pub mod backup;
pub mod detect;
pub mod espanso;
pub mod merge;
pub mod raycast;
mod rich;
pub mod strftime;
mod template_out;
pub mod textexpander;

/// A library built from another app's export, plus everything that didn't convert cleanly.
pub type ImportResult = espanso::EspansoImportResult;

/// Errors from reading, writing or decoding interchange files.
#[derive(Debug, thiserror::Error)]
pub enum InterchangeError {
    /// The library was written by a newer Kudzu than this build understands.
    #[error("Unsupported Kudzu library format version {found}. Maximum supported version is {max_supported}.")]
    UnsupportedFormatVersion { found: i64, max_supported: i64 },
    /// A file from another app couldn't be read as the format it claims to be.
    #[error("{0}")]
    InvalidSource(String),
    /// The JSON was malformed or did not match the library shape.
    #[error("Failed to decode Kudzu library: {0}")]
    Decoding(String),
    /// Encoding failed (should not happen for well-formed libraries).
    #[error("Failed to encode Kudzu library: {0}")]
    Encoding(String),
    /// A filesystem operation failed.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
}

impl InterchangeError {
    pub(crate) fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}
