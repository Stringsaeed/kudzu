use super::FormError;
use crate::{
    persist::{PersistError, Revision},
    ports::engine::AdmissionError,
    tasks::{Task, TaskError},
};
use std::fmt;

pub struct Submission {
    pub memory_revision: Revision,
    /// Queue errors only. Asynchronous disk errors are published through saved().
    pub memory_error: Option<PersistError>,
    /// Admission only. EngineEvents/Activity publish the final result through outcomes().
    pub admission: Result<(), AdmissionError>,
}

/// Worker-start boundary. On Err the operation must be dropped, never run or retained.
/// Production uses one Task thread; fixtures can refuse startup without exhausting OS threads.
pub trait SubmissionWorker: Send + Sync {
    fn spawn(
        &self,
        operation: Box<dyn FnOnce() -> Submission + Send>,
    ) -> Result<Task<Submission>, TaskError>;
}
pub(super) struct ThreadedSubmission;
impl SubmissionWorker for ThreadedSubmission {
    fn spawn(
        &self,
        operation: Box<dyn FnOnce() -> Submission + Send>,
    ) -> Result<Task<Submission>, TaskError> {
        Task::spawn("kudzu-form-submit", operation)
    }
}
/// Keep the restoration handle: S40 must settle it, and S57 can report a failed restoration
/// separately from the refused submission. Starting a second thread can itself fail too.
pub struct SubmitStartError {
    pub start: TaskError,
    pub restoration: Result<Task<Result<(), FormError>>, TaskError>,
}
impl fmt::Debug for SubmitStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubmitStartError")
            .field("start", &self.start)
            .field(
                "restoration_start",
                &self.restoration.as_ref().map(|_| ()).map_err(|e| *e),
            )
            .finish()
    }
}
impl fmt::Display for SubmitStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(f)
    }
}
impl std::error::Error for SubmitStartError {}
