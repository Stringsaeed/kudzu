//! Whether a typed form may still delete its trigger (FRM-4, D9).
//!
//! A typed trigger stays in the target's document while its form is open. Submitting deletes it
//! with Backspaces from wherever the caret is then, which is only right if nothing reached the
//! target in between. The listener sees every key-down and click before the app does. For a
//! click it also sees where it lands: the window under the pointer, and so the process that
//! owns it. For a key-down it only knows where the keyboard is from the form panel's focus
//! reports.
//!
//! The rules, conservative wherever a destination is unknown:
//! - A click interrupts unless it lands in one of Kudzu's own windows (the form panel, another
//!   Kudzu panel). A click in the target, in another app, on the menu bar, or anywhere the
//!   window server doesn't name, interrupts at once. No later focus report changes that.
//! - A key-down while the panel is not key reaches the target: interrupted. One while it is
//!   key counts as typed in the panel, until a later report says otherwise: each report and
//!   the submission carry the stamp of the last event the app had taken when it happened, and a
//!   key-down stamped later was routed after it. If the panel had lost key by then, or the
//!   form was already submitted, that key-down interrupts. A loss reported without a stamp
//!   counts against every key-down since the last report.
//! - After the submission every key-down and click interrupts until the trigger is deleted, and
//!   focus reports no longer count (the panel is going away).
//! - Focus moving without input (a shortcut opens another panel, a panel closes) is not an
//!   interruption.
//! - An interruption is final.

use crate::EventStamp;

/// A pending typed form's view of the input it saw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Interruption {
    interrupted: bool,
    /// The panel is key, as last reported.
    key: bool,
    /// The latest key-down counted as typed in the panel since the last report.
    in_panel: Option<EventStamp>,
    /// Submitted, and the trigger not deleted yet.
    submitted: bool,
}

impl Interruption {
    /// A key-down stamped `stamp` reached the session (the target or the panel; the listener
    /// can't tell).
    pub fn key_down(&mut self, stamp: EventStamp) {
        if !self.key || self.submitted {
            self.interrupted = true;
        } else {
            self.in_panel = self.in_panel.max(Some(stamp));
        }
    }

    /// Whether a key-down counted as typed in the panel came after `boundary` (or, without
    /// one, at all).
    fn typed_after(&self, boundary: Option<EventStamp>) -> bool {
        self.in_panel
            .is_some_and(|typed| boundary.is_none_or(|boundary| typed > boundary))
    }

    /// A mouse-down reached the session; `on_kudzu` when the window under the pointer is one of
    /// Kudzu's own.
    pub fn click(&mut self, on_kudzu: bool) {
        if self.submitted || !on_kudzu {
            self.interrupted = true;
        }
    }

    /// The panel reported that it became key (`true`) or stopped being key, after the events up
    /// to `after`.
    pub fn focus(&mut self, key: bool, after: Option<EventStamp>) {
        if self.submitted {
            return;
        }
        if !key && self.typed_after(after) {
            self.interrupted = true;
        }
        self.key = key;
        self.in_panel = None;
    }

    /// The form was submitted from its panel by the event stamped `at`.
    pub fn submit(&mut self, at: EventStamp) {
        if self.typed_after(Some(at)) {
            self.interrupted = true;
        }
        self.submitted = true;
    }

    /// Whether the form was submitted (its trigger isn't deleted yet).
    pub fn submitted(&self) -> bool {
        self.submitted
    }

    /// Something may have changed the document where the trigger is.
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(n: u64) -> EventStamp {
        EventStamp(n)
    }

    /// A form whose panel became key after event 1.
    fn key() -> Interruption {
        let mut form = Interruption::default();
        form.focus(true, Some(at(1)));
        form
    }

    #[test]
    fn typing_and_clicking_in_kudzu_windows_is_not_an_interruption() {
        let mut form = key();
        form.click(true);
        form.key_down(at(2));
        form.focus(false, Some(at(3)));
        form.click(true);
        form.focus(true, Some(at(4)));
        form.key_down(at(5));
        form.submit(at(6));
        assert!(!form.interrupted());
    }

    #[test]
    fn a_key_down_while_not_key_interrupts() {
        let mut form = key();
        form.focus(false, Some(at(2)));
        assert!(
            !form.interrupted(),
            "losing focus alone isn't an interruption"
        );
        form.key_down(at(3));
        assert!(form.interrupted());
    }

    #[test]
    fn a_click_outside_kudzu_interrupts_whatever_focus_does() {
        for (before, after) in [(true, true), (true, false), (false, true), (false, false)] {
            let mut form = Interruption::default();
            form.focus(before, Some(at(1)));
            form.click(false);
            form.focus(after, Some(at(2)));
            assert!(form.interrupted(), "key {before} then {after}");
        }
    }

    #[test]
    fn a_click_before_the_panel_was_ever_key_is_judged_by_where_it_lands() {
        let mut on_kudzu = Interruption::default();
        on_kudzu.click(true);
        assert!(!on_kudzu.interrupted());
        let mut elsewhere = Interruption::default();
        elsewhere.click(false);
        assert!(elsewhere.interrupted());
    }

    #[test]
    fn a_late_loss_of_key_catches_the_typing_routed_after_it() {
        // Key-downs 2 and 3 came while the panel still counted as key; it had lost key after 2.
        let mut form = key();
        form.key_down(at(2));
        form.key_down(at(3));
        form.focus(false, Some(at(2)));
        assert!(form.interrupted());
        // Lost after the last key-down: that typing was in the panel.
        let mut form = key();
        form.key_down(at(2));
        form.focus(false, Some(at(2)));
        assert!(!form.interrupted());
    }

    #[test]
    fn a_loss_of_key_without_a_stamp_counts_against_any_typing_since_the_last_report() {
        let mut typed = key();
        typed.key_down(at(2));
        typed.focus(false, None);
        assert!(typed.interrupted());
        let mut quiet = key();
        quiet.focus(false, None);
        assert!(!quiet.interrupted());
    }

    #[test]
    fn typing_after_the_submit_event_interrupts() {
        let mut form = key();
        form.key_down(at(2));
        form.submit(at(2));
        assert!(
            !form.interrupted(),
            "the submitting Return itself is in the panel"
        );
        let mut form = key();
        form.key_down(at(2));
        form.key_down(at(3));
        form.submit(at(2));
        assert!(form.interrupted());
    }

    #[test]
    fn after_the_submission_any_key_or_click_interrupts_and_focus_no_longer_counts() {
        let then: [fn(&mut Interruption); 2] = [|f| f.key_down(at(9)), |f| f.click(true)];
        for then in then {
            let mut form = key();
            form.submit(at(2));
            form.focus(false, Some(at(3)));
            assert!(!form.interrupted(), "the panel closing is expected");
            form.focus(true, Some(at(4)));
            then(&mut form);
            assert!(form.interrupted());
        }
    }

    #[test]
    fn an_interruption_is_final() {
        let mut form = key();
        form.click(false);
        form.focus(true, Some(at(2)));
        form.click(true);
        form.submit(at(3));
        assert!(form.interrupted());
    }
}
