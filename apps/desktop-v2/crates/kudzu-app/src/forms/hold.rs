//! A submitted form keeps typed triggers suppressed until the engine reports its outcome.
//!
//! Submitting hides the form, and the engine then activates the target, lets it settle and
//! deletes the form's trigger; it holds input only from then on. A trigger typed before that
//! would expand on its own, inside the form's insertion. So on submit the form's suppression
//! guard moves here, and goes with the first of:
//! - the form's outcome: any final report for its snippet as a form, or D17's notice that its
//!   trigger was lost;
//! - an admission error, or the submission failing to start (no outcome will come);
//! - a deadline, so a report that never arrives can't leave triggers suppressed for good;
//! - the service being dropped.
//!
//! The outcomes() waker reads each notice as it is published and only marks the hold it ends
//! and wakes the hold's thread. The thread drops the guard. Notices carry no token, so a hold
//! is matched by snippet, oldest first, among notices published after its submission.
use crate::{
    expansion::{InsertionOutcome, OutcomeNotice, SuppressionGuard},
    ports::FormToken,
    watch::{Subscription, Watch},
};
use kudzu_core::activity::ActivitySource;
use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};
use uuid::Uuid;

/// How long a submitted form may keep triggers suppressed without an outcome. The engine's
/// own bounds (activation, settling, its input hold, a paste's restore) add up to a few
/// seconds at worst, even behind another insertion.
pub(super) const OUTCOME_DEADLINE: Duration = Duration::from_secs(10);

struct Hold {
    token: FormToken,
    snippet: Uuid,
    /// Outcome notices up to this sequence were published before the submission.
    after: u64,
    /// Its outcome has arrived; its thread drops the guard.
    settled: bool,
    _guard: SuppressionGuard,
}

#[derive(Default)]
pub(super) struct Holds {
    holds: Mutex<Vec<Hold>>,
    changed: Condvar,
}

impl Holds {
    /// Watches `outcomes` for the rest of the service's life.
    pub(super) fn watch(self: &Arc<Self>, outcomes: Watch<Option<OutcomeNotice>>) -> Subscription {
        let holds = Arc::downgrade(self);
        let notices = outcomes.clone();
        outcomes.subscribe(move || {
            let (Some(holds), Some(notice)) = (holds.upgrade(), *notices.get()) else {
                return;
            };
            let mut list = holds.lock();
            if let Some(hold) = list.iter_mut().find(|h| !h.settled && h.ends_with(&notice)) {
                hold.settled = true;
                holds.changed.notify_all();
            }
        })
    }

    /// Keeps `guard` until form `token`'s outcome, or `deadline` from now. `after` is the
    /// outcome sequence before the submission. If no thread can wait for it, the guard goes
    /// now: an unwanted expansion is better than triggers that never expand again.
    pub(super) fn hold(
        self: &Arc<Self>,
        token: FormToken,
        snippet: Uuid,
        after: u64,
        guard: SuppressionGuard,
        deadline: Duration,
    ) {
        self.lock().push(Hold {
            token,
            snippet,
            after,
            settled: false,
            _guard: guard,
        });
        let holds = self.clone();
        let started = std::thread::Builder::new()
            .name("kudzu-form-hold".into())
            .spawn(move || holds.wait(token, deadline));
        if let Err(error) = started {
            log::warn!("could not wait for a submitted form's outcome: {error}");
            self.release(token);
        }
    }

    fn wait(&self, token: FormToken, deadline: Duration) {
        let now = Instant::now();
        let end = now
            .checked_add(deadline)
            .unwrap_or_else(|| now + OUTCOME_DEADLINE);
        let mut list = self.lock();
        let hold = loop {
            let Some(index) = list.iter().position(|h| h.token == token) else {
                return;
            };
            let left = end.saturating_duration_since(Instant::now());
            if list[index].settled || left.is_zero() {
                if !list[index].settled {
                    log::warn!("a submitted form's outcome never arrived; triggers resume");
                }
                break list.remove(index);
            }
            list = self
                .changed
                .wait_timeout(list, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        };
        drop(list);
        drop(hold);
    }

    /// Releases form `token`'s guard now: its admission was refused or it never started.
    pub(super) fn release(&self, token: FormToken) {
        let hold = {
            let mut list = self.lock();
            let index = list.iter().position(|h| h.token == token);
            self.changed.notify_all();
            index.map(|index| list.remove(index))
        };
        drop(hold);
    }

    /// Releases every guard (the service is going away).
    pub(super) fn clear(&self) {
        let holds = {
            let mut list = self.lock();
            self.changed.notify_all();
            std::mem::take(&mut *list)
        };
        drop(holds);
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Hold>> {
        self.holds.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Hold {
    fn ends_with(&self, notice: &OutcomeNotice) -> bool {
        use InsertionOutcome as O;
        let final_report = match notice.source {
            Some(ActivitySource::Form) => !matches!(
                notice.outcome,
                O::Undone | O::FormRejected { .. } | O::SubmitStartFailed { .. }
            ),
            None => notice.outcome == O::TriggerLost,
            Some(_) => false,
        };
        final_report && notice.sequence > self.after && notice.snippet_id == Some(self.snippet)
    }
}
