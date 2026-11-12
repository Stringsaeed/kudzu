//! The picker's keys as pure data: which key asks for what (`onKeyDown` in
//! `clipboard-picker.tsx`). The window applies the effect; nothing here touches a service.
//!
//! The search field keeps keyboard focus the whole time, so everything the picker owns is
//! named here and everything else goes on to the field.

use rune::{Key, KeyEvent};

/// Rows moved by Page Up and Page Down.
pub const PAGE: isize = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The key is the field's (a letter, a caret move, ⌘Z while the query has text).
    None,
    /// Arrows and ⌃N/⌃P by one, Page keys by [`PAGE`]; clamped at both ends.
    Move(isize),
    /// ↩ and ⌘1–9 paste. `nth` is the 1-based number key. ⇧ flips plain and formatted.
    Paste {
        nth: Option<usize>,
        flip: bool,
    },
    /// ⌘↩ copies without pasting.
    CopyOnly {
        flip: bool,
    },
    Pin,
    Delete,
    /// ⌘Z, only while there is a deletion to undo and the query is empty.
    Undo,
    Snippet,
    /// ⌘O: open the link or file and close.
    Open,
    /// ⌘,: clipboard settings.
    Settings,
    /// Tab and Shift-Tab.
    Filter {
        backwards: bool,
    },
    /// Escape with a query.
    ClearQuery,
    /// Escape without one.
    Dismiss,
}

/// What the keys depend on besides the key.
#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub query_empty: bool,
    pub can_undo: bool,
}

fn typed(key: &KeyEvent) -> Option<char> {
    match &key.key {
        Key::Command(c) => Some(c.to_ascii_lowercase()),
        Key::Text(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            chars.next().is_none().then_some(c.to_ascii_lowercase())
        }
        _ => None,
    }
}

pub fn effect(key: &KeyEvent, ctx: Ctx) -> Effect {
    let m = key.mods;
    if m.ctrl && !m.cmd && !m.alt {
        match typed(key) {
            Some('n') => return Effect::Move(1),
            Some('p') => return Effect::Move(-1),
            _ => {}
        }
    }
    match &key.key {
        Key::Down => Effect::Move(1),
        Key::Up => Effect::Move(-1),
        Key::PageDown => Effect::Move(PAGE),
        Key::PageUp => Effect::Move(-PAGE),
        Key::Enter if m.cmd => Effect::CopyOnly { flip: m.shift },
        Key::Enter => Effect::Paste {
            nth: None,
            flip: m.shift,
        },
        Key::Tab => Effect::Filter { backwards: m.shift },
        Key::Escape if ctx.query_empty => Effect::Dismiss,
        Key::Escape => Effect::ClearQuery,
        Key::Backspace | Key::Delete if m.cmd => Effect::Delete,
        _ if m.cmd && !m.ctrl && !m.alt => match typed(key) {
            Some(c @ '1'..='9') => Effect::Paste {
                nth: c.to_digit(10).map(|d| d as usize),
                flip: m.shift,
            },
            Some('p') => Effect::Pin,
            Some('z') if ctx.can_undo && ctx.query_empty => Effect::Undo,
            Some('s') => Effect::Snippet,
            Some('o') => Effect::Open,
            Some(',') => Effect::Settings,
            _ => Effect::None,
        },
        _ => Effect::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    const CTX: Ctx = Ctx {
        query_empty: true,
        can_undo: true,
    };

    fn key(key: Key, cmd: bool, shift: bool, ctrl: bool) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                cmd,
                shift,
                ctrl,
                ..Mods::default()
            },
        }
    }

    fn cmd(c: char) -> KeyEvent {
        key(Key::Command(c), true, false, false)
    }

    #[test]
    fn arrows_control_n_p_and_pages_move() {
        let go = |k: Key, ctrl: bool| effect(&key(k, false, false, ctrl), CTX);
        assert_eq!(go(Key::Down, false), Effect::Move(1));
        assert_eq!(go(Key::Up, false), Effect::Move(-1));
        assert_eq!(go(Key::Text("n".into()), true), Effect::Move(1));
        assert_eq!(go(Key::Text("p".into()), true), Effect::Move(-1));
        assert_eq!(go(Key::PageDown, false), Effect::Move(7));
        assert_eq!(go(Key::PageUp, false), Effect::Move(-7));
        assert_eq!(
            go(Key::Text("n".into()), false),
            Effect::None,
            "a plain n is typed"
        );
    }

    #[test]
    fn return_pastes_shift_flips_and_command_return_only_copies() {
        let enter = |cmd, shift| effect(&key(Key::Enter, cmd, shift, false), CTX);
        assert_eq!(
            enter(false, false),
            Effect::Paste {
                nth: None,
                flip: false
            }
        );
        assert_eq!(
            enter(false, true),
            Effect::Paste {
                nth: None,
                flip: true
            }
        );
        assert_eq!(enter(true, false), Effect::CopyOnly { flip: false });
        assert_eq!(enter(true, true), Effect::CopyOnly { flip: true });
    }

    #[test]
    fn command_digits_paste_the_nth_row() {
        assert_eq!(
            effect(&cmd('1'), CTX),
            Effect::Paste {
                nth: Some(1),
                flip: false
            }
        );
        assert_eq!(
            effect(&cmd('9'), CTX),
            Effect::Paste {
                nth: Some(9),
                flip: false
            }
        );
        assert_eq!(effect(&cmd('0'), CTX), Effect::None);
        let shifted = key(Key::Command('3'), true, true, false);
        assert_eq!(
            effect(&shifted, CTX),
            Effect::Paste {
                nth: Some(3),
                flip: true
            }
        );
    }

    #[test]
    fn the_chords_are_pin_delete_snippet_open_and_settings() {
        assert_eq!(effect(&cmd('p'), CTX), Effect::Pin);
        assert_eq!(effect(&cmd('s'), CTX), Effect::Snippet);
        assert_eq!(effect(&cmd('o'), CTX), Effect::Open);
        assert_eq!(effect(&cmd(','), CTX), Effect::Settings);
        assert_eq!(
            effect(&key(Key::Backspace, true, false, false), CTX),
            Effect::Delete
        );
        assert_eq!(
            effect(&key(Key::Delete, true, false, false), CTX),
            Effect::Delete
        );
        assert_eq!(
            effect(&key(Key::Backspace, false, false, false), CTX),
            Effect::None,
            "a plain Backspace edits the query"
        );
    }

    #[test]
    fn undo_needs_a_deletion_and_an_empty_query() {
        assert_eq!(effect(&cmd('z'), CTX), Effect::Undo);
        let typed = Ctx {
            query_empty: false,
            ..CTX
        };
        assert_eq!(
            effect(&cmd('z'), typed),
            Effect::None,
            "the field undoes its own text"
        );
        let nothing = Ctx {
            can_undo: false,
            ..CTX
        };
        assert_eq!(effect(&cmd('z'), nothing), Effect::None);
    }

    #[test]
    fn tab_filters_and_escape_clears_then_closes() {
        assert_eq!(
            effect(&key(Key::Tab, false, false, false), CTX),
            Effect::Filter { backwards: false }
        );
        assert_eq!(
            effect(&key(Key::Tab, false, true, false), CTX),
            Effect::Filter { backwards: true }
        );
        assert_eq!(
            effect(&key(Key::Escape, false, false, false), CTX),
            Effect::Dismiss
        );
        let typed = Ctx {
            query_empty: false,
            ..CTX
        };
        assert_eq!(
            effect(&key(Key::Escape, false, false, false), typed),
            Effect::ClearQuery
        );
    }

    #[test]
    fn letters_and_other_chords_are_the_fields() {
        assert_eq!(
            effect(&key(Key::Text("a".into()), false, false, false), CTX),
            Effect::None
        );
        assert_eq!(effect(&cmd('a'), CTX), Effect::None);
        assert_eq!(effect(&cmd('c'), CTX), Effect::None);
    }
}
