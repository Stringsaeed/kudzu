//! Which outcome notices belong to an insert this palette started.
//!
//! The baseline is the sequence read *before* `session.insert`. A notice is new when its
//! sequence is greater. `EngineStopped` carries no source and no snippet, so it is matched
//! on its own and is not dropped by the Quick Search filter.

use kudzu_app::expansion::InsertionOutcome;
use kudzu_app::expansion::OutcomeNotice;
use kudzu_core::activity::ActivitySource;
use uuid::Uuid;

/// What `poll` should do with one notice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// Already seen, another source, or a different snippet.
    Ignore,
    /// The engine stopped. Nothing admitted before this notice will report.
    EngineStopped,
    /// This palette's insert finished. The hold has already settled.
    Settled(InsertionOutcome),
}

/// `pending` is the snippet id passed to `insert`.
pub fn classify(notice: &OutcomeNotice, baseline: u64, pending: Option<Uuid>) -> NoticeKind {
    if notice.sequence <= baseline {
        return NoticeKind::Ignore;
    }
    if notice.outcome == InsertionOutcome::EngineStopped {
        return NoticeKind::EngineStopped;
    }
    let ours = notice.source == Some(ActivitySource::QuickSearch)
        && notice.snippet_id.is_some()
        && notice.snippet_id == pending;
    if ours {
        NoticeKind::Settled(notice.outcome)
    } else {
        NoticeKind::Ignore
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notice(
        sequence: u64,
        snippet: Option<Uuid>,
        source: Option<ActivitySource>,
        outcome: InsertionOutcome,
    ) -> OutcomeNotice {
        OutcomeNotice {
            sequence,
            snippet_id: snippet,
            source,
            outcome,
        }
    }

    #[test]
    fn a_quick_search_notice_after_the_baseline_settles_that_snippet() {
        let id = Uuid::from_u128(1);
        let n = notice(
            4,
            Some(id),
            Some(ActivitySource::QuickSearch),
            InsertionOutcome::Expanded,
        );
        assert_eq!(
            classify(&n, 3, Some(id)),
            NoticeKind::Settled(InsertionOutcome::Expanded)
        );
    }

    #[test]
    fn the_baseline_and_other_sources_are_ignored() {
        let id = Uuid::from_u128(1);
        let seen = notice(
            3,
            Some(id),
            Some(ActivitySource::QuickSearch),
            InsertionOutcome::Expanded,
        );
        assert_eq!(classify(&seen, 3, Some(id)), NoticeKind::Ignore);
        let typed = notice(
            5,
            Some(id),
            Some(ActivitySource::Typed),
            InsertionOutcome::Expanded,
        );
        assert_eq!(classify(&typed, 3, Some(id)), NoticeKind::Ignore);
        let other = notice(
            5,
            Some(Uuid::from_u128(2)),
            Some(ActivitySource::QuickSearch),
            InsertionOutcome::Expanded,
        );
        assert_eq!(classify(&other, 3, Some(id)), NoticeKind::Ignore);
    }

    #[test]
    fn engine_stopped_matches_with_no_source_and_no_snippet() {
        let n = notice(9, None, None, InsertionOutcome::EngineStopped);
        assert_eq!(
            classify(&n, 1, Some(Uuid::from_u128(1))),
            NoticeKind::EngineStopped
        );
        assert_eq!(classify(&n, 9, None), NoticeKind::Ignore);
    }
}
