//! Form window actions (LIB-13, LIB-14): Insert and Cancel.
use rune::{Keybind, Tx};

use super::Action;
use crate::shell::windows::FormState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormAction {
    Insert,
    Cancel,
}

impl Action<FormState> for FormAction {
    const ALL: &'static [Self] = &[Self::Insert, Self::Cancel];

    fn label(self) -> &'static str {
        match self {
            Self::Insert => "Insert",
            Self::Cancel => "Cancel",
        }
    }

    fn keys(self) -> Keybind {
        match self {
            Self::Insert => Keybind::new("mod+enter"),
            Self::Cancel => Keybind::new("escape"),
        }
    }

    fn enabled(self, state: &FormState) -> bool {
        state.model.request.is_some()
    }

    fn run(self, state: &mut FormState, _: &mut Tx) {
        match self {
            Self::Insert => state.submit(),
            Self::Cancel => state.cancel(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use rune::{Key, KeyEvent, Mods};

    fn event(key: Key, cmd: bool) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                cmd,
                ..Mods::default()
            },
        }
    }

    #[test]
    fn insert_is_mod_enter_and_cancel_is_escape() {
        assert!(FormAction::Insert.keys().matches(&event(Key::Enter, true)));
        assert!(FormAction::Cancel
            .keys()
            .matches(&event(Key::Escape, false)));
        assert!(!FormAction::Insert.keys().matches(&event(Key::Enter, false)));
    }
}
