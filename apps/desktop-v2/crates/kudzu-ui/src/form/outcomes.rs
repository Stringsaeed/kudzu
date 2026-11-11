//! Content-free completion notices. The form hides when pending clears; this only
//! acknowledges the engine's outcomes so an exhaustive match includes `EngineStopped`.
use kudzu_app::expansion::{InsertionOutcome, OutcomeNotice};
use kudzu_core::activity::ActivitySource;
use uuid::Uuid;

/// Whether this notice belongs to the form still on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ours,
    Ignore,
}

/// Filter by source and snippet. `EngineStopped` has neither; windows ignore it.
pub fn classify(notice: &OutcomeNotice, snippet: Option<Uuid>) -> Kind {
    acknowledge(notice);
    let ours = snippet.is_some()
        && notice.snippet_id == snippet
        && notice.source == Some(ActivitySource::Form);
    match notice.outcome {
        InsertionOutcome::EngineStopped => Kind::Ignore,
        InsertionOutcome::TriggerLost => Kind::Ignore,
        _ if ours => Kind::Ours,
        _ => Kind::Ignore,
    }
}

/// Exhaustive on `InsertionOutcome` so a new variant fails to compile here (S34 follow-up).
pub fn acknowledge(notice: &OutcomeNotice) {
    match notice.outcome {
        InsertionOutcome::Expanded
        | InsertionOutcome::Undone
        | InsertionOutcome::Interrupted
        | InsertionOutcome::Failed(_)
        | InsertionOutcome::TriggerLost
        | InsertionOutcome::Cancelled
        | InsertionOutcome::Blocked
        | InsertionOutcome::FormRejected { .. }
        | InsertionOutcome::SubmitStartFailed { .. }
        | InsertionOutcome::EngineStopped => {}
    }
}

/// The newest notice after `last_seq`, if the Watch moved.
pub fn next(notice: Option<&OutcomeNotice>, last_seq: u64) -> Option<OutcomeNotice> {
    notice.copied().filter(|notice| notice.sequence > last_seq)
}
