//! Errors for import, export and backups. `Display` is the sentence the user sees. Nothing here
//! carries a path, a parser message or file contents, so nothing that reaches the log can leak
//! snippet text.

use kudzu_core::interchange::{raycast, textexpander, InterchangeError};

use crate::error::IoFailure;
use crate::library::LibraryError;
use crate::schedule::ScheduleError;
use crate::settings::SettingsError;
use crate::tasks::TaskError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DataError {
    #[error("Kudzu can import .json (Kudzu or Raycast), .yml (Espanso), and .textexpander or .csv (TextExpander) files.")]
    UnsupportedFile,
    /// The file is not what its type says. The name is the format, never anything from the file.
    #[error("This isn't a valid {0} file.")]
    Invalid(&'static str),
    /// A fixed instruction for a known mistake, such as exporting from TextExpander as CSV.
    #[error("{0}")]
    Advice(&'static str),
    #[error("Unsupported Kudzu library format version {found}. Maximum supported version is {max_supported}.")]
    UnsupportedVersion { found: i64, max_supported: i64 },
    #[error("Couldn't read that file or folder.")]
    Read(#[source] IoFailure),
    #[error("Couldn't write the export.")]
    Write(#[source] IoFailure),
    #[error("Couldn't prepare the library for saving.")]
    Encode,
    #[error("Couldn't write the backup.")]
    BackupWrite(#[source] IoFailure),
    /// A required safety backup failed, so nothing was changed.
    #[error("Couldn't back up your library first, so nothing was changed.")]
    BackupRequired(#[source] IoFailure),
    #[error("Couldn't list the backups.")]
    BackupList(#[source] IoFailure),
    #[error("That backup couldn't be found.")]
    BackupMissing,
    #[error("That backup couldn't be read.")]
    BackupUnreadable,
    #[error("Another import, backup or restore is still running.")]
    Busy,
    #[error("Your library kept changing. Try again.")]
    LibraryBusy,
    #[error(transparent)]
    Library(#[from] LibraryError),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(transparent)]
    Task(#[from] TaskError),
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
}

impl DataError {
    /// Maps a core error for a file of the named format. File contents and paths are dropped.
    pub(super) fn from_core(error: InterchangeError, format: &'static str) -> Self {
        match error {
            InterchangeError::UnsupportedFormatVersion {
                found,
                max_supported,
            } => Self::UnsupportedVersion {
                found,
                max_supported,
            },
            // Only fixed sentences pass through. Anything else may quote the file.
            InterchangeError::InvalidSource(message) => {
                advice(&message).map_or(Self::Invalid(format), Self::Advice)
            }
            InterchangeError::Decoding(_) => Self::Invalid(format),
            InterchangeError::Encoding(_) => Self::Encode,
            InterchangeError::Io { source, .. } => Self::Read(source.into()),
        }
    }
}

/// The path-free sentences core's readers use for known mistakes.
fn advice(message: &str) -> Option<&'static str> {
    [
        textexpander::FOLDER_MESSAGE,
        textexpander::BINARY_MESSAGE,
        textexpander::COMPRESSED_MESSAGE,
        textexpander::NO_SNIPPETS_MESSAGE,
        raycast::NOT_RAYCAST_MESSAGE,
    ]
    .into_iter()
    .find(|known| *known == message)
}
