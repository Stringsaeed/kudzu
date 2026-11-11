//! Library state, transactions, persistence and conflict queries.

mod conflicts;
mod groups;
mod snippets;
mod storage;
mod usage;
pub use conflicts::ConflictSnapshot;
pub use groups::GroupDeletion;
pub use snippets::{Deleted, SnippetDraft};

use crate::{
    persist::{PersistError, Persisted, Revision, SaveState},
    ports::{Clock, IdSource},
    schedule::{ScheduleError, Scheduler},
    watch::{Publisher, Watch},
};
use kudzu_core::library::model::KudzuLibrary;
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LibraryError {
    #[error("Trigger conflicts could not be scheduled.")]
    Schedule(#[from] ScheduleError),
    #[error("The trigger conflict worker could not start.")]
    WorkerStart(#[from] crate::error::IoFailure),
    #[error("The trigger conflict worker stopped unexpectedly.")]
    WorkerStopped,
    #[error("That trigger is already in use.")]
    TriggerTaken,
    #[error("Group not found")]
    MissingGroup,
    #[error("A group can't be moved inside itself.")]
    GroupCycle,
    #[error("That snippet no longer exists.")]
    MissingSnippet,
    #[error("The library could not be saved or loaded.")]
    Persistence(#[from] PersistError),
}

/// The sole mutation boundary for snippets, groups and usage. Shares snapshots, never state locks.
pub struct LibraryService {
    state: Arc<Mutex<Publisher<KudzuLibrary>>>,
    conflicts: conflicts::ConflictWorker,
    persisted: Persisted<KudzuLibrary>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdSource>,
    device_id: String,
}

impl LibraryService {
    /// Register the target at 0 for unsaved input or 1 for an already durable snapshot.
    pub fn new(
        initial: KudzuLibrary,
        device_id: String,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdSource>,
        persisted: Persisted<KudzuLibrary>,
        scheduler: Arc<Scheduler>,
    ) -> Result<Self, LibraryError> {
        let initial = Arc::new(initial);
        let state = Arc::new(Mutex::new(Publisher::with_revision(
            initial.clone(),
            Revision(1),
        )));
        let conflicts = conflicts::ConflictWorker::new(state.clone(), scheduler)?;
        persisted.submit(Revision(1), initial.clone())?;
        Ok(Self {
            state,
            conflicts,
            persisted,
            clock,
            ids,
            device_id,
        })
    }

    pub fn watch(&self) -> Watch<KudzuLibrary> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watch()
    }

    pub fn snapshot(&self) -> (Revision, Arc<KudzuLibrary>) {
        self.watch().snapshot()
    }

    pub fn saved(&self) -> Watch<SaveState> {
        self.persisted.saved()
    }

    /// Debounced results carry the source revision; readers can detect a still-pending query.
    pub fn conflicts(&self) -> Watch<ConflictSnapshot> {
        self.conflicts.watch()
    }

    /// Run a pure closure on a working copy of the latest library, under one lock.
    /// No I/O or service calls inside the closure. Errors and panics leave the snapshot intact.
    /// Use this boundary for agent edits, import/restore and sync merges, including their checks.
    pub fn transact<T>(
        &self,
        edit: impl FnOnce(&mut KudzuLibrary) -> Result<T, LibraryError>,
    ) -> Result<(T, Revision), LibraryError> {
        let (result, revision, notification) = {
            let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let (revision, current) = state.watch().snapshot();
            let mut next = (*current).clone();
            let result = edit(&mut next)?;
            if next == *current {
                return Ok((result, revision));
            }
            let revision = Revision(
                revision
                    .0
                    .checked_add(1)
                    .expect("library revision exhausted"),
            );
            let next = Arc::new(next);
            // Reserve persistence before publishing, so a stopped writer cannot partially accept.
            self.persisted.submit(revision, next.clone())?;
            let committed = state.replace(next);
            (result, committed.revision, committed.notification)
        };
        notification.notify();
        Ok((result, revision))
    }
}

#[cfg(test)]
mod tests;
