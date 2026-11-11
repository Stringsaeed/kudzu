//! Where the user's keys and clicks go on the fake desktop, and the form panel's focus reports.
//!
//! Three destinations: the target app's text field, Kudzu's form panel, and another Kudzu
//! panel (Quick Search, the picker). Kudzu's panels don't activate Kudzu, so the target stays
//! frontmost whichever has the keyboard. The listener sees every key and click before it lands,
//! as the event tap does. When focus crosses the form panel's edge, the panel reports it, the
//! way the form window calls `Engine::form_panel_focused`; with [`Panels::late`] those reports
//! wait for [`Desk::deliver_reports`], as they do when the main thread is busy.

use super::desk::{windows, Desk};
use super::keyboard::apply_key;
use crate::macos::ffi::{kCGEventKeyDown, kCGEventKeyUp};

/// What has the keyboard, and what a click lands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Focus {
    /// The target app's text field.
    Target,
    /// Kudzu's form panel.
    Form,
    /// Another Kudzu panel (Quick Search, the clipboard picker).
    Panel,
}

#[derive(Debug)]
pub(super) struct Panels {
    pub focus: Focus,
    /// The token of the form on the form panel, once it is shown.
    pub form: Option<u64>,
    /// Focus reports wait for [`Desk::deliver_reports`] instead of going out at once.
    pub late: bool,
    /// Reports not yet delivered: whether the form panel is key.
    pub queued: Vec<bool>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            focus: Focus::Target,
            form: None,
            late: false,
            queued: Vec::new(),
        }
    }
}

impl Desk {
    /// The form window shows `token`'s form and makes its panel key.
    pub fn show_form(&self, token: u64) {
        self.with(|s| s.panels.form = Some(token));
        self.focus_moves(Focus::Form);
    }

    /// Focus moves without a click: a shortcut opens another panel, a panel closes, the form
    /// window makes its panel key again.
    pub fn focus_moves(&self, to: Focus) {
        let report = self.with(|s| {
            let was_form = s.panels.focus == Focus::Form;
            s.panels.focus = to;
            let crosses = was_form != (to == Focus::Form) && s.panels.form.is_some();
            if crosses {
                s.panels.queued.push(to == Focus::Form);
            }
            !s.panels.late
        });
        if report {
            self.deliver_reports();
        }
    }

    /// The user clicks `on`, in the target's text before grapheme `caret` when given. The
    /// listener sees the click (and the window it lands on) first; then it lands and moves
    /// focus.
    pub fn click_on(&self, on: Focus, caret: Option<usize>) {
        self.click_in(match on {
            Focus::Target => windows::TARGET,
            Focus::Form => windows::FORM,
            Focus::Panel => windows::PANEL,
        });
        if let (Focus::Target, Some(index)) = (on, caret) {
            self.with(|s| s.doc.place_caret(index));
        }
        self.focus_moves(on);
    }

    /// One key event, going wherever focus is: only the target's document changes.
    pub fn key_at_focus(&self, key: &str, down: bool) -> bool {
        let event = self.key_event(key, down);
        let kind = if down { kCGEventKeyDown } else { kCGEventKeyUp };
        let swallowed = self.deliver(kind, event.event.0);
        self.with(|s| {
            if !swallowed && down && s.panels.focus == Focus::Target {
                apply_key(s, event.code, 0, &event.text);
            }
        });
        swallowed
    }

    /// Sends the form panel's queued focus reports to the engine, in order.
    pub fn deliver_reports(&self) {
        let (token, reports) = self.with(|s| (s.panels.form, std::mem::take(&mut s.panels.queued)));
        let Some(token) = token else { return };
        let engine = self.engine();
        for key in reports {
            engine.form_panel_focused(token, key);
        }
    }
}
