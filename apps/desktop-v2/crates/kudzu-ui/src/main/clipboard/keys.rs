//! Keys on the clipboard screen: the list's, the search field's, and the chords that work in
//! both (`clipboard-view.tsx` `onListKey`, the field's `onKeyDown` and `useHotkeys`).
//! `handle` runs before the frame's [`crate::actions::main::MainAction`]s, so ⌘⌫ here deletes
//! clips, not the snippet the library last had open.

use rune::{Key, KeyEvent, Tx};
use uuid::Uuid;

use super::{actions, focus_list, focus_search};
use crate::clip::model::PAGE;
use crate::shell::MainState;

/// True when the screen took the key.
pub fn handle(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    let editing = s.clipboard.is_editing_text();
    let took = if s.clipboard.large_preview {
        large_preview_key(s, ev, tx)
    } else if m.cmd && !m.ctrl && !m.alt {
        chord(s, ev, editing, tx)
    } else if m.cmd || m.ctrl || m.alt {
        false
    } else if editing {
        search_key(s, ev, tx)
    } else {
        list_key(s, ev, tx)
    };
    // The key may have moved the active row: point the preview at it now, not next poll.
    super::track_active(s);
    took
}

/// ⌘ chords. ⌘P, ⌘S, ⌘O and ⌘R also work inside the search field; the rest leave the field's
/// own editing alone.
fn chord(s: &mut MainState, ev: &KeyEvent, editing: bool, tx: &mut Tx) -> bool {
    let shift = ev.mods.shift;
    match &ev.key {
        Key::Command('p') if !shift => actions::toggle_pin(s),
        Key::Command('s') if !shift => actions::save_as_snippet(s),
        Key::Command('o') if !shift => actions::open_active(s, tx),
        Key::Command('r') if !shift => actions::reveal_active(s),
        Key::Backspace if !editing => actions::delete_selection(s, tx),
        Key::Command('c') if !editing && !shift => actions::copy_active(s, false, tx),
        Key::Command('a') if !editing && !shift => s.clipboard.clips.list.select_all(),
        _ => return false,
    }
    true
}

fn search_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Down => step(s, 1, ev.mods.shift, tx),
        Key::Up => step(s, -1, ev.mods.shift, tx),
        Key::Enter => actions::copy_active(s, ev.mods.shift, tx),
        Key::Escape => {
            if s.clipboard.search.text.is_empty() {
                focus_list(s, tx);
            } else {
                s.clipboard.search.replace_text("");
            }
        }
        _ => return false,
    }
    true
}

fn list_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let shift = ev.mods.shift;
    let len = s.clipboard.clips.list.len() as isize;
    match &ev.key {
        Key::Down => step(s, 1, shift, tx),
        Key::Up => step(s, -1, shift, tx),
        Key::Text(t) if t == "j" => step(s, 1, shift, tx),
        Key::Text(t) if t == "k" => step(s, -1, shift, tx),
        Key::Home => step(s, -len, shift, tx),
        Key::End => step(s, len, shift, tx),
        Key::PageDown => step(s, PAGE, shift, tx),
        Key::PageUp => step(s, -PAGE, shift, tx),
        Key::Enter => actions::copy_active(s, shift, tx),
        Key::Text(t) if t == " " => {
            if s.clipboard.clips.list.active().is_some() {
                s.clipboard.large_preview = true;
            }
        }
        Key::Backspace | Key::Delete => actions::delete_selection(s, tx),
        Key::Escape => return s.clipboard.clips.list.clear_multi(),
        Key::Text(t) if t == "/" => focus_search(s, tx),
        Key::Text(t) if is_typing(t) => {
            focus_search(s, tx);
            let typed = format!("{}{t}", s.clipboard.search.text);
            s.clipboard.search.replace_text(typed);
        }
        _ => return false,
    }
    true
}

/// The large preview takes Space and Escape to close and the arrows to move.
fn large_preview_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match &ev.key {
        Key::Escape => s.clipboard.large_preview = false,
        Key::Text(t) if t == " " => s.clipboard.large_preview = false,
        Key::Down => step(s, 1, false, tx),
        Key::Up => step(s, -1, false, tx),
        _ => return false,
    }
    true
}

/// A single visible character: "type to search".
fn is_typing(t: &str) -> bool {
    let mut chars = t.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if !c.is_whitespace() && !c.is_control())
}

fn step(s: &mut MainState, delta: isize, extend: bool, tx: &mut Tx) {
    if let Some(id) = s.clipboard.clips.list.move_by(delta, extend) {
        reveal(s, id, tx);
    }
}

/// Scrolls `id` into view, keeping the first row of a section under its header.
pub(super) fn reveal(s: &MainState, id: Uuid, tx: &mut Tx) {
    if let Some(key) = s.clipboard.clips.list.scroll_key(id) {
        tx.scroll_into_view(key.to_owned());
    }
}
