//! Where the user's keys and clicks go on the fake desktop, and the form panel's focus reports.
//!
//! Three destinations: the target app's text field, Kudzu's form panel, and another Kudzu
//! panel (Quick Search, the picker). Kudzu's panels don't activate Kudzu, so the target stays
//! frontmost whichever has the keyboard. The listener sees every key and click before it lands,
//! as the event tap does. When focus crosses the form panel's edge, the panel reports it, the
//! way the form window calls `Engine::form_panel_focused`; with [`Panels::late`] those reports
//! wait for [`Desk::deliver_reports`], as they do when the main thread is busy.

use std::collections::HashMap;

use super::desk::{Desk, State};
use super::keyboard::apply_key;
use crate::{EventStamp, InputEvent, PanelFocus, Submission};

/// The windows a click can land on, by window-server id, and who owns them.
pub(super) mod windows {
    /// The target app's document window.
    pub const TARGET: i64 = 11;
    /// Kudzu's form panel.
    pub const FORM: i64 = 12;
    /// Another Kudzu panel (Quick Search, the picker).
    pub const PANEL: i64 = 13;
    /// Another app's window.
    pub const OTHER: i64 = 14;

    /// Kudzu's windows, as the host reports them: the form panel and another panel.
    pub fn kudzu() -> Vec<crate::KudzuWindow> {
        [FORM, PANEL]
            .map(|number| crate::KudzuWindow {
                number: number as u32,
                passes_clicks_through: false,
            })
            .to_vec()
    }
}

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
    /// Reports not yet delivered, per form token.
    pub queued: Vec<(u64, PanelFocus)>,
    /// Reports numbered so far, per form token.
    pub seqs: HashMap<u64, u64>,
}

impl Default for Panels {
    fn default() -> Self {
        Self {
            focus: Focus::Target,
            form: None,
            late: false,
            queued: Vec::new(),
            seqs: HashMap::new(),
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
            if let Some(token) = s.panels.form.filter(|_| was_form != (to == Focus::Form)) {
                let report = s.numbered(token, to == Focus::Form);
                s.panels.queued.push((token, report));
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
        let swallowed = self.deliver_key(&event);
        self.with(|s| {
            if !swallowed && down && s.panels.focus == Focus::Target {
                apply_key(s, event.code, 0, &event.text);
            }
        });
        swallowed
    }

    /// [`Desk::key_at_focus`] with a chosen timestamp (0: the event carries none).
    pub fn key_at_focus_stamped(&self, key: &str, down: bool, stamp: u64) -> bool {
        let event = self.key_event(key, down);
        unsafe { crate::macos::ffi::CGEventSetTimestamp(event.event.0, stamp) };
        let swallowed = self.deliver_key(&event);
        self.with(|s| {
            if !swallowed && down && s.panels.focus == Focus::Target {
                apply_key(s, event.code, 0, &event.text);
            }
        });
        swallowed
    }

    /// [`Desk::key_at_focus`] for an event the session says goes to process `pid`
    /// (`kCGEventTargetUnixProcessID`).
    pub fn key_at_focus_to(&self, key: &str, down: bool, pid: i32) -> bool {
        let event = self.key_event(key, down);
        unsafe {
            crate::macos::ffi::CGEventSetIntegerValueField(
                event.event.0,
                crate::macos::ffi::kCGEventTargetUnixProcessID,
                i64::from(pid),
            )
        };
        let swallowed = self.deliver_key(&event);
        self.with(|s| {
            if !swallowed && down && s.panels.focus == Focus::Target {
                apply_key(s, event.code, 0, &event.text);
            }
        });
        swallowed
    }

    /// Sends the form panel's queued focus reports to the engine, in order.
    pub fn deliver_reports(&self) {
        let reports = self.with(|s| std::mem::take(&mut s.panels.queued));
        let engine = self.engine();
        for (token, report) in reports {
            engine.form_panel_focused(token, report);
        }
    }

    /// The form window reports that `token`'s panel became key or stopped being key, now.
    pub fn report_focus(&self, token: u64, key: bool) {
        let report = self.with(|s| s.numbered(token, key));
        self.engine().form_panel_focused(token, report);
    }

    /// How the form window submits `token` by the last event received.
    pub fn submission(&self, token: u64) -> Submission {
        self.submission_by(token, Some(self.last_event()))
    }

    /// How the form window submits `token` by `event`.
    pub fn submission_by(&self, token: u64, event: Option<InputEvent>) -> Submission {
        let focus_seq = self.with(|s| s.panels.seqs.get(&token).copied().unwrap_or(0));
        Submission { event, focus_seq }
    }
}

impl State {
    /// The next numbered report for `token`, bounded by the last event received.
    fn numbered(&mut self, token: u64, key: bool) -> PanelFocus {
        let seq = self.panels.seqs.entry(token).or_default();
        *seq += 1;
        PanelFocus {
            key,
            seq: *seq,
            after: Some(self.last_event()),
        }
    }

    /// The last event the user's apps received.
    pub fn last_event(&self) -> InputEvent {
        InputEvent {
            stamp: EventStamp(self.last_stamp),
            key_code: self.last_key,
        }
    }
}
