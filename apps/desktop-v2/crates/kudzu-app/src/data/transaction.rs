//! The one rule for changing the library from a file: back up first, then change.

use kudzu_core::interchange::backup::{BackupInfo, BackupReason};
use kudzu_core::library::model::KudzuLibrary;

use super::{DataError, Inner};

/// How many times a change is started over when something else edits the library meanwhile.
const ATTEMPTS: usize = 3;

impl Inner {
    /// Backs up the library, then applies `apply` to it inside a library transaction.
    ///
    /// The backup is of the exact library the change is applied to. If anything else commits
    /// between the backup and the transaction (an agent edit, a sync merge, the user), the
    /// backup is discarded and the whole step starts over against the new state, so the change
    /// always lands on top of the current library and the backup always matches what it replaced.
    /// A failed backup stops everything before any change is made.
    ///
    /// `between` runs after each backup and before its transaction; tests use it to edit the
    /// library at that moment.
    pub(super) fn run_backed_up<T>(
        &self,
        reason: BackupReason,
        mut between: impl FnMut(usize),
        apply: impl Fn(&mut KudzuLibrary) -> T,
    ) -> Result<(BackupInfo, T), DataError> {
        for attempt in 0..ATTEMPTS {
            let (_, snapshot) = self.library.snapshot();
            let backup = self
                .store
                .write(&snapshot, reason, self.now())
                .map_err(|error| match error {
                    DataError::BackupWrite(cause) => DataError::BackupRequired(cause),
                    other => other,
                })?;
            between(attempt);
            let (applied, _) = self.library.transact(|library| {
                if *library != *snapshot {
                    return Ok(None);
                }
                Ok(Some(apply(library)))
            })?;
            match applied {
                Some(value) => return Ok((backup, value)),
                None => self.store.discard(&backup),
            }
        }
        Err(DataError::LibraryBusy)
    }
}
