//! Whether a typed form may still delete its trigger (FRM-4, D9).
//!
//! A typed trigger stays in the target's document while its form is open. Submitting deletes it
//! with Backspaces from wherever the caret is then, which is only right if nothing reached the
//! target in between. The listener sees every key-down and click, but not where it lands. The
//! form panel reports when it becomes key or stops being key: later than the listener sees the
//! event that caused it, and in the order the panel handled events.
//!
//! The rules, conservative wherever a destination is unknown:
//! - A key-down while the panel is not key reaches the target: interrupted.
//! - A click is settled by the next focus report. Made while the panel was key, it stayed in
//!   the panel unless the panel then lost key, in which case it may have moved the target's
//!   caret: interrupted. Made while the panel was not key, it was on the panel only if the panel
//!   became key next; a key-down, another click or the submission first means it went
//!   elsewhere: interrupted.
//! - Before the panel has ever been key, no click can be on it: interrupted.
//! - The submission comes from the panel, after the panel reported every focus change the
//!   events before it caused, so it settles a click made while the panel was key as having
//!   stayed.
//! - Focus moving without input (a shortcut opens another panel, a panel closes) is not an
//!   interruption.

/// A click the next focus report settles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Click {
    /// Made while the panel was key.
    WhileKey,
    /// Made while it wasn't.
    WhileAway,
}

/// A pending typed form's view of the input it saw.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Interruption {
    interrupted: bool,
    /// The panel is key, as last reported.
    key: bool,
    /// The panel has been key at least once.
    was_key: bool,
    click: Option<Click>,
}

impl Interruption {
    /// A key-down reached the session (the target or the panel; the listener can't tell).
    pub fn key_down(&mut self) {
        if self.click == Some(Click::WhileAway) {
            self.click = None;
        }
        if !self.key {
            self.interrupted = true;
        }
    }

    /// A mouse-down reached the session.
    pub fn click(&mut self) {
        if self.click == Some(Click::WhileAway) || !(self.key || self.was_key) {
            self.interrupted = true;
        }
        self.click = Some(if self.key {
            Click::WhileKey
        } else {
            Click::WhileAway
        });
    }

    /// The panel reported that it became key (`true`) or stopped being key.
    pub fn focus(&mut self, key: bool) {
        if self.click.take().is_some() && !key {
            self.interrupted = true;
        }
        self.key = key;
        self.was_key |= key;
    }

    /// The form was submitted from its panel.
    pub fn submit(&mut self) {
        if self.click.take() == Some(Click::WhileAway) {
            self.interrupted = true;
        }
    }

    /// Something may have changed the document where the trigger is.
    pub fn interrupted(&self) -> bool {
        self.interrupted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A form whose panel is key.
    fn key() -> Interruption {
        let mut form = Interruption::default();
        form.focus(true);
        form
    }

    #[test]
    fn typing_and_clicking_in_the_key_panel_is_not_an_interruption() {
        let mut form = key();
        form.click();
        form.key_down();
        form.click();
        form.key_down();
        form.submit();
        assert!(!form.interrupted());
    }

    #[test]
    fn a_key_down_while_not_key_interrupts() {
        let mut form = key();
        form.focus(false);
        assert!(
            !form.interrupted(),
            "losing focus alone isn't an interruption"
        );
        form.key_down();
        assert!(form.interrupted());
    }

    #[test]
    fn a_click_that_takes_focus_away_interrupts() {
        let mut form = key();
        form.click();
        form.focus(false);
        assert!(form.interrupted());
    }

    #[test]
    fn a_click_is_settled_by_a_late_report_even_after_typing() {
        // The report that the panel lost key arrives after the user typed in the target.
        let mut form = key();
        form.click();
        form.key_down();
        assert!(!form.interrupted());
        form.focus(false);
        assert!(form.interrupted());
    }

    #[test]
    fn the_click_that_makes_the_panel_key_is_on_the_panel() {
        let mut form = key();
        form.focus(false);
        form.click();
        form.focus(true);
        form.key_down();
        form.submit();
        assert!(!form.interrupted());
    }

    #[test]
    fn a_click_while_not_key_that_no_report_claims_went_elsewhere() {
        for then in [
            Interruption::key_down,
            Interruption::click,
            Interruption::submit,
        ] {
            let mut form = key();
            form.focus(false);
            form.click();
            then(&mut form);
            assert!(form.interrupted());
        }
        let mut form = key();
        form.focus(false);
        form.click();
        form.focus(false);
        assert!(form.interrupted());
    }

    #[test]
    fn a_click_before_the_panel_was_ever_key_interrupts() {
        let mut form = Interruption::default();
        form.click();
        form.focus(true);
        assert!(form.interrupted());
    }

    #[test]
    fn the_submission_settles_a_click_made_while_key() {
        let mut form = key();
        form.click();
        form.submit();
        // The panel closing after the submission is not that click leaving it.
        form.focus(false);
        assert!(!form.interrupted());
    }

    #[test]
    fn an_interruption_is_final() {
        let mut form = key();
        form.focus(false);
        form.key_down();
        form.focus(true);
        form.click();
        form.submit();
        assert!(form.interrupted());
    }
}
