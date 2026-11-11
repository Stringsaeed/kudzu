//! An insertion started from one of Kudzu's windows (a submitted form, a Quick Search pick)
//! keeps typed triggers suppressed until the engine reports its outcome.
//!
//! Hiding the window gives the target back, and the engine then activates it and lets it
//! settle (and, for a typed form, deletes the trigger); it holds input only from then on. A
//! trigger typed before that would expand on its own, inside the insertion. So the window's
//! suppression guard moves here, and goes with the first of:
//! - the insertion's outcome: any final report for its snippet from its source, or D17's
//!   notice that its snippet's trigger was lost;
//! - its hand-off: a pick whose snippet opens a form, once the form holds its own guard;
//! - an admission error, or the work failing to start (no outcome will come);
//! - the engine stopping, or a newer engine starting (S40 resyncs the services after every
//!   start): nothing in flight on the old one will report;
//! - a deadline, so a report that never arrives can't leave triggers suppressed for good;
//! - its owner being dropped.
//!
//! The wakers on outcomes() and feed() only mark the holds they end and wake their threads;
//! each hold's thread drops its guard. Notices carry no token, so a hold is matched by source
//! and snippet, oldest first, among notices published after it began.
use super::{FeedState, InsertionOutcome, OutcomeNotice, SuppressionGuard};
use crate::watch::{Subscription, Watch};
use kudzu_core::activity::ActivitySource;
use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak},
    time::{Duration, Instant},
};
use uuid::Uuid;

/// How long an insertion may keep triggers suppressed without an outcome. The engine's own
/// bounds (activation, settling, its input hold, a paste's restore) add up to a few seconds at
/// worst, even behind another insertion.
pub(crate) const OUTCOME_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HoldId(u64);

struct Hold {
    id: HoldId,
    source: ActivitySource,
    snippet: Uuid,
    /// Outcome notices up to this sequence were published before the hold began.
    after: u64,
    /// The engine generation (`FeedState::generation`) it began under.
    generation: u64,
    /// Its end has come; its thread drops the guard.
    settled: bool,
    _guard: SuppressionGuard,
}

#[derive(Default)]
struct List {
    next: u64,
    holds: Vec<Hold>,
}

pub(crate) struct Holds {
    list: Mutex<List>,
    changed: Condvar,
    outcomes: Watch<Option<OutcomeNotice>>,
    feed: Watch<FeedState>,
    _wakers: [Subscription; 2],
}

impl Holds {
    /// Holds that end on `outcomes` and on a new engine generation in `feed`.
    pub(crate) fn new(outcomes: Watch<Option<OutcomeNotice>>, feed: Watch<FeedState>) -> Arc<Self> {
        Arc::new_cyclic(|holds: &Weak<Self>| {
            let ended = {
                let (holds, notices) = (holds.clone(), outcomes.clone());
                outcomes.subscribe(move || {
                    if let (Some(holds), Some(notice)) = (holds.upgrade(), *notices.get()) {
                        holds.mark(|list| match notice.outcome {
                            InsertionOutcome::EngineStopped => {
                                settle_all(list, |h| h.after < notice.sequence)
                            }
                            _ => list
                                .iter_mut()
                                .find(|h| !h.settled && h.ends_with(&notice))
                                .map(|h| h.settled = true)
                                .is_some(),
                        });
                    }
                })
            };
            let replaced = {
                let (holds, states) = (holds.clone(), feed.clone());
                feed.subscribe(move || {
                    let generation = states.get().generation;
                    if let Some(holds) = holds.upgrade() {
                        holds.mark(|list| settle_all(list, |h| h.generation < generation));
                    }
                })
            };
            Self {
                list: Mutex::default(),
                changed: Condvar::new(),
                outcomes,
                feed,
                _wakers: [ended, replaced],
            }
        })
    }

    /// Keeps `guard` until the outcome of the insertion of `snippet` from `source` that the
    /// caller is about to hand the engine, or `deadline` from now. If no thread can wait for
    /// it, the guard goes now: an unwanted expansion is better than triggers that never expand.
    pub(crate) fn hold(
        self: &Arc<Self>,
        source: ActivitySource,
        snippet: Uuid,
        guard: SuppressionGuard,
        deadline: Duration,
    ) -> HoldId {
        let (after, generation) = (self.outcomes.revision().0, self.feed.get().generation);
        let id = {
            let mut list = self.lock();
            let id = HoldId(list.next);
            list.next += 1;
            list.holds.push(Hold {
                id,
                source,
                snippet,
                after,
                generation,
                settled: false,
                _guard: guard,
            });
            id
        };
        let holds = self.clone();
        let started = std::thread::Builder::new()
            .name("kudzu-hold".into())
            .spawn(move || holds.wait(id, deadline));
        if let Err(error) = started {
            log::warn!("could not wait for an insertion's outcome: {error}");
            self.release(id);
        }
        id
    }

    fn wait(&self, id: HoldId, deadline: Duration) {
        let now = Instant::now();
        let end = now
            .checked_add(deadline)
            .unwrap_or_else(|| now + OUTCOME_DEADLINE);
        let mut list = self.lock();
        let hold = loop {
            let Some(index) = list.holds.iter().position(|h| h.id == id) else {
                return;
            };
            let left = end.saturating_duration_since(Instant::now());
            if list.holds[index].settled || left.is_zero() {
                if !list.holds[index].settled {
                    log::warn!("an insertion's outcome never arrived; triggers resume");
                }
                break list.holds.remove(index);
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

    /// Releases hold `id` now: its admission was refused or its work never started.
    pub(crate) fn release(&self, id: HoldId) {
        self.take(|h| h.id == id);
    }

    /// The oldest hold for `snippet` from `source` hands over to something that holds its own
    /// guard (a pick's form): it goes now.
    pub(crate) fn hand_off(&self, source: ActivitySource, snippet: Uuid) {
        self.take(|h| !h.settled && h.source == source && h.snippet == snippet);
    }

    /// Releases every hold (the owner is going away).
    pub(crate) fn clear(&self) {
        let holds = {
            let mut list = self.lock();
            self.changed.notify_all();
            std::mem::take(&mut list.holds)
        };
        drop(holds);
    }

    /// Removes the first hold matching `which` and drops its guard, with no lock held.
    fn take(&self, which: impl Fn(&Hold) -> bool) {
        let hold = {
            let mut list = self.lock();
            let index = list.holds.iter().position(which);
            self.changed.notify_all();
            index.map(|index| list.holds.remove(index))
        };
        drop(hold);
    }

    /// For the wakers: `settle` marks holds; their threads are woken if it marked any.
    fn mark(&self, settle: impl FnOnce(&mut [Hold]) -> bool) {
        let mut list = self.lock();
        if settle(&mut list.holds) {
            self.changed.notify_all();
        }
    }

    fn lock(&self) -> MutexGuard<'_, List> {
        self.list.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Marks every unsettled hold that `ends`; whether there was one.
fn settle_all(list: &mut [Hold], ends: impl Fn(&Hold) -> bool) -> bool {
    let mut any = false;
    for hold in list.iter_mut().filter(|h| !h.settled && ends(h)) {
        hold.settled = true;
        any = true;
    }
    any
}

impl Hold {
    fn ends_with(&self, notice: &OutcomeNotice) -> bool {
        use InsertionOutcome as O;
        let final_report = match notice.source {
            Some(source) => {
                source == self.source
                    && !matches!(
                        notice.outcome,
                        O::Undone | O::FormRejected { .. } | O::SubmitStartFailed { .. }
                    )
            }
            None => notice.outcome == O::TriggerLost,
        };
        final_report && notice.sequence > self.after && notice.snippet_id == Some(self.snippet)
    }
}
