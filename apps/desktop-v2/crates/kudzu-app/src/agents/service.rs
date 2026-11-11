//! Service-owned inbound adapter. All entry points that do I/O run on workers.
use super::{Access, RenderEnv, OFFLINE_MESSAGE};
use crate::{
    clipboard::{AgentItem, ClipboardService},
    library::LibraryService,
    ports::{Clock, IdSource},
    settings::Settings,
    watch::{Publisher, Watch},
};
use kudzu_core::{
    clipboard::query::{ClipFilter, ClipQuery},
    KudzuLibrary,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentConfig {
    pub socket: PathBuf,
    pub executable: PathBuf,
    pub home: PathBuf,
    /// Registration name selected by composition's identity (e.g. kudzu-next).
    pub name: String,
    pub args: Vec<String>,
    pub version: String,
    pub locale: String,
    pub bundled_skill_dir: Option<PathBuf>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AgentError {
    #[error("AI agent transport or setup failed.")]
    Io(#[from] crate::error::IoFailure),
}
impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.into())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentStatus {
    pub running: bool,
}

pub struct AgentsService {
    pub(super) config: AgentConfig,
    pub(super) host: Arc<ServiceHost>,
    running: Mutex<Option<super::socket::Running>>,
    status: Arc<Publisher<AgentStatus>>,
}
impl AgentsService {
    pub fn new(
        config: AgentConfig,
        library: Arc<LibraryService>,
        clipboard: Arc<ClipboardService>,
        settings: Watch<Settings>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdSource>,
    ) -> Self {
        let locale = config.locale.clone();
        let status = Arc::new(Publisher::new(AgentStatus { running: false }));
        Self {
            config,
            host: Arc::new(ServiceHost {
                library,
                clipboard,
                settings,
                clock,
                ids,
                locale,
                status: status.clone(),
                #[cfg(test)]
                before_reply: Mutex::new(None),
                #[cfg(test)]
                request_gate: Mutex::new(None),
                #[cfg(test)]
                render_override: None,
            }),
            running: Mutex::new(None),
            status,
        }
    }
    pub fn status(&self) -> Watch<AgentStatus> {
        self.status.watch()
    }
    /// S40 calls on its settings reactor, never the UI thread. Access is also checked per request.
    pub fn apply(&self) -> Result<(), AgentError> {
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if self.host.settings.get().agent_access {
            if running.as_ref().is_some_and(|r| r.is_finished()) {
                if let Some(r) = running.take() {
                    r.shutdown();
                }
            }
            if running.is_none() {
                *running = Some(super::socket::listen(
                    self.host.clone(),
                    self.config.socket.clone(),
                    self.config.version.clone(),
                )?);
            }
        } else if let Some(r) = running.take() {
            r.shutdown();
        }
        drop(running);
        Ok(())
    }
    pub fn shutdown(&self) {
        if let Some(r) = self
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
        {
            r.shutdown();
        }
        self.status
            .replace(Arc::new(AgentStatus { running: false }))
            .notification
            .notify();
    }
}
impl Drop for AgentsService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub(super) struct ServiceHost {
    pub library: Arc<LibraryService>,
    pub clipboard: Arc<ClipboardService>,
    pub settings: Watch<Settings>,
    pub clock: Arc<dyn Clock>,
    pub ids: Arc<dyn IdSource>,
    locale: String,
    pub status: Arc<Publisher<AgentStatus>>,
    #[cfg(test)]
    pub before_reply: Mutex<Option<std::sync::mpsc::Sender<std::thread::ThreadId>>>,
    #[cfg(test)]
    pub render_override: Option<RenderEnv>,
    #[cfg(test)]
    pub request_gate: Mutex<Option<RequestGate>>,
}
impl ServiceHost {
    pub(super) fn access(&self) -> Result<Access, String> {
        let s = self.settings.get();
        if !s.agent_access {
            return Err(OFFLINE_MESSAGE.into());
        }
        Ok(Access {
            clipboard: s.agent_clipboard && s.clipboard_enabled,
            write: s.agent_write,
            trigger_prefix: s.default_trigger_prefix.clone(),
        })
    }
    pub(super) fn library(&self) -> Result<KudzuLibrary, String> {
        self.access()?;
        Ok((*self.library.watch().get()).clone())
    }
    pub(super) fn clipboard(
        &self,
        filter: ClipFilter,
        query: &ClipQuery,
    ) -> Result<Vec<AgentItem>, String> {
        if !self.access()?.clipboard {
            return Err("Clipboard history isn't shared with agents.".into());
        }
        Ok(self.clipboard.agent_query(filter, query))
    }
    pub(super) fn clipboard_item(&self, id: Uuid) -> Result<Option<AgentItem>, String> {
        if !self.access()?.clipboard {
            return Err("Clipboard history isn't shared with agents.".into());
        }
        Ok(self.clipboard.agent_item(id))
    }
    pub(super) fn agent_history_text(&self, n: usize, current: Option<&str>) -> Option<String> {
        self.access().ok().filter(|a| a.clipboard)?;
        self.clipboard.agent_history_text(n, current)
    }
    pub(super) fn render_env(&self) -> RenderEnv {
        #[cfg(test)]
        if let Some(env) = &self.render_override {
            return env.clone();
        }
        // S28 resolves history slot zero only if it still matches the current clipboard.
        // Never substitute stale recorded text for an unrecorded/private live clipboard.
        let clipboard = self.agent_history_text(0, None);
        RenderEnv {
            now: chrono::DateTime::<chrono::Utc>::from(self.clock.now()).fixed_offset(),
            locale: self.locale.clone(),
            clipboard,
        }
    }
}

#[cfg(test)]
pub(super) struct RequestGate {
    pub started: std::sync::mpsc::Sender<()>,
    pub release: std::sync::mpsc::Receiver<()>,
}
