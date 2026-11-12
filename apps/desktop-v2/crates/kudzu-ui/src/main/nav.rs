//! Navigation for the main window (`PLAN.md` S41): views, selection, breadcrumbs, the sidebar
//! slot, pane focus and the routes `Shell::open_main` sends. The frame owns this file. Screens
//! call these rather than writing `MainFrame` fields themselves.

use kudzu_core::KudzuLibrary;
use rune::Tx;
use uuid::Uuid;

use crate::main::library;
use crate::main::state::{ClipFilter, LibraryFilter, MainPane, MainView};
use crate::main::toolbar::compute_crumbs;
use crate::shell::{MainState, Route, SettingsPane};

impl MainState {
    /// The library snapshot `sync` keeps current.
    pub fn library_snapshot(&self) -> &KudzuLibrary {
        &self.model.library
    }

    /// Re-reads the library after this window changed it, and drops a selection that is gone.
    pub fn refresh_library_snapshot(&mut self) {
        let (rev, snap) = self.svc.library.watch().snapshot();
        self.model.seen_library = Some(rev);
        self.model.library = snap;
        if let Some(sel) = self.frame.selected_snippet {
            if !self.model.library.snippets.iter().any(|s| s.id == sel) {
                self.frame.selected_snippet = None;
            }
        }
        library::sync(self);
    }

    /// Feeds the leaf breadcrumb's stable id into [`crate::motion::Arrivals`], so mount,
    /// renames and `< 300ms` bursts stay still while distinct navigations animate.
    pub fn observe_breadcrumbs(&mut self) {
        let crumbs = compute_crumbs(
            &self.frame.view,
            self.frame.selected_snippet,
            &self.model.library,
        );
        if let Some(leaf) = crumbs.last() {
            let _ = self
                .frame
                .breadcrumb_arrivals
                .observe(&leaf.id, self.frame.clock_secs);
        }
    }

    /// Goes to `target`: history, `Focus<MainPane>` and breadcrumb arrivals.
    pub fn navigate_to(&mut self, target: MainView) {
        let entering_onboarding = matches!(target, MainView::Onboarding);
        self.frame.navigate(target);
        if entering_onboarding {
            self.clear_sidebar_material();
        }
        self.observe_breadcrumbs();
        library::sync(self);
    }

    /// Selects (or clears) a snippet and observes breadcrumb arrivals.
    pub fn select_snippet(&mut self, snippet_id: Option<Uuid>) {
        if self.frame.selected_snippet == snippet_id {
            return;
        }
        self.frame.selected_snippet = snippet_id;
        crate::main::editor::open_selected(self);
        self.observe_breadcrumbs();
    }

    /// Toggles the 224 pt sidebar slot (`⌘\` or the toolbar button). The subtree stays mounted,
    /// and focus never stays inside the hidden sidebar.
    pub fn toggle_sidebar(&mut self) {
        self.frame.sidebar_open = !self.frame.sidebar_open;
        self.frame.sync_focus_order();
    }

    /// Asks for `route`, as `Shell::open_main` does. The next poll applies it once, so the
    /// same route sent twice applies twice. `model.route` keeps the last one asked for.
    pub fn open_route(&mut self, route: Route) {
        self.model.route = route.clone();
        self.frame.pending_route = Some(route);
    }

    /// Applies a route `Shell::open_main` or a selection capture sent, and leaves Onboarding
    /// once `onboarding_completed` turns on. True when the frame changed.
    pub fn reconcile_route(&mut self) -> bool {
        let mut changed = false;

        if matches!(self.frame.view, MainView::Onboarding)
            && self.svc.settings.watch().get().onboarding_completed
        {
            let lib_view = MainView::Library(LibraryFilter::All);
            self.frame.history.replace_current(lib_view.clone());
            self.frame.view = lib_view;
            self.frame.sync_focus_order();
            if matches!(self.model.route, Route::Onboarding) {
                self.model.route = Route::Library;
            }
            self.observe_breadcrumbs();
            changed = true;
        }

        if let Some(route) = self.frame.pending_route.take() {
            match route {
                Route::Library => self.navigate_to(MainView::Library(LibraryFilter::All)),
                Route::Snippet(id) => {
                    self.navigate_to(MainView::Library(LibraryFilter::All));
                    self.select_snippet(Some(id));
                }
                // Tauri's `newSnippet` event: the snippet exists before the window shows it.
                Route::NewSnippet(seed) => {
                    let _ = library::actions::new_snippet(self, seed);
                }
                Route::Settings(pane) => self.navigate_to(MainView::Settings(pane)),
                Route::Clipboard => self.navigate_to(MainView::Clipboard(ClipFilter::All)),
                Route::Insights => self.navigate_to(MainView::Insights),
                Route::Onboarding => self.navigate_to(MainView::Onboarding),
            }
            changed = true;
        }
        changed
    }
}

/// `⌘1`–`⌘5`, the sidebar's fixed rows and the breadcrumbs: go to `target` and drop text focus.
pub fn view(s: &mut MainState, target: MainView, tx: &mut Tx) {
    s.navigate_to(target);
    s.blur_all_inputs();
    tx.blur();
}

/// `⌘,` always opens Settings at General (`main-app.tsx:129`); it never toggles back.
pub fn open_settings(s: &mut MainState, tx: &mut Tx) {
    view(s, MainView::Settings(SettingsPane::General), tx);
}

/// `⌘[`.
pub fn back(s: &mut MainState, tx: &mut Tx) {
    if s.frame.go_back() {
        s.observe_breadcrumbs();
    }
    library::sync(s);
    s.blur_all_inputs();
    tx.blur();
}

/// `⌘]`.
pub fn forward(s: &mut MainState, tx: &mut Tx) {
    if s.frame.go_forward() {
        s.observe_breadcrumbs();
    }
    library::sync(s);
    s.blur_all_inputs();
    tx.blur();
}

/// Moves keyboard focus to a list pane: drops text focus, then focuses the element keyed
/// `list_key` (`FocusList`, Escape from the editor, after a delete).
pub fn focus_list_pane(s: &mut MainState, list_key: &str, tx: &mut Tx) {
    s.blur_all_inputs();
    tx.blur();
    s.frame.focus.move_to(MainPane::List);
    tx.focus(list_key, false);
}

/// The first view of a route, for a window that opens on it.
pub(crate) fn view_for_route(route: &Route) -> MainView {
    match route {
        Route::Library | Route::Snippet(_) | Route::NewSnippet(_) => {
            MainView::Library(LibraryFilter::All)
        }
        Route::Settings(pane) => MainView::Settings(*pane),
        Route::Clipboard => MainView::Clipboard(ClipFilter::All),
        Route::Insights => MainView::Insights,
        Route::Onboarding => MainView::Onboarding,
    }
}

/// The snippet a route opens on, if any.
pub(crate) fn selected_for_route(route: &Route) -> Option<Uuid> {
    match route {
        Route::Snippet(id) => Some(*id),
        _ => None,
    }
}
