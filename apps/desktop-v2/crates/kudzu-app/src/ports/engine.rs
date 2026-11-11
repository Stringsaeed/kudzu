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
/// NSEvent's floating-point seconds, or the time a worker eventually runs. 0 means unknown.
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
/// The CGEvent behind NSApp.currentEvent, as the form window saw it. The key code tells it
/// apart from another event with the same stamp; with neither, the engine assumes the worst.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub stamp: EventStamp,
    /// kCGKeyboardEventKeycode for a key event; None for a click.
    pub key_code: Option<u16>,
}
/// One change of the form panel's key status, from the panel's key-status notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelFocus {
    /// The panel became key (true) or stopped being key.
    pub key: bool,
    /// 1 for the first change of this form's panel, then 2, 3, …; a gap interrupts a typed form.
    pub seq: u64,
    /// NSApp.currentEvent when the change happened; None when there was none. Never invented.
    pub after: Option<InputEvent>,
}
/// How a form was submitted. Maps to the native engine's `Submission`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FormSubmission {
    /// The submitting event (Return, the click on Insert); None counts any typing since the
    /// last focus report as after it.
    pub event: Option<InputEvent>,
    /// The `seq` of the panel's last key-status change before submitting (0 if none).
    pub focus_seq: u64,
}
/// One of Kudzu's own windows on screen, for judging clicks against a typed form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KudzuWindow {
    /// NSWindow.windowNumber.
    pub number: u32,
    /// The window ignores mouse events (an overlay), so clicks go to what is under it.
    pub passes_clicks_through: bool,
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
    /// Maps to native submit_form, which accepts on the calling thread and enqueues. Ok is
    /// acceptance only; EngineEvents reports the outcome later. Input after
    /// `submission.event`, and any focus report up to `submission.focus_seq` that has not
    /// arrived when the trigger would be deleted, interrupts a typed form: it deletes nothing.
    /// S57 fills both from NSApp.currentEvent, never from a delayed worker. Hide the UI first.
    fn submit_form(
        &self,
        token: FormToken,
        values: HashMap<String, String>,
        submission: FormSubmission,
    ) -> Result<(), AdmissionError>;
    /// Ignore tokens belonging to a retired EngineInstance.
    fn cancel_form(&self, token: FormToken);
    /// Every key-status change of the form panel, numbered from 1 per form. Reports may arrive
    /// late, even after submission, and still count. Ignore retired tokens.
    fn panel_focused(&self, token: FormToken, focus: PanelFocus);
}
/// The shell's narrow handle for telling the engine which windows are Kudzu's. Push the full
/// list whenever a Kudzu window opens, closes or changes click-through; until the first push
/// every click interrupts a typed form. Cheap and non-blocking; safe on the UI thread.
pub trait KudzuWindows: Send + Sync {
    fn set_kudzu_windows(&self, windows: &[KudzuWindow]);
}
