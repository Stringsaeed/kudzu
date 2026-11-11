//! Typed engine callbacks go to services, never directly to a window.
//! Every callback runs after engine locks have been released.
use crate::error::PortError;
use kudzu_core::{
    activity::ActivityEvent,
    library::model::{GroupRecord, SnippetRecord},
    matching::MatchingConfig,
    system::{AppInfo, EngineStatus, Rect},
    template::FormFieldSpec,
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
#[derive(Clone, Default, PartialEq)]
pub struct EngineLibrary {
    pub snippets: Vec<SnippetRecord>,
    pub groups: Vec<GroupRecord>,
    pub config: MatchingConfig,
    pub locale: String,
}
#[derive(Clone, PartialEq)]
pub struct FormRequest {
    pub token: u64,
    pub snippet_id: Uuid,
    pub snippet_name: String,
    pub fields: Vec<FormFieldSpec>,
    pub target: Option<AppInfo>,
    pub caret: Option<Rect>,
    pub selection: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expansion {
    pub snippet_id: Uuid,
    pub chars_saved: i64,
}
pub trait EngineEvents: Send + Sync {
    fn status_changed(&self, status: EngineStatus);
    fn expanded(&self, expansion: Expansion);
    fn form_requested(&self, request: FormRequest);
    fn activity(&self, event: ActivityEvent);
    fn clipboard_history(&self, index: usize, current: Option<&str>) -> Option<String>;
}
pub trait ExpansionEngine: Send + Sync {
    fn start(&self, events: Arc<dyn EngineEvents>) -> Result<(), PortError>;
    fn stop(&self);
    fn reload(&self, library: EngineLibrary);
    fn status(&self) -> EngineStatus;
    fn set_paused(&self, paused: bool);
    fn set_suppressed(&self, suppressed: bool);
    fn insert(
        &self,
        snippet: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<(), PortError>;
}
pub trait FormChannel: Send + Sync {
    fn submit_form(&self, token: u64, values: HashMap<String, String>) -> Result<(), PortError>;
    fn cancel_form(&self, token: u64);
    /// Focus is independent of suppression: typing outside a form still interrupts it.
    fn panel_focused(&self, token: u64, focused: bool);
}
