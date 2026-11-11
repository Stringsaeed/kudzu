//! Typed engine callbacks go to services, never directly to a window.
//! Every callback runs after engine locks have been released.
use crate::error::PortError;
use kudzu_core::{
    activity::{ActivityEvent, InsertFailure},
    library::model::{GroupRecord, SnippetRecord},
    matching::MatchingConfig,
    system::{AppInfo, EngineStatus, Rect},
    template::FormFieldSpec,
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
/// Content-free insertion rejection/failure codes. Adapters may reject synchronously;
/// native worker outcomes arrive later through EngineEvents, including D9/D14/D17.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InsertionError {
    #[error("Kudzu is paused.")]
    Paused,
    #[error("Secure Input is blocking expansion.")]
    SecureInput,
    #[error("This app is excluded from expansion.")]
    Excluded,
    #[error("That snippet no longer exists.")]
    MissingSnippet,
    #[error("Typing or a click interrupted the insertion. Your text was left alone.")]
    Interrupted,
    #[error("{}", failure_message(*.0))]
    Failed(InsertFailure),
    #[error(transparent)]
    Port(#[from] PortError),
}
fn failure_message(error: InsertFailure) -> &'static str {
    match error {
        InsertFailure::NotAllowed => "macOS refused the insertion.",
        InsertFailure::EventCreation => "macOS could not create the keystrokes.",
        InsertFailure::PasteUnavailable => {
            "The clipboard could not be used to paste this expansion."
        }
        InsertFailure::TargetGone => "The target app is no longer available.",
    }
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
    /// Nonzero and strictly increasing for the lifetime of this service/router, including engine restarts.
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
    ) -> Result<(), InsertionError>;
}
pub trait FormChannel: Send + Sync {
    /// Maps to native submit_form, which enqueues and returns. Ok is acceptance only;
    /// EngineEvents reports the outcome later. The worker consumes the matching token,
    /// rechecks interruption independently of suppression, activates/waits for the target,
    /// then inserts. Hide the UI before calling. An interrupted typed form deletes nothing.
    fn submit_form(
        &self,
        token: u64,
        values: HashMap<String, String>,
    ) -> Result<(), InsertionError>;
    fn cancel_form(&self, token: u64);
    /// Maps to native form_panel_focused. Report key-status changes on the main thread
    /// before the next panel event, and make it key on clicks inside. Native clicks are
    /// settled by the next focus report. Suppression never disables this interruption signal.
    fn panel_focused(&self, token: u64, focused: bool);
}
