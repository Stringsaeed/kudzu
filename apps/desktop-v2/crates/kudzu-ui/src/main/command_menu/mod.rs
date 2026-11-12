//! Command menu (`⌘K`). A 600-point panel, 18% down the window, listing frequent or
//! matching snippets and the actions that can run right now.
//!
//! Rune has no `h_frac`, so the top gap is a `grow(0.18)` spacer over a `grow(0.82)`
//! column. The menu's own field is not `is_editing_text`. Opening snapshots the field
//! under the menu: once this query takes focus, Rune clears that field's `focused`.
//! A click focuses the search button before its press handler, so `poll` records
//! `text_field_live` while the menu is closed and `open` reads that latch.
//! The list is at most `min(440, 60vh)` and shrinks to its rows.

mod catalog;
mod io;
mod rows;

use rune::{el, El, Key, KeyEvent, TextEdit, Tx};

pub use io::{export_library, import_snippets, toggle_appearance, toggle_clipboard};

use crate::actions::main::MainAction;
use crate::actions::Action;
use crate::main::nav;
use crate::main::state::{LibraryFilter, MainView};
use crate::shell::MainState;
use crate::theme::Theme;

const QUERY_KEY: &str = "command-menu-query";
const PLACEHOLDER: &str = "Search snippets, groups and actions…";

/// The menu's query and which row Up/Down last landed on.
#[derive(Clone, Default)]
pub struct State {
    pub query: TextEdit,
    pub selected: usize,
    /// A text field under the menu was focused when the menu opened.
    underlying_editing: bool,
    /// The last poll while the menu was closed. A click blurs that field before
    /// the press handler runs, so `open` cannot read `is_editing_text` alone.
    text_field_live: bool,
}

/// Records whether a text field under the menu is focused. Poll runs before the
/// pointer queue, and only while the menu is closed, so the menu's own field
/// never counts.
pub fn note_text_field(s: &mut MainState) -> bool {
    if s.frame.command_open {
        return false;
    }
    let live = s.is_editing_text();
    if s.command_menu.text_field_live == live {
        return false;
    }
    s.command_menu.text_field_live = live;
    true
}

/// Opens the menu. A second call leaves it open: `⌘K` opens, it does not toggle.
pub fn open(s: &mut MainState, tx: &mut Tx) {
    if s.frame.command_open {
        return;
    }
    s.command_menu.underlying_editing = s.command_menu.text_field_live || s.is_editing_text();
    s.frame.command_open = true;
    s.command_menu.selected = 0;
    s.command_menu.query.focused = true;
    tx.focus(QUERY_KEY, true);
}

/// Closes the menu and clears the query, the selection and the field focus.
pub fn close(s: &mut MainState) {
    s.frame.command_open = false;
    s.command_menu.query.clear();
    s.command_menu.query.focused = false;
    s.command_menu.selected = 0;
    s.command_menu.underlying_editing = false;
}

/// Arrows and Return while the menu field is focused. Keys during composition, and
/// plain text and Escape, return false so the field inserts and the modal dismisses.
pub fn on_key(s: &mut MainState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if s.command_menu.query.is_composing() {
        return false;
    }
    if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt || ev.mods.shift {
        return false;
    }
    match ev.key {
        Key::Up => {
            nudge(s, -1);
            true
        }
        Key::Down => {
            nudge(s, 1);
            true
        }
        Key::Enter => {
            activate(s, tx);
            true
        }
        _ => false,
    }
}

pub fn view(s: &MainState, theme: &Theme, viewport_h: f32, overlays: &mut Vec<El>) {
    if !s.frame.command_open {
        return;
    }
    let hits = catalog::hits(s);
    let scrim = el()
        .key("command-menu-scrim")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(theme.kit().c.scrim)
        .a11y_hidden();
    let layer = el()
        .key("command-menu-layer")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .child(
            rune::col()
                .w_full()
                .h_full()
                .items_center()
                .child(el().grow(0.18).w_full())
                .child(
                    rune::col()
                        .grow(0.82)
                        .w_full()
                        .min_h(0.0)
                        .items_center()
                        .child(rows::panel(s, theme, viewport_h, &hits)),
                ),
        );
    overlays.push(scrim);
    overlays.push(layer);
}

fn nudge(s: &mut MainState, delta: isize) {
    let len = catalog::hits(s).len();
    if len == 0 {
        return;
    }
    let cur = if s.command_menu.selected < len {
        s.command_menu.selected
    } else {
        0
    };
    s.command_menu.selected = (cur as isize + delta).rem_euclid(len as isize) as usize;
}

pub(super) fn activate(s: &mut MainState, tx: &mut Tx) {
    let Some(hit) = catalog::hits(s).get(s.command_menu.selected) else {
        return;
    };
    match hit {
        catalog::Flat::Snippet(id) => {
            s.navigate_to(MainView::Library(LibraryFilter::All));
            s.select_snippet(Some(id));
            close(s);
        }
        // Closing and then running would open the menu again.
        catalog::Flat::Command(catalog::Kind::Action(MainAction::CommandMenu)) => close(s),
        catalog::Flat::Command(catalog::Kind::Action(action)) => {
            close(s);
            action.run(s, tx);
        }
        catalog::Flat::Command(catalog::Kind::GoToGroup(id)) => {
            close(s);
            nav::view(s, MainView::Library(LibraryFilter::Group(id)), tx);
        }
        catalog::Flat::Command(catalog::Kind::GroupSettings(id)) => {
            close(s);
            s.frame.group_settings_id = Some(id);
        }
    }
}
