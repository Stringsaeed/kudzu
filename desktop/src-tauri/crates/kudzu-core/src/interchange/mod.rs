//! Kudzu JSON, Espanso import/export, backups, dedupe and import helpers.
//!
//! - [`kudzu_json`]: the native library file format.
//! - [`espanso`]: Espanso match-file import and export.
//! - [`strftime`]: strftime ⇄ ICU pattern and offset conversion used by the Espanso bridge.
//! - [`backup`]: timestamped JSON snapshots on disk.
//! - [`dedupe`]: deterministic duplicate detection and merging.
//! - [`coordinator`]: import orchestration (backup, merge, dedupe), starter snippets.
//! - [`stats`]: usage totals, time saved and palette ranking.
//! - [`unique_name`]: collision-free names and triggers.

pub mod backup;
pub mod coordinator;
pub mod dedupe;
pub mod espanso;
pub mod kudzu_json;
pub mod stats;
pub mod strftime;
pub mod unique_name;

/// Errors from reading, writing or decoding interchange files.
#[derive(Debug, thiserror::Error)]
pub enum InterchangeError {
    /// The library was written by a newer Kudzu than this build understands.
    #[error("Unsupported Kudzu library format version {found}. Maximum supported version is {max_supported}.")]
    UnsupportedFormatVersion { found: i64, max_supported: i64 },
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
