//! Completion signals for local windows. No trigger, output, form values or app names.
use super::ExpansionService;
use crate::{ports::FormToken, watch::Watch};
use kudzu_core::activity::{ActivityEvent, ActivityOutcome, ActivitySource, InsertFailure};
use std::sync::{Arc, PoisonError};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionOutcome {
    Expanded,
    Undone,
    Interrupted,
    Failed(InsertFailure),
    TriggerLost,
    Cancelled,
    Blocked,
    FormRejected { token: FormToken },
    SubmitStartFailed { token: FormToken },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutcomeNotice {
    pub sequence: u64,
    pub snippet_id: Option<Uuid>,
    pub source: Option<ActivitySource>,
    pub outcome: InsertionOutcome,
}
impl ExpansionService {
    /// S40 forwards every EngineEvents::activity report here as well as to ActivityService.
    /// D17's returned Activity event follows the same route. EngineEvents::expanded alone
    /// records usage; its accompanying Activity event publishes the UI completion.
    pub fn activity_outcome(&self, event: &ActivityEvent) {
        use ActivityOutcome as A;
        let outcome = match event.outcome {
            A::Expanded => InsertionOutcome::Expanded,
            A::Undone => InsertionOutcome::Undone,
            A::Interrupted => InsertionOutcome::Interrupted,
            A::InsertionFailed { error } => InsertionOutcome::Failed(error),
            A::TriggerLost => InsertionOutcome::TriggerLost,
            A::FormCancelled => InsertionOutcome::Cancelled,
            A::Paused
            | A::SnippetOff
            | A::GroupOff { .. }
            | A::AppRule { .. }
            | A::SecureInput { .. }
            | A::MissingPermission { .. } => InsertionOutcome::Blocked,
            A::TapDisabled { .. } | A::ListenerLost | A::EngineStopped => return,
        };
        self.publish_outcome(event.snippet_id, event.source, outcome);
    }
    pub fn outcomes(&self) -> Watch<Option<OutcomeNotice>> {
        self.outcomes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .watch()
    }
    pub(crate) fn publish_outcome(
        &self,
        snippet_id: Option<Uuid>,
        source: Option<ActivitySource>,
        outcome: InsertionOutcome,
    ) {
        let committed = {
            let state = self.outcomes.lock().unwrap_or_else(PoisonError::into_inner);
            let sequence = state
                .watch()
                .revision()
                .0
                .checked_add(1)
                .expect("outcome sequence exhausted");
            state.replace(Arc::new(Some(OutcomeNotice {
                sequence,
                snippet_id,
                source,
                outcome,
            })))
        };
        committed.notification.notify();
    }
}
