//! Pending forms belong to this service, not to the panel. Tokens are consumed exactly once.
mod memory;
mod submission;
use crate::{
    expansion::{ExpansionService, SuppressionGuard},
    persist::{PersistError, Persisted, Revision, SaveState},
    ports::{
        FormChannel, FormRequest as EngineRequest, FormSubmission, FormToken, PanelFocus, TargetApp,
    },
    tasks::{Task, TaskError},
    watch::{Publisher, Watch},
};
use kudzu_core::{
    library::model::KudzuLibrary,
    system::{AppInfo, Rect},
    template::{FormFieldSpec, FormMemory},
};
pub use memory::PruneState;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};
pub use submission::{Submission, SubmissionWorker, SubmitStartError};
use uuid::Uuid;

/// Local product data only. Deliberately has neither Debug nor serialization.
#[derive(Clone, PartialEq)]
pub struct FormRequest {
    pub token: FormToken,
    pub snippet_id: Uuid,
    pub snippet_name: String,
    pub fields: Vec<FormFieldSpec>,
    pub initial_values: HashMap<String, String>,
    pub target: Option<AppInfo>,
    pub caret: Option<Rect>,
    pub selection: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FormError {
    #[error("Form memory pruning could not start.")]
    Start(#[from] crate::error::IoFailure),
    #[error("Form memory pruning stopped unexpectedly.")]
    WorkerStopped,
    #[error(transparent)]
    Persistence(#[from] PersistError),
    #[error(transparent)]
    Target(#[from] crate::error::PortError),
}
struct Pending {
    request: Arc<FormRequest>,
    guard: SuppressionGuard,
}
struct State {
    pending: Option<Pending>,
    last_token: FormToken,
    output: Publisher<Option<Arc<FormRequest>>>,
}
pub struct FormService {
    state: Mutex<State>,
    memory: Arc<memory::Memory>,
    expansion: Arc<ExpansionService>,
    channel: Arc<dyn FormChannel>,
    submit_worker: Arc<dyn SubmissionWorker>,
    target: Arc<dyn TargetApp>,
}
impl FormService {
    /// Startup I/O only. This file is local product data and is never handed to sync or agents.
    pub fn open(
        path: std::path::PathBuf,
        writer: &crate::persist::Writer,
        library: Watch<KudzuLibrary>,
        expansion: Arc<ExpansionService>,
        channel: Arc<dyn FormChannel>,
        target: Arc<dyn TargetApp>,
    ) -> Result<Self, FormError> {
        let (file, loaded) = crate::persist::JsonFile::open(path, FormMemory::default)?;
        let durable = if loaded.source == crate::persist::LoadSource::Existing {
            Revision(1)
        } else {
            Revision(0)
        };
        Self::new(
            loaded.value,
            writer.register(file, durable)?,
            library,
            expansion,
            channel,
            target,
        )
    }

    pub fn new(
        initial: FormMemory,
        persisted: Persisted<FormMemory>,
        library: Watch<KudzuLibrary>,
        expansion: Arc<ExpansionService>,
        channel: Arc<dyn FormChannel>,
        target: Arc<dyn TargetApp>,
    ) -> Result<Self, FormError> {
        Ok(Self {
            state: Mutex::new(State {
                pending: None,
                last_token: FormToken::default(),
                output: Publisher::new(None),
            }),
            memory: Arc::new(memory::Memory::new(initial, persisted, library)?),
            expansion,
            channel,
            submit_worker: Arc::new(submission::ThreadedSubmission),
            target,
        })
    }
    pub fn with_submission_worker(mut self, worker: Arc<dyn SubmissionWorker>) -> Self {
        self.submit_worker = worker;
        self
    }
    pub fn pending(&self) -> Watch<Option<Arc<FormRequest>>> {
        self.state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .output
            .watch()
    }
    pub fn pruned(&self) -> Watch<PruneState> {
        self.memory.pruned()
    }
    pub fn stop_pruning(&self) {
        self.memory.stop();
    }
    /// Shared content-free completions; filter by source/snippet and observe sequence changes.
    pub fn outcomes(&self) -> Watch<Option<crate::expansion::OutcomeNotice>> {
        self.expansion.outcomes()
    }
    pub fn saved(&self) -> Watch<SaveState> {
        self.memory.saved()
    }
    /// EngineRouter qualifies native tokens with its EngineInstance before forwarding here.
    pub fn requested(&self, request: EngineRequest) -> bool {
        let rejected = !request.token.is_valid()
            || request.token
                <= self
                    .state
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .last_token;
        if rejected {
            return self.reject_request(request.token, request.snippet_id);
        }
        let initial_values = self
            .memory
            .initial_values(request.snippet_id, &request.fields);
        let guard = self.expansion.suppress();
        let request = Arc::new(FormRequest {
            token: request.token,
            snippet_id: request.snippet_id,
            snippet_name: request.snippet_name,
            fields: request.fields,
            initial_values,
            target: request.target,
            caret: request.caret,
            selection: request.selection,
        });
        let (old, committed) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            if request.token <= state.last_token {
                drop(state);
                drop(guard);
                return self.reject_request(request.token, request.snippet_id);
            }
            state.last_token = request.token;
            let old = state.pending.replace(Pending {
                request: request.clone(),
                guard,
            });
            (old, state.output.replace(Arc::new(Some(request))))
        };
        if let Some(old) = old {
            self.channel.cancel_form(old.request.token);
            drop(old);
        }
        committed.notification.notify();
        true
    }
    fn reject_request(&self, token: FormToken, snippet: Uuid) -> bool {
        drop(self.take(Some(token)));
        self.channel.cancel_form(token);
        self.expansion.publish_outcome(
            Some(snippet),
            Some(kudzu_core::activity::ActivitySource::Form),
            crate::expansion::InsertionOutcome::FormRejected { token },
        );
        false
    }
    fn take(&self, token: Option<FormToken>) -> Option<Pending> {
        let (pending, committed) = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            let pending_token = state.pending.as_ref()?.request.token;
            if token.is_some_and(|token| token != pending_token) {
                return None;
            }
            let pending = state.pending.take();
            (pending, state.output.replace(Arc::new(None)))
        };
        committed.notification.notify();
        pending
    }
    /// Hide the panel before calling. Memory is accepted before engine submission; disk I/O
    /// never blocks insertion. EngineEvents::expanded, not this result, records usage.
    /// `submission` comes from NSApp.currentEvent in the submitting handler (S57).
    pub fn submit(
        &self,
        token: FormToken,
        values: HashMap<String, String>,
        submission: FormSubmission,
    ) -> Result<Option<Task<Submission>>, SubmitStartError> {
        let Some(pending) = self.take(Some(token)) else {
            return Ok(None);
        };
        let (memory_revision, memory_error) = self
            .memory
            .record(pending.request.snippet_id, values.clone());
        drop(pending.guard);
        let channel = self.channel.clone();
        match self.submit_worker.spawn(Box::new(move || {
            let admission = channel.submit_form(token, values, submission);
            if admission.is_err() {
                channel.cancel_form(token);
            }
            Submission {
                memory_revision,
                memory_error,
                admission,
            }
        })) {
            Ok(task) => Ok(Some(task)),
            Err(start) => {
                let snippet = pending.request.snippet_id;
                let restoration = self.cancel_request(pending.request, true);
                self.expansion.publish_outcome(
                    Some(snippet),
                    Some(kudzu_core::activity::ActivitySource::Form),
                    crate::expansion::InsertionOutcome::SubmitStartFailed { token },
                );
                Err(SubmitStartError { start, restoration })
            }
        }
    }
    pub fn cancel(
        &self,
        token: FormToken,
    ) -> Result<Option<Task<Result<(), FormError>>>, TaskError> {
        let Some(pending) = self.take(Some(token)) else {
            return Ok(None);
        };
        self.cancel_pending(pending, true).map(Some)
    }
    fn cancel_pending(
        &self,
        pending: Pending,
        restore_target: bool,
    ) -> Result<Task<Result<(), FormError>>, TaskError> {
        drop(pending.guard);
        self.cancel_request(pending.request, restore_target)
    }
    fn cancel_request(
        &self,
        request: Arc<FormRequest>,
        restore_target: bool,
    ) -> Result<Task<Result<(), FormError>>, TaskError> {
        self.channel.cancel_form(request.token);
        if !restore_target {
            return Ok(Task::done(Ok(())));
        }
        let target = self.target.clone();
        Task::spawn("kudzu-form-cancel", move || {
            if let Some(app) = &request.target {
                target.activate(app)?;
            }
            Ok(())
        })
    }
    /// Every shell close-windows path calls this, even when the form panel is hidden.
    pub fn close_windows(&self) -> Result<Option<Task<Result<(), FormError>>>, TaskError> {
        match self.take(None) {
            Some(pending) => self.cancel_pending(pending, false).map(Some),
            None => Ok(None),
        }
    }
    pub fn preview(
        &self,
        token: FormToken,
        values: HashMap<String, String>,
    ) -> Result<
        Option<Task<Result<kudzu_core::template::RenderResult, crate::expansion::ExpansionError>>>,
        TaskError,
    > {
        let snapshot = self.pending().get();
        let Some(request) = snapshot.as_ref().as_ref().filter(|p| p.token == token) else {
            return Ok(None);
        };
        self.expansion
            .preview_snippet(request.snippet_id, request.selection.clone(), values)
            .map(Some)
    }
    /// Forwards every key-status change of the latest form's panel, numbered from 1. That
    /// includes reports arriving after it was submitted (the panel resigning key as it hides):
    /// the engine still counts them against a typed form. A replaced form's reports are dropped.
    pub fn panel_focused(&self, token: FormToken, focus: PanelFocus) {
        let latest = token.is_valid()
            && self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_token
                == token;
        if latest {
            self.channel.panel_focused(token, focus);
        }
    }
}
impl Drop for FormService {
    fn drop(&mut self) {
        let state = self.state.get_mut().unwrap_or_else(PoisonError::into_inner);
        let pending = state.pending.take();
        let committed = state.output.replace(Arc::new(None));
        if let Some(pending) = pending {
            self.channel.cancel_form(pending.request.token);
            drop(pending);
        }
        committed.notification.notify();
    }
}
#[cfg(test)]
mod tests;
