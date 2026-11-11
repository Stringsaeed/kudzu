//! Pending forms belong to this service, not to the panel. Tokens are consumed exactly once.
mod memory;
use crate::{
    expansion::{ExpansionService, SuppressionGuard},
    persist::{PersistError, Persisted, Revision, SaveState},
    ports::{engine::InsertionError, FormChannel, FormRequest as EngineRequest, TargetApp},
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
use uuid::Uuid;

/// Local product data only. Deliberately has neither Debug nor serialization.
#[derive(Clone, PartialEq)]
pub struct FormRequest {
    pub token: u64,
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
pub struct Submission {
    pub memory_revision: Revision,
    /// Queue errors only. Asynchronous disk errors are published through saved().
    pub memory_error: Option<PersistError>,
    pub insertion: Result<(), InsertionError>,
}
struct Pending {
    request: Arc<FormRequest>,
    guard: SuppressionGuard,
}
struct State {
    pending: Option<Pending>,
    last_token: u64,
    output: Publisher<Option<Arc<FormRequest>>>,
}
pub struct FormService {
    state: Mutex<State>,
    memory: Arc<memory::Memory>,
    expansion: Arc<ExpansionService>,
    channel: Arc<dyn FormChannel>,
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
                last_token: 0,
                output: Publisher::new(None),
            }),
            memory: Arc::new(memory::Memory::new(initial, persisted, library)?),
            expansion,
            channel,
            target,
        })
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
    pub fn saved(&self) -> Watch<SaveState> {
        self.memory.saved()
    }
    /// EngineRouter calls this. Tokens must be nonzero, monotonically increasing across restarts.
    pub fn requested(&self, request: EngineRequest) -> bool {
        if request.token
            <= self
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .last_token
        {
            return false;
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
                return false;
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
    fn take(&self, token: Option<u64>) -> Option<Pending> {
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
    pub fn submit(
        &self,
        token: u64,
        values: HashMap<String, String>,
    ) -> Result<Option<Task<Submission>>, TaskError> {
        let Some(pending) = self.take(Some(token)) else {
            return Ok(None);
        };
        let (memory_revision, memory_error) = self
            .memory
            .record(pending.request.snippet_id, values.clone());
        drop(pending.guard);
        let channel = self.channel.clone();
        match Task::spawn("kudzu-form-submit", move || Submission {
            memory_revision,
            memory_error,
            insertion: channel.submit_form(token, values),
        }) {
            Ok(task) => Ok(Some(task)),
            Err(error) => {
                self.channel.cancel_form(token);
                Err(error)
            }
        }
    }
    pub fn cancel(&self, token: u64) -> Result<Option<Task<Result<(), FormError>>>, TaskError> {
        let Some(pending) = self.take(Some(token)) else {
            return Ok(None);
        };
        self.cancel_pending(pending).map(Some)
    }
    fn cancel_pending(&self, pending: Pending) -> Result<Task<Result<(), FormError>>, TaskError> {
        self.channel.cancel_form(pending.request.token);
        drop(pending.guard);
        let target = self.target.clone();
        Task::spawn("kudzu-form-cancel", move || {
            if let Some(app) = &pending.request.target {
                target.activate(app)?;
            }
            Ok(())
        })
    }
    /// Every shell close-windows path calls this, even when the form panel is hidden.
    pub fn close_windows(&self) -> Result<Option<Task<Result<(), FormError>>>, TaskError> {
        match self.take(None) {
            Some(pending) => self.cancel_pending(pending).map(Some),
            None => Ok(None),
        }
    }
    pub fn preview(
        &self,
        token: u64,
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
    pub fn panel_focused(&self, token: u64, focused: bool) {
        if self
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .is_some_and(|p| p.token == token)
        {
            self.channel.panel_focused(token, focused);
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
