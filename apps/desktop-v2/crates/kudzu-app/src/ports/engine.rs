//! Typed engine callbacks go to services, never directly to a window.
//! Every callback runs after engine locks have been released.
use crate::{error::PortError, ports::FormToken};
use kudzu_core::{
    activity::ActivityEvent,
    library::model::{GroupRecord, SnippetRecord},
    matching::MatchingConfig,
    system::{AppInfo, EngineStatus, Rect},
    template::FormFieldSpec,
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
/// Opaque CGEvent timestamp. S57 captures CGEventGetTimestamp from the CGEvent of
/// NSApp.currentEvent in the submit/key-status handler. Do not substitute wall time,
/// NSEvent's floating-point seconds, or the time a worker eventually runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventStamp(u64);
impl EventStamp {
    pub fn from_cg_event_timestamp(value: u64) -> Self {
        Self(value)
    }
    pub fn cg_event_timestamp(self) -> u64 {
        self.0
    }
}
/// Admission only (D20). Expansion results are reported by EngineEvents/Activity later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionError {
    #[error("The expansion engine is not running.")]
    NoEngine,
    #[error("That form is no longer pending.")]
    StaleToken,
    #[error("The expansion engine is busy. Try again.")]
    Busy,
}
#[derive(Clone, Default, PartialEq)]
pub struct EngineLibrary {
    pub snippets: Vec<SnippetRecord>,
    pub groups: Vec<GroupRecord>,
    pub config: MatchingConfig,
    pub locale: String,
}
#[derive(Clone, PartialEq)]
pub struct FormRequest {
    /// Qualified by the forwarding host's EngineInstance; native tokens restart per Engine.
    pub token: FormToken,
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
/// A stored trigger the engine could not restore after a refused paste (D17).
/// The original clipboard has been restored. This is neither typed context nor snippet
/// output. Keep it in the local recovery path only: no Debug, serialization or logging.
#[derive(Clone, PartialEq, Eq)]
pub struct TriggerLost {
    pub snippet_id: Uuid,
    pub trigger: String,
}

pub trait EngineEvents: Send + Sync {
    fn status_changed(&self, status: EngineStatus);
    fn expanded(&self, expansion: Expansion);
    fn form_requested(&self, request: FormRequest);
    fn activity(&self, event: ActivityEvent);
    /// Forward to expansion.trigger_lost, then record its content-free Activity event.
    /// The raw report must never be logged, persisted, synced or forwarded to agents.
    fn trigger_lost(&self, lost: TriggerLost);
    fn clipboard_history(&self, index: usize, current: Option<&str>) -> Option<String>;
}
pub trait ExpansionEngine: Send + Sync {
    /// Installs a forwarding host's event sink and starts the native engine. Readiness is
    /// reported through status; a successful return does not prove a listener is attached.
    fn start(&self, events: Arc<dyn EngineEvents>) -> Result<(), PortError>;
    fn stop(&self);
    /// Native reload builds the index on the calling thread; keep this off the UI thread.
    fn reload(&self, library: EngineLibrary);
    fn status(&self) -> EngineStatus;
    fn set_paused(&self, paused: bool);
    /// Synchronous matching-state update (native: flag plus matcher reset), with no OS
    /// I/O or EngineEvents callback. Guard transitions serialize this call. Suppression
    /// must never disable form interruption.
    fn set_suppressed(&self, suppressed: bool);
    /// Admission only: NoEngine, StaleToken or Busy. Never return a runtime outcome.
    /// Maps to native insert_snippet_with_selection, which enqueues and returns. Ok means
    /// accepted, not inserted: final outcomes arrive through EngineEvents. On its worker,
    /// the engine rechecks posting, pause, Secure Input and global exclusions; RUL-9 bypasses
    /// snippet/group enablement and app eligibility while keeping insertion precedence.
    /// Emit expanded exactly once after insertion, never on acceptance/form opening.
    /// Keep typed failures in Activity; forward D17 through trigger_lost. Recovery is native.
    fn insert(
        &self,
        snippet: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<(), AdmissionError>;
}
pub trait FormChannel: Send + Sync {
    /// Admission only, never a final insertion outcome.
    /// Maps to native submit_form, which enqueues and returns. Ok is acceptance only;
    /// EngineEvents reports the outcome later. Native submission marks the supplied event
    /// boundary at admission; its worker consumes the matching token,
    /// rechecks interruption independently of suppression, activates/waits for the target,
    /// then inserts. `at` is the submitting event's CGEvent timestamp, captured by S57
    /// from NSApp.currentEvent, never a delayed worker timestamp. Hide the UI before calling.
    /// An interrupted typed form deletes nothing.
    fn submit_form(
        &self,
        token: FormToken,
        values: HashMap<String, String>,
        at: EventStamp,
    ) -> Result<(), AdmissionError>;
    /// Ignore tokens belonging to a retired EngineInstance.
    fn cancel_form(&self, token: FormToken);
    /// S57 passes CGEventGetTimestamp of NSApp.currentEvent's CGEvent as `after`, captured
    /// at the key-status change. S35 maps Some to form_panel_focused_after and None to the
    /// conservative form_panel_focused fallback. Never invent a stamp for a missing event.
    /// Native click targets are determined by the window server, independently of focus.
    fn panel_focused(&self, token: FormToken, focused: bool, after: Option<EventStamp>);
}
