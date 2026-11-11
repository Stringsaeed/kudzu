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
/// Content-free completion failures. Adapters preserve these codes, including D9/D14.
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
    /// A synchronous flag-only operation: no I/O and no EngineEvents callback. Guard
    /// transitions serialize this call. Suppression must never disable form interruption.
    fn set_suppressed(&self, suppressed: bool);
    /// Runs on a worker. Recheck posting, pause, Secure Input and global exclusions at
    /// execution. RUL-9 bypasses snippet/group enablement and app eligibility, but keeps
    /// resolved insertion precedence. Success may only mean a form was opened: emit
    /// expanded exactly once after actual insertion, never on acceptance. Emit Activity
    /// outcomes separately, with no content. D14 recovery belongs to the engine.
    fn insert(
        &self,
        snippet: Uuid,
        target: Option<AppInfo>,
        selection: Option<String>,
    ) -> Result<(), InsertionError>;
}
pub trait FormChannel: Send + Sync {
    /// Atomically consumes the engine's matching token; rechecks interruption independently
    /// of suppression, reactivates/waits for the original target, then inserts. The UI must
    /// hide before invoking this. Interrupted typed forms must never delete the trigger.
    fn submit_form(
        &self,
        token: u64,
        values: HashMap<String, String>,
    ) -> Result<(), InsertionError>;
    fn cancel_form(&self, token: u64);
    /// Focus is independent of suppression: typing outside a form still interrupts it.
    fn panel_focused(&self, token: u64, focused: bool);
}
