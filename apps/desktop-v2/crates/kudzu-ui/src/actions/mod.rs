//! Typed actions (PLAN §1.3): one enum per window in this folder, each listing its actions
//! with a label, a keybind, an enabled predicate and a run function. `on_key` routes through
//! `route`; the ⌘K menu, the cheat sheet and menu labels read `Action::ALL`. Shortcuts are
//! declared once (LIB-13, LIB-14). This is not a command bus: no names, no dispatch table.
pub mod form;
#[allow(special_module_name)]
pub mod main;
pub mod menu;
pub mod palette;
pub mod permissions;
pub mod picker;

use rune::{KeyEvent, Keybind, Tx};

pub trait Action<S>: Copy + Eq + 'static {
    /// Every action of the window, in menu order.
    const ALL: &'static [Self];
    fn label(self) -> &'static str;
    /// `Keybind::default()` for an action with no shortcut.
    fn keys(self) -> Keybind;
    fn enabled(self, _state: &S) -> bool {
        true
    }
    fn run(self, state: &mut S, tx: &mut Tx);
}

/// The first enabled action whose keybind matches `key`.
pub fn find<S, A: Action<S>>(state: &S, key: &KeyEvent) -> Option<A> {
    A::ALL
        .iter()
        .copied()
        .find(|action| action.keys().matches(key) && action.enabled(state))
}

/// Run the matching action. True consumes the key, as `AppSpec::on_key` expects.
pub fn route<S, A: Action<S>>(state: &mut S, key: &KeyEvent, tx: &mut Tx) -> bool {
    match find::<S, A>(state, key) {
        Some(action) => {
            action.run(state, tx);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Key, Mods};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Demo {
        Close,
        Save,
    }

    struct State {
        dirty: bool,
    }

    impl Action<State> for Demo {
        const ALL: &'static [Self] = &[Demo::Close, Demo::Save];
        fn label(self) -> &'static str {
            match self {
                Demo::Close => "Close Windows",
                Demo::Save => "Save",
            }
        }
        fn keys(self) -> Keybind {
            match self {
                Demo::Close => Keybind::new("mod+q"),
                Demo::Save => Keybind::new("mod+s"),
            }
        }
        fn enabled(self, state: &State) -> bool {
            self != Demo::Save || state.dirty
        }
        fn run(self, _: &mut State, _: &mut Tx) {}
    }

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
    fn keys_find_enabled_actions_only() {
        let clean = State { dirty: false };
        assert_eq!(find::<_, Demo>(&clean, &command('q')), Some(Demo::Close));
        assert_eq!(find::<_, Demo>(&clean, &command('s')), None);
        assert_eq!(
            find::<_, Demo>(&State { dirty: true }, &command('s')),
            Some(Demo::Save)
        );
        assert_eq!(find::<_, Demo>(&clean, &command('x')), None);
        let labels: Vec<_> = Demo::ALL
            .iter()
            .map(|a| Action::<State>::label(*a))
            .collect();
        assert_eq!(labels, ["Close Windows", "Save"]);
    }
}
