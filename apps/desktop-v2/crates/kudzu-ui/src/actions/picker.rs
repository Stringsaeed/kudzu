//! Clipboard picker commands, for a cheat sheet and menu labels. The window does not route
//! keys through [`super::route`]: the search field has to see ⌃N before any command and
//! `picker::keys` already owns the chords, so `run` performs the same effect.
use rune::{Keybind, Tx};

use super::Action;
use crate::picker::drive::apply;
use crate::picker::keys::Effect;
use crate::shell::windows::PickerState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickerAction {
    Paste,
    PlainText,
    CopyOnly,
    Pin,
    Delete,
    Undo,
    SaveAsSnippet,
    Open,
    ClipboardSettings,
    NextFilter,
    PreviousFilter,
    Close,
}

impl PickerAction {
    /// The effect `picker::keys` gives this action's key.
    pub fn effect(self) -> Effect {
        match self {
            Self::Paste => Effect::Paste {
                nth: None,
                flip: false,
            },
            Self::PlainText => Effect::Paste {
                nth: None,
                flip: true,
            },
            Self::CopyOnly => Effect::CopyOnly { flip: false },
            Self::Pin => Effect::Pin,
            Self::Delete => Effect::Delete,
            Self::Undo => Effect::Undo,
            Self::SaveAsSnippet => Effect::Snippet,
            Self::Open => Effect::Open,
            Self::ClipboardSettings => Effect::Settings,
            Self::NextFilter => Effect::Filter { backwards: false },
            Self::PreviousFilter => Effect::Filter { backwards: true },
            Self::Close => Effect::Dismiss,
        }
    }
}

impl Action<PickerState> for PickerAction {
    const ALL: &'static [Self] = &[
        Self::Paste,
        Self::PlainText,
        Self::CopyOnly,
        Self::Pin,
        Self::Delete,
        Self::Undo,
        Self::SaveAsSnippet,
        Self::Open,
        Self::ClipboardSettings,
        Self::NextFilter,
        Self::PreviousFilter,
        Self::Close,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Paste => "Paste",
            Self::PlainText => "Paste as plain text",
            Self::CopyOnly => "Copy",
            Self::Pin => "Pin",
            Self::Delete => "Delete",
            Self::Undo => "Undo delete",
            Self::SaveAsSnippet => "Save as snippet",
            Self::Open => "Open link or file",
            Self::ClipboardSettings => "Clipboard settings",
            Self::NextFilter => "Next filter",
            Self::PreviousFilter => "Previous filter",
            Self::Close => "Close",
        }
    }

    fn keys(self) -> Keybind {
        match self {
            Self::Paste => Keybind::new("enter"),
            Self::PlainText => Keybind::new("shift+enter"),
            Self::CopyOnly => Keybind::new("mod+enter"),
            Self::Pin => Keybind::new("mod+p"),
            Self::Delete => Keybind::new("mod+backspace"),
            Self::Undo => Keybind::new("mod+z"),
            Self::SaveAsSnippet => Keybind::new("mod+s"),
            Self::Open => Keybind::new("mod+o"),
            Self::ClipboardSettings => Keybind::new("mod+comma"),
            Self::NextFilter => Keybind::new("tab"),
            Self::PreviousFilter => Keybind::new("shift+tab"),
            Self::Close => Keybind::new("escape"),
        }
    }

    fn enabled(self, state: &PickerState) -> bool {
        let p = &state.model.picker;
        match self {
            Self::Undo => p.undo.is_some() && p.query().is_empty(),
            Self::Paste | Self::PlainText | Self::CopyOnly | Self::Pin | Self::Delete => {
                p.active().is_some()
            }
            Self::SaveAsSnippet => p
                .active()
                .is_some_and(kudzu_core::clipboard::facts::has_text),
            Self::Open => p.active().is_some_and(|e| {
                matches!(
                    e.kind,
                    kudzu_core::clipboard::ClipKind::Link | kudzu_core::clipboard::ClipKind::File
                )
            }),
            _ => true,
        }
    }

    fn run(self, state: &mut PickerState, tx: &mut Tx) {
        let effect = match self {
            // Escape clears a query before it closes.
            Self::Close if !state.model.picker.query().is_empty() => Effect::ClearQuery,
            other => other.effect(),
        };
        apply(state, effect, tx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picker::keys::{effect, Ctx};
    use rune::{Key, KeyEvent, Mods};

    fn event(key: Key, cmd: bool, shift: bool) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                cmd,
                shift,
                ..Mods::default()
            },
        }
    }

    /// An action's key is the key `picker::keys` gives the same effect, so the cheat sheet
    /// and the window cannot disagree.
    #[test]
    fn every_action_key_asks_for_its_own_effect() {
        let ctx = Ctx {
            query_empty: true,
            can_undo: true,
        };
        let presses = [
            (PickerAction::Paste, event(Key::Enter, false, false)),
            (PickerAction::PlainText, event(Key::Enter, false, true)),
            (PickerAction::CopyOnly, event(Key::Enter, true, false)),
            (PickerAction::Pin, event(Key::Command('p'), true, false)),
            (PickerAction::Delete, event(Key::Backspace, true, false)),
            (PickerAction::Undo, event(Key::Command('z'), true, false)),
            (
                PickerAction::SaveAsSnippet,
                event(Key::Command('s'), true, false),
            ),
            (PickerAction::Open, event(Key::Command('o'), true, false)),
            (
                PickerAction::ClipboardSettings,
                event(Key::Command(','), true, false),
            ),
            (PickerAction::NextFilter, event(Key::Tab, false, false)),
            (PickerAction::PreviousFilter, event(Key::Tab, false, true)),
            (PickerAction::Close, event(Key::Escape, false, false)),
        ];
        assert_eq!(presses.len(), PickerAction::ALL.len());
        for (action, ev) in presses {
            assert!(
                action.keys().matches(&ev),
                "{action:?} has the key it is shown with"
            );
            assert_eq!(effect(&ev, ctx), action.effect(), "{action:?}");
        }
    }
}
