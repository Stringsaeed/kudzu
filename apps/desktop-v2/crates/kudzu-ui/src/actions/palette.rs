//! Quick Search commands. The window does not route keys through [`super::route`]:
//! the field has to see Control+N before any command, and `drive` already owns the
//! chords. `run` performs the same effect, for a later cheat sheet.
use rune::{Keybind, Tx};

use super::Action;
use crate::palette::{self, clamped, Effect};
use crate::shell::windows::PaletteState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    Insert,
    Copy,
    Edit,
    NewSnippet,
    Dismiss,
}

impl Action<PaletteState> for PaletteAction {
    const ALL: &'static [Self] = &[
        Self::Insert,
        Self::Copy,
        Self::Edit,
        Self::NewSnippet,
        Self::Dismiss,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Insert => "Insert",
            Self::Copy => "Copy",
            Self::Edit => "Edit",
            Self::NewSnippet => "New Snippet",
            Self::Dismiss => "Close",
        }
    }

    fn keys(self) -> Keybind {
        match self {
            Self::Insert => Keybind::new("enter"),
            Self::Copy => Keybind::new("mod+enter"),
            Self::Edit => Keybind::new("mod+e"),
            Self::NewSnippet => Keybind::new("mod+n"),
            Self::Dismiss => Keybind::new("escape"),
        }
    }

    fn run(self, state: &mut PaletteState, tx: &mut Tx) {
        match self {
            Self::Insert => palette::pick(state, clamped(state), tx),
            Self::Copy => {
                palette::apply(state, hit(state).map(Effect::Copy).unwrap_or(Effect::None))
            }
            Self::Edit => {
                palette::apply(state, hit(state).map(Effect::Edit).unwrap_or(Effect::None))
            }
            Self::NewSnippet => {
                let prefix = state
                    .svc
                    .settings
                    .watch()
                    .get()
                    .default_trigger_prefix
                    .clone();
                let seed = palette::create_seed(&state.model.search.keys.query, &prefix);
                palette::apply(state, Effect::Create(seed));
            }
            Self::Dismiss => {
                let effect = if state.model.search.keys.query.is_empty() {
                    Effect::Dismiss {
                        restore: !state.shell.main_focused(),
                    }
                } else {
                    state.model.search.keys.query.clear();
                    state.model.search.keys.active = 0;
                    Effect::QueryCleared
                };
                palette::apply(state, effect);
            }
        }
    }
}

fn hit(state: &PaletteState) -> Option<uuid::Uuid> {
    let index = clamped(state);
    state
        .model
        .search
        .presentation
        .rows
        .get(index)
        .map(|row| row.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Key, KeyEvent, Mods};

    fn command(c: char) -> KeyEvent {
        KeyEvent {
            key: Key::Command(c),
            mods: Mods {
                cmd: true,
                ..Mods::default()
            },
        }
    }

    #[test]
    fn command_q_is_not_a_palette_action() {
        let quit = command('q');
        assert!(PaletteAction::ALL
            .iter()
            .all(|action| !action.keys().matches(&quit)));
        assert!(PaletteAction::Edit.keys().matches(&command('e')));
        assert!(PaletteAction::NewSnippet.keys().matches(&command('n')));
    }
}
