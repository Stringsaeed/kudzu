//! D17 recovery is local and transient. Only IDs and fixed copy enter the notice Watch;
//! the stored trigger is released separately, once, after an explicit local UI action.
use super::ExpansionService;
use crate::{
    ports::TriggerLost,
    watch::{Publisher, Watch},
};
use kudzu_core::activity::{ActivityEvent, ActivityOutcome, TRIGGER_LOST_MESSAGE};
use std::sync::{Arc, PoisonError};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LostTriggerNotice {
    pub id: RecoveryId,
    pub snippet_id: Uuid,
}
impl LostTriggerNotice {
    pub fn message(&self) -> &'static str {
        TRIGGER_LOST_MESSAGE
    }
    pub fn action_label(&self) -> &'static str {
        "Retype trigger"
    }
}

/// The local UI may show this after the Retype action, so the user can retype in the
/// intended app. D17 supplies no target identity; never feed this to snippet insertion
/// or overwrite the restored clipboard. No Debug/Serialize: never log or send to agents.
pub struct RetypeTrigger(String);
impl RetypeTrigger {
    pub fn trigger(&self) -> &str {
        &self.0
    }
}

pub(super) struct Recovery {
    next_id: u64,
    pending: Option<(RecoveryId, RetypeTrigger)>,
    output: Publisher<Option<LostTriggerNotice>>,
}
impl Recovery {
    pub(super) fn new() -> Self {
        Self {
            next_id: 1,
            pending: None,
            output: Publisher::new(None),
        }
    }
}
impl ExpansionService {
    /// EngineRouter forwards D17 here and sends the returned event to ActivityService::record.
    /// The newest notice replaces the old one; old UI actions cannot take the new trigger.
    #[must_use = "record this content-free event in the in-memory Activity service"]
    pub fn trigger_lost(&self, lost: TriggerLost) -> ActivityEvent {
        let event = ActivityEvent {
            id: 0,
            at: self.renderer.environment.clock.now().into(),
            source: None,
            snippet_id: Some(lost.snippet_id),
            trigger: None,
            app: None,
            outcome: ActivityOutcome::TriggerLost,
        };
        let committed = {
            let mut state = self.recovery.lock().unwrap_or_else(PoisonError::into_inner);
            let id = RecoveryId(state.next_id);
            state.next_id = state.next_id.checked_add(1).expect("recovery id exhausted");
            state.pending = Some((id, RetypeTrigger(lost.trigger)));
            state.output.replace(Arc::new(Some(LostTriggerNotice {
                id,
                snippet_id: lost.snippet_id,
            })))
        };
        committed.notification.notify();
        event
    }
    pub fn lost_trigger_notice(&self) -> Watch<Option<LostTriggerNotice>> {
        self.recovery
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .output
            .watch()
    }
    /// Dismiss the notice and forget its private recovery payload without exposing it.
    pub fn dismiss_lost_trigger(&self, id: RecoveryId) -> bool {
        self.take_trigger_for_retyping(id).is_some()
    }
    /// Explicit local UI action, not automatic typing. Taking consumes the notice and the
    /// private payload together. The original stored trigger survives library edits/deletion.
    pub fn take_trigger_for_retyping(&self, id: RecoveryId) -> Option<RetypeTrigger> {
        let (trigger, committed) = {
            let mut state = self.recovery.lock().unwrap_or_else(PoisonError::into_inner);
            if state.pending.as_ref()?.0 != id {
                return None;
            }
            let (_, trigger) = state.pending.take()?;
            (trigger, state.output.replace(Arc::new(None)))
        };
        committed.notification.notify();
        Some(trigger)
    }
}
