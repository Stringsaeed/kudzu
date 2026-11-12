//! Headless states for the picker, named as in `reference/STATES.md`: `history` is the
//! preview's sample, `clips-5000` adds its 5,000 generated copies (`?clips=5000`) and
//! `empty` has none (`?emptyclips`).

use kudzu_core::clipboard::History;

use crate::main::clipboard::scenes::{history_5000, sample};
use crate::shell::windows::PickerState;

type Scene = (&'static str, fn(&mut PickerState));

pub const SCENES: &[Scene] = &[
    ("history", history),
    ("clips-5000", clips_5000),
    ("empty", empty),
];

fn open(s: &mut PickerState, history: History) {
    if let Err(error) = s.svc.clipboard.replace_history(history) {
        log::warn!("the scene history was not applied: {error}");
    }
    s.model.open = true;
    let now = s.now();
    s.model.picker.reset(now);
    super::drive::pull(s);
}

pub fn history(s: &mut PickerState) {
    open(s, sample());
}

pub fn clips_5000(s: &mut PickerState) {
    open(s, history_5000());
}

/// `clips-5000` with a history the caller trimmed (the scroll trace drops concealed rows).
#[cfg(test)]
pub(crate) fn clips_5000_with(s: &mut PickerState, history: History) {
    open(s, history);
}

pub fn empty(s: &mut PickerState) {
    open(s, History::new());
}
