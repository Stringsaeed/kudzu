//! Whether a typed form may still delete its trigger (FRM-4, D9).
//!
//! A typed trigger stays in the target's document while its form is open. Submitting deletes it
//! with Backspaces from wherever the caret is then, which is only right if nothing reached the
//! target in between. The listener sees every key-down and click before the app does. For a
//! click it also sees the window that will receive it, which it compares with Kudzu's own
//! windows. For a key-down it may see the process it goes to; otherwise only the form panel's
//! focus reports say where the keyboard is.
//!
//! The rules, conservative wherever a destination is unknown:
//! - A click interrupts unless it lands in one of Kudzu's own windows.
//! - A key-down that the session says goes to another process, or that comes while the panel
//!   is not key, interrupts. One while the panel is key counts as typed in the panel only
//!   provisionally: a later report of a loss of key with an earlier boundary interrupts it.
//! - Reports are numbered. A missing one interrupts. Reports the submission counted still
//!   count when they arrive after it; the trigger may only be deleted once every one has
//!   arrived, so a report that never comes fails closed.
//! - Events are told apart by stamp and key code. A key-down after the submitting event (or a
//!   report's boundary), one with the same stamp that isn't that event, and one with no stamp
//!   count as after it. A submission without its event counts every provisional key-down
//!   against it.
//! - After the submission every key-down interrupts until the trigger is deleted, and so does
//!   every click outside Kudzu's windows. A click in one (the second click of a double-click
//!   on Submit) can't move the target's caret, so it doesn't.
//! - An interruption is final.

use crate::{InputEvent, PanelFocus, Submission};

/// Provisional key-downs or reports beyond this many interrupt the form (memory stays bounded).
const LIMIT: usize = 4096;

/// Where a key-down falls relative to an event the app reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// It is that event, or came before it.
    Before,
    After,
    /// Unstamped, or stamped the same without being it.
    Unknown,
}

/// A key-down counted as typed in the panel until the reports settle where it went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Typed {
    /// 0: the event had no stamp.
    stamp: u64,
    key_code: u16,
    /// Reports already received when it came: they all describe changes before it.
    known: usize,
}

impl Typed {
    fn place(&self, event: Option<&InputEvent>) -> Place {
        let Some(event) = event.filter(|e| e.stamp.0 != 0 && self.stamp != 0) else {
            return Place::Unknown;
        };
        match self.stamp.cmp(&event.stamp.0) {
            std::cmp::Ordering::Less => Place::Before,
            std::cmp::Ordering::Greater => Place::After,
            std::cmp::Ordering::Equal if event.key_code == Some(self.key_code) => Place::Before,
            std::cmp::Ordering::Equal => Place::Unknown,
        }
    }
}

/// A pending typed form's view of the input it saw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Interruption {
    interrupted: bool,
    /// The panel is key, as the reports so far say.
    key: bool,
    /// The last report's number.
    seq: u64,
    /// The reports counted so far, in order: key status and boundary.
    reports: Vec<(bool, Option<InputEvent>)>,
    /// Key-downs counted as typed in the panel until the reports settle them.
    provisional: Vec<Typed>,
    /// The submission's count of reports, once submitted.
    submitted: Option<u64>,
}

impl Interruption {
    /// A key-down with `stamp` and `key_code` reached the session; `elsewhere` when the session
    /// said it goes to a process other than Kudzu.
    pub fn key_down(&mut self, stamp: u64, key_code: u16, elsewhere: bool) {
        if elsewhere || self.submitted.is_some() || !self.key || self.provisional.len() >= LIMIT {
            self.interrupted = true;
            return;
        }
        self.provisional.push(Typed {
            stamp,
            key_code,
            known: self.reports.len(),
        });
    }

    /// A mouse-down reached the session; `on_kudzu` when it lands in one of Kudzu's windows.
    /// That stays true after the submission: such a click can't reach the target.
    pub fn click(&mut self, on_kudzu: bool) {
        if !on_kudzu {
            self.interrupted = true;
        }
    }

    /// The panel reported a change of its key status.
    pub fn focus(&mut self, report: PanelFocus) {
        if report.seq <= self.seq {
            return;
        }
        if report.seq != self.seq + 1 || self.reports.len() >= LIMIT {
            self.interrupted = true;
        }
        self.seq = report.seq;
        // Changes after the submission's count are the panel going away.
        if self.submitted.is_none_or(|count| report.seq <= count) {
            self.reports.push((report.key, report.after));
            self.key = report.key;
        }
        self.settle();
    }

    /// The form was submitted. Typing after its event (or that can't be placed before it)
    /// wasn't in the form; two key-downs that both look like the event are ambiguous.
    pub fn submit(&mut self, submission: Submission) {
        let event = submission.event;
        let mut matches = 0;
        let mut after = false;
        for typed in &self.provisional {
            match typed.place(event.as_ref()) {
                Place::Before => {
                    let same = event.is_some_and(|e| {
                        e.stamp.0 == typed.stamp && e.key_code == Some(typed.key_code)
                    });
                    matches += usize::from(same);
                }
                Place::After | Place::Unknown => after = true,
            }
        }
        if after || matches > 1 {
            self.interrupted = true;
        }
        self.submitted = Some(submission.focus_seq);
        self.settle();
    }

    /// Decides the provisional key-downs the reports now place: each went where the last
    /// report before it says (to the panel if none came after it arrived). One a report can't
    /// place interrupts. Once every report the submission counted has arrived, all are decided.
    fn settle(&mut self) {
        let all = self.settled();
        let reports = &self.reports;
        let mut interrupted = false;
        self.provisional.retain(|typed| {
            let late = &reports[typed.known.min(reports.len())..];
            let places: Vec<Place> = late.iter().map(|(_, b)| typed.place(b.as_ref())).collect();
            let decided = all || places.contains(&Place::Before);
            if decided {
                let key = late
                    .iter()
                    .zip(&places)
                    .filter(|(_, place)| **place == Place::After)
                    .map(|((key, _), _)| *key)
                    .next_back()
                    .unwrap_or(true);
                interrupted |= places.contains(&Place::Unknown) || !key;
            }
            !decided
        });
        self.interrupted |= interrupted;
    }

    /// Submitted, and every report the submission counted has arrived.
    fn settled(&self) -> bool {
        self.submitted.is_some_and(|count| self.seq >= count)
    }

    /// Whether the form was submitted (its trigger isn't deleted yet).
    pub fn submitted(&self) -> bool {
        self.submitted.is_some()
    }

    /// Something may have changed the document where the trigger is.
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }

    /// Whether the trigger may be deleted now: submitted, not interrupted, and every report the
    /// submission counted has arrived.
    pub fn may_delete(&self) -> bool {
        !self.interrupted && self.settled() && self.provisional.is_empty()
    }
}

#[cfg(test)]
mod tests;
