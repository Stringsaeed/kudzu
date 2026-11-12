//! The list's and the filter's keys (`snippet-list.tsx`, LIB-4).
//!
//! List: arrows and bare J/K move (⇧ extends from the anchor), Home and End, Page ±8, Return
//! and → open the editor, ⌘A selects every row, Escape drops the multi-selection, `/` focuses
//! the filter, and any other printable character focuses the filter and lands in it.
//!
//! Filter: arrows move the list, Return opens the editor, Escape clears the text, then a
//! second Escape returns to the list.

use rune::{Key, KeyEvent, Tx};

use super::model::PAGE;
use super::{actions, row_key, sync, SCROLL_KEY};
use crate::main::editor;
use crate::main::shortcut_meta::LocalShortcut;
use crate::shell::MainState;

/// List chords that are not window actions. [`list`] still handles the keys.
pub(crate) const LIST_SHORTCUTS: &[LocalShortcut] = &[
    LocalShortcut {
        id: "list-up",
        chord: "up",
        label: "Previous (or K)",
    },
    LocalShortcut {
        id: "list-down",
        chord: "down",
        label: "Next (or J)",
    },
    LocalShortcut {
        id: "list-extend",
        chord: "shift+down",
        label: "Extend selection",
    },
    LocalShortcut {
        id: "list-all",
        chord: "mod+a",
        label: "Select all",
    },
    LocalShortcut {
        id: "list-open",
        chord: "enter",
        label: "Open in editor",
    },
    LocalShortcut {
        id: "list-filter",
        chord: "/",
        label: "Filter",
    },
];

/// Moves the open snippet `delta` rows and reveals it. Consumed even on an empty list.
fn step(s: &mut MainState, delta: isize, extend: bool, tx: &mut Tx) -> bool {
    let Some(listing) = s.library.listing(s) else {
        return false;
    };
    let cursor = s.frame.selected_snippet;
    if let Some(id) = s
        .library
        .selection
        .step(&listing.ids, cursor, delta, extend)
    {
        s.select_snippet(Some(id));
        tx.scroll_into_view_in(SCROLL_KEY, row_key(id));
    }
    true
}

fn rows(s: &MainState) -> isize {
    s.library.listing(s).map_or(0, |l| l.ids.len() as isize)
}

/// `onOpen`: the editor's content takes focus.
fn open(s: &mut MainState, tx: &mut Tx) -> bool {
    editor::edit_content(s, tx);
    true
}

/// `on_key` of the list.
pub(super) fn list(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.is_composing() || s.frame.any_modal_open() || s.library.menu_open() {
        return false;
    }
    let m = ev.mods;
    let bare = !m.cmd && !m.ctrl && !m.alt;
    match &ev.key {
        Key::Down => step(s, 1, m.shift, tx),
        Key::Up => step(s, -1, m.shift, tx),
        Key::Text(t) if bare && t == "j" => step(s, 1, m.shift, tx),
        Key::Text(t) if bare && t == "k" => step(s, -1, m.shift, tx),
        Key::Home => {
            let n = rows(s);
            step(s, -n, m.shift, tx)
        }
        Key::End => {
            let n = rows(s);
            step(s, n, m.shift, tx)
        }
        Key::PageDown => step(s, PAGE, m.shift, tx),
        Key::PageUp => step(s, -PAGE, m.shift, tx),
        Key::Enter | Key::Right => open(s, tx),
        Key::Command('a') if !m.shift && !m.alt => select_all(s),
        Key::Text(t) if m.ctrl && t == "a" => select_all(s),
        Key::Escape if !s.library.selection.multi().is_empty() => {
            s.library.selection.clear();
            true
        }
        Key::Text(t) if bare && t == "/" => {
            actions::focus_filter(s, tx);
            true
        }
        // `?` is the cheat sheet's, as the window shortcut sees it before the filter could.
        Key::Text(t) if bare && t != "?" && is_printable(t) => {
            actions::focus_filter(s, tx);
            let end = s.library.filter.text.len();
            s.library.filter.set_caret(end);
            s.library.filter.insert(t);
            sync(s);
            true
        }
        _ => false,
    }
}

/// One character that isn't whitespace (`k.length === 1 && /\S/.test(k)`).
fn is_printable(t: &str) -> bool {
    let mut chars = t.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if !c.is_whitespace() && !c.is_control())
}

fn select_all(s: &mut MainState) -> bool {
    if let Some(listing) = s.library.listing(s) {
        s.library.selection.select_all(&listing.ids);
    }
    true
}

/// `on_key` of the filter field.
pub(super) fn filter(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.library.filter.is_composing() || s.frame.any_modal_open() {
        return false;
    }
    let m = ev.mods;
    if m.cmd || m.ctrl || m.alt {
        return false;
    }
    match ev.key {
        Key::Down => step(s, 1, m.shift, tx),
        Key::Up => step(s, -1, m.shift, tx),
        Key::Enter => open(s, tx),
        Key::Escape if !s.library.filter.text.is_empty() => {
            actions::clear_filter(s);
            true
        }
        Key::Escape => {
            actions::focus_list(s, tx);
            true
        }
        _ => false,
    }
}
