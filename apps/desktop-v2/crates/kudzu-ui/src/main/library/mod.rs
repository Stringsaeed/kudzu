//! The library list (S50, `snippet-list.tsx`): the header with sort and new, the filter, the
//! virtual rows, the bulk bar and the empty states. LIB-1 to LIB-5 and LIB-12.
//!
//! - `model`: scope, sort, cap, rows and the selection, with no `El`.
//! - `listing`: the derived list, cached per (snapshot, view, filter, sort), and the change
//!   census that decides which rows rise in.
//! - `view`, `row`, `bulk`, `empty`: what is drawn. `keys`: the list's and the filter's keys.
//! - `drag`: rows as drag sources for the sidebar (S42). `actions`: what the list does to the
//!   library. `scenes`: the reference states.
//!
//! [`sync`] keeps the open snippet inside the list. The poll runs it, navigation and library
//! refreshes run it, and every handler here that changes what is listed runs it, so a frame
//! never shows a selection the list has lost.

pub mod actions;
pub mod bulk;
pub mod drag;
pub mod empty;
mod keys;
mod listing;
pub mod model;
#[cfg(test)]
mod model_tests;
mod row;
pub mod scenes;
mod view;

use std::cell::RefCell;
use std::rc::Rc;

use rune::{TextEdit, Tx};
use uuid::Uuid;

use crate::main::state::MainView;
use crate::shell::MainState;

pub use listing::Listing;
use model::{keep_valid, Selection, Sort};
pub use view::view;

pub const FILTER_KEY: &str = "library-filter";
pub const LIST_KEY: &str = "library-list-slot";
/// The scroll viewport around the rows: reveals are scoped to it (#547).
pub const SCROLL_KEY: &str = "library-scroll";
pub const SORT_KEY: &str = "library-sort";
pub const NEW_KEY: &str = "library-new";

/// The key of a snippet's row.
pub fn row_key(id: Uuid) -> String {
    format!("snippet-{id}")
}

/// The library list's own state. The open snippet is the frame's `selected_snippet`.
#[derive(Default)]
pub struct State {
    pub filter: TextEdit,
    pub sort: Sort,
    pub sort_open: bool,
    pub move_open: bool,
    pub selection: Selection,
    pub(crate) drag: drag::Source,
    cache: RefCell<Option<listing::Cache>>,
    /// Tauri's `init` opened the first snippet in library order once; so do we.
    started: bool,
    /// The view the multi-selection belongs to; another view drops it.
    seen_view: Option<MainView>,
    /// The open snippet the list last scrolled to.
    revealed: Option<Uuid>,
    /// The listing version the multi-selection was last pruned against.
    pruned: Option<u64>,
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.filter.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.filter.focused
    }

    pub fn focus_filter(&mut self, tx: &mut Tx) {
        self.filter.focused = true;
        tx.focus(FILTER_KEY, true);
    }

    pub fn blur_inputs(&mut self) {
        self.filter.focused = false;
    }

    /// True while a menu of the list is open.
    pub fn menu_open(&self) -> bool {
        self.sort_open || self.move_open
    }

    /// The list for this window's library snapshot and the library view on screen. Built
    /// again only when one of its inputs changed.
    pub fn listing(&self, s: &MainState) -> Option<Rc<Listing>> {
        let MainView::Library(scope) = &s.frame.view else {
            return None;
        };
        let now = crate::palette::model::utc_from(s.svc.clock.as_ref());
        let mut cache = self.cache.borrow_mut();
        Some(listing::refresh(
            &mut cache,
            &s.model.library,
            scope,
            &self.filter.text,
            self.sort,
            now,
        ))
    }

    /// Ids of rows that arrived with the last library change, and whether that change was a
    /// bulk one that lands without motion.
    pub(crate) fn motion(&self) -> listing::Motion {
        self.cache
            .borrow()
            .as_ref()
            .map(|c| c.motion.clone())
            .unwrap_or_default()
    }

    /// What ⌘⌫ deletes: the multi-selection when it holds more than one row, else the open
    /// snippet (`selection()`).
    pub fn targets(&self, cursor: Option<Uuid>) -> Vec<Uuid> {
        self.selection.targets(cursor)
    }
}

/// Keeps the open snippet inside the list (`snippet-list.tsx`), drops the multi-selection on
/// another view and forgets deleted ids. True when the selection changed.
pub fn sync(s: &mut MainState) -> bool {
    let mut changed = false;
    if !s.library.started {
        s.library.started = true;
        if s.frame.selected_snippet.is_none() {
            if let Some(first) = s.model.library.snippets.first().map(|sn| sn.id) {
                s.select_snippet(Some(first));
                changed = true;
            }
        }
    }
    let view = s.frame.view.clone();
    let MainView::Library(scope) = &view else {
        return changed;
    };
    if s.library.seen_view.as_ref() != Some(&view) {
        if s.library.seen_view.is_some() {
            changed |= s.library.selection.clear();
            s.library.sort_open = false;
            s.library.move_open = false;
        }
        s.library.seen_view = Some(view.clone());
    }
    let Some(listing) = s.library.listing(s) else {
        return changed;
    };
    let exists = |id: Uuid| listing.indexed.position(id).is_some();
    // Deleted ids leave the multi-selection; only a new listing can have deleted any.
    if s.library.pruned != Some(listing.version) {
        s.library.pruned = Some(listing.version);
        s.library.selection.retain(exists);
    }
    let cursor = s.frame.selected_snippet;
    let next = keep_valid(cursor, &listing.ids, &s.library.filter.text, scope, exists);
    if next != cursor {
        s.select_snippet(next);
        changed = true;
    }
    changed
}

/// Scrolls the open snippet's row into view when it changed since the last time (the list's
/// `scrollToIndex` effect): a route, a new snippet or a sort can move it off screen. The poll
/// runs this; the list's own keys reveal as they move.
pub fn reveal(s: &mut MainState, tx: &mut Tx) {
    let cursor = s.frame.selected_snippet;
    if cursor == s.library.revealed || !s.frame.view.in_library() {
        return;
    }
    s.library.revealed = cursor;
    if let Some(id) = cursor {
        if s.library.listing(s).is_some_and(|l| l.ids.contains(&id)) {
            tx.scroll_into_view_in(SCROLL_KEY, row_key(id));
        }
    }
}

/// Starts the list over: the next [`sync`] opens the first snippet in library order again,
/// as a fresh window does. Scenes call it after they replace the library.
pub fn restart(s: &mut MainState) {
    s.library = State::default();
    s.select_snippet(None);
    s.refresh_library_snapshot();
}
