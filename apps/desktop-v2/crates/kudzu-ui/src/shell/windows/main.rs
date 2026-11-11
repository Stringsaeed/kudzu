//! `main`: the library, editor and settings window. Closes to hidden and drops the Dock icon.
//! It also owns the menu-bar item, the app menu patch, the sidebar vibrancy backdrop, and the
//! S41 window frame + per-screen state.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use kudzu_app::assembly::Kudzu;
use kudzu_app::expansion::ExpansionService;
use kudzu_app::hotkeys::{HotkeyError, HotkeySlots};
use kudzu_app::library::{LibraryService, SnippetDraft};
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use kudzu_app::updates::Updates;
use kudzu_app::watch::Revision;
use kudzu_appkit::app_menu::AppMenu;
use kudzu_appkit::geometry::Rect;
use kudzu_appkit::sidebar_material::SidebarMaterial;
use kudzu_appkit::window;
use kudzu_core::KudzuLibrary;
use objc2_foundation::MainThreadMarker;
use rune::raw_window_handle::RawWindowHandle;
use uuid::Uuid;

use super::super::{
    routes::{Route, SettingsPane},
    theme_sync::ThemeSync,
    tray::Tray,
    Shell,
};
use crate::actions::main::MainAction;
use crate::main::toolbar::compute_crumbs;
use crate::main::{
    clipboard, editor, insights, library, onboarding, settings, sidebar, ClipFilter, LibraryFilter,
    MainFrame, MainPane, MainView, ToastAction, ToastSpec, UNDO_TOAST_SECS,
};

/// The services `main` reads. S41 onward add what their screens need, here only.
pub struct MainServices {
    pub settings: Arc<SettingsService>,
    pub library: Arc<LibraryService>,
    pub expansion: Arc<ExpansionService>,
    pub updates: Updates,
    pub hotkeys: Arc<HotkeySlots>,
}

impl From<&Arc<Kudzu>> for MainServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            library: kudzu.library.clone(),
            expansion: kudzu.expansion.clone(),
            updates: kudzu.updates.clone(),
            hotkeys: kudzu.hotkeys.clone(),
        }
    }
}

/// The inputs `HotkeySlots::apply` depends on; a change re-applies (recording off unbinds).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HotkeyInputs {
    pub palette: String,
    pub clipboard: String,
    pub recording: bool,
}

#[derive(Default)]
pub struct MainModel {
    pub route: Route,
    pub theme: ThemeSync,
    /// The open editor's uncommitted draft. S51 replaces this with the editor's own; every
    /// path that hides `main` or quits commits it first. User content: never logged.
    pub draft: Option<SnippetDraft>,
    /// "New Snippet from Selection" in flight.
    pub capture: Option<Task<String>>,
    /// The last `HotkeySlots::apply` failure, for Settings (S61) to show.
    pub hotkey_error: Option<HotkeyError>,
    /// Cached library snapshot synchronized on revision changes in `MainState::sync`.
    pub library: Arc<KudzuLibrary>,
    pub(crate) seen_library: Option<Revision>,
    pub(crate) hotkeys_seen: Option<HotkeyInputs>,
    pub(crate) started: bool,
    /// The appearance last sent to `NSApp.appearance` (recorded headless too).
    pub applied_appearance: Option<crate::theme::Appearance>,
    /// `main` opens at launch (not `--hidden`, or the first run).
    pub(crate) open_at_launch: bool,
    pub(crate) tray: Option<Tray>,
    pub(crate) menu: Option<AppMenu>,
    pub(crate) sidebar_material: Option<SidebarMaterial>,
}

pub struct MainState {
    pub svc: MainServices,
    pub shell: Shell,
    pub model: MainModel,
    pub frame: MainFrame,
    pub sidebar: sidebar::State,
    pub library: library::State,
    pub editor: editor::State,
    pub clipboard: clipboard::State,
    pub settings: settings::State,
    pub insights: insights::State,
    pub onboarding: onboarding::State,
}

impl MainState {
    pub fn new(svc: MainServices, shell: Shell, open_at_launch: bool) -> Self {
        let onboarded = svc.settings.watch().get().onboarding_completed;
        let route = if onboarded {
            Route::Library
        } else {
            Route::Onboarding
        };
        let initial_view = view_for_route(&route);
        let initial_selected = selected_for_route(&route);
        let library_snap = svc.library.watch().get();
        let model = MainModel {
            open_at_launch,
            route: route.clone(),
            library: library_snap,
            ..MainModel::default()
        };
        let mut state = Self {
            svc,
            shell,
            model,
            frame: MainFrame::new(initial_view, route),
            sidebar: sidebar::State,
            library: library::State::default(),
            editor: editor::State::default(),
            clipboard: clipboard::State::default(),
            settings: settings::State,
            insights: insights::State,
            onboarding: onboarding::State,
        };
        state.frame.selected_snippet = initial_selected;
        state.observe_breadcrumbs();
        state
    }

    /// Latest cached library snapshot (`Arc<KudzuLibrary>`), kept synchronized by `MainState::sync`.
    pub fn library_snapshot(&self) -> &KudzuLibrary {
        &self.model.library
    }

    /// True if any text input in any screen currently has an active IME composition preedit.
    pub fn is_composing(&self) -> bool {
        self.sidebar.is_composing()
            || self.library.is_composing()
            || self.editor.is_composing()
            || self.clipboard.is_composing()
            || self.settings.is_composing()
            || self.insights.is_composing()
            || self.onboarding.is_composing()
    }

    /// True if any editable text input in the active screen currently holds keyboard focus.
    pub fn is_editing_text(&self) -> bool {
        self.sidebar.is_editing_text()
            || self.library.is_editing_text()
            || self.editor.is_editing_text()
            || self.clipboard.is_editing_text()
            || self.settings.is_editing_text()
            || self.insights.is_editing_text()
            || self.onboarding.is_editing_text()
    }

    /// Clears internal `focused` flags across screen text inputs when moving pane focus.
    pub fn blur_all_inputs(&mut self) {
        self.library.blur_inputs();
        self.editor.blur_inputs();
        self.clipboard.blur_inputs();
        self.settings.blur_inputs();
        self.onboarding.blur_inputs();
    }

    /// Refreshes the cached library snapshot directly from `LibraryService`.
    pub fn refresh_library_snapshot(&mut self) {
        let (rev, snap) = self.svc.library.watch().snapshot();
        self.model.seen_library = Some(rev);
        self.model.library = snap;
        if let Some(sel) = self.frame.selected_snippet {
            if !self.model.library.snippets.iter().any(|s| s.id == sel) {
                self.frame.selected_snippet = None;
            }
        }
    }

    /// Feeds the current leaf breadcrumb's stable ID into [`crate::motion::Arrivals`] so mount,
    /// in-place renames, and `< 300ms` bursts stay still while distinct navigations animate.
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

    /// Navigates to `target`, recording history, updating `Focus<MainPane>`, and observing
    /// breadcrumb arrivals.
    pub fn navigate_to(&mut self, target: MainView) {
        let entering_onboarding = matches!(target, MainView::Onboarding);
        self.frame.navigate(target);
        if entering_onboarding {
            self.clear_sidebar_material();
        }
        self.observe_breadcrumbs();
    }

    /// Selects (or clears) a snippet in the Library and observes breadcrumb arrivals.
    pub fn select_snippet(&mut self, snippet_id: Option<Uuid>) {
        if self.frame.selected_snippet == snippet_id {
            return;
        }
        self.frame.selected_snippet = snippet_id;
        if let Some(id) = snippet_id {
            if let Some(sn) = self.model.library.snippets.iter().find(|s| s.id == id) {
                self.editor.content = rune::TextEdit::new(&sn.content);
            }
        }
        self.observe_breadcrumbs();
    }

    /// Toggles the 224pt sidebar slot (`⌘\` or toolbar button), keeping the subtree mounted and
    /// adjusting `Focus<MainPane>` so focus never remains trapped inside the hidden sidebar.
    pub fn toggle_sidebar(&mut self) {
        self.frame.sidebar_open = !self.frame.sidebar_open;
        self.frame.sync_focus_order();
    }

    /// Synchronizes the native `NSVisualEffectView` sidebar backdrop with the drawn sidebar slot
    /// rect (`[x, y, width, height]`). No-op in headless runs.
    pub fn sync_sidebar_material(&mut self, rect: [f64; 4]) {
        self.frame.sidebar_material_rect = Some(rect);
        if !self.shell.native() {
            return;
        }
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let Some(material) = &self.model.sidebar_material else {
            return;
        };
        let Some(handle) = self.shell.window("main") else {
            return;
        };
        let drawn = Rect {
            x: rect[0],
            y: rect[1],
            width: rect[2],
            height: rect[3],
        };
        let _ = handle.with_raw_handle(|raw| {
            let RawWindowHandle::AppKit(appkit) = raw.as_raw() else {
                return;
            };
            // SAFETY: Rune's scoped `with_raw_handle` borrow keeps `ns_view` alive on this main thread.
            unsafe {
                window::with_raw_view(mtm, appkit.ns_view, |win| {
                    let _ = material.sync(win, drawn);
                });
            }
        });
    }

    /// Detaches the native sidebar vibrancy view (used when entering chromeless Onboarding).
    pub fn clear_sidebar_material(&mut self) {
        self.frame.sidebar_material_rect = None;
        if let Some(material) = &self.model.sidebar_material {
            material.remove();
        }
    }

    /// Reconciles external `Shell::open_main(route)` or selection-capture updates into `MainFrame`
    /// without clobbering `self.model.route`.
    pub fn reconcile_route(&mut self) -> bool {
        let mut changed = false;

        // Auto-advance out of Onboarding once `onboarding_completed` flips to `true`.
        if matches!(self.frame.view, MainView::Onboarding)
            && self.svc.settings.watch().get().onboarding_completed
        {
            let lib_view = MainView::Library(LibraryFilter::All);
            self.frame.history.replace_current(lib_view.clone());
            self.frame.view = lib_view;
            self.frame.sync_focus_order();
            if matches!(self.model.route, Route::Onboarding) {
                self.model.route = Route::Library;
                self.frame.synced_route = Route::Library;
            }
            self.observe_breadcrumbs();
            changed = true;
        }

        if self.frame.synced_route != self.model.route {
            let route = self.model.route.clone();
            self.frame.synced_route = route.clone();
            match route {
                Route::Library => self.navigate_to(MainView::Library(LibraryFilter::All)),
                Route::Snippet(id) => {
                    self.navigate_to(MainView::Library(LibraryFilter::All));
                    self.select_snippet(Some(id));
                }
                Route::NewSnippet(ns) => {
                    self.navigate_to(MainView::Library(LibraryFilter::All));
                    self.editor.content = rune::TextEdit::new(&ns.content);
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

    /// Deletes `ids` through `LibraryService::delete_snippets` and posts a 6-second Undo toast
    /// (`LIB-7`). Deleting the currently open snippet clears selection and moves focus back to
    /// `MainPane::List`.
    pub fn delete_snippets_with_undo(&mut self, ids: &[Uuid]) -> bool {
        if ids.is_empty() {
            return false;
        }
        let Ok((deleted, _)) = self.svc.library.delete_snippets(ids) else {
            return false;
        };
        if deleted.snippets.is_empty() {
            return false;
        }
        let count = deleted.snippets.len();
        let single_name = (count == 1).then(|| {
            let n = deleted.snippets[0].name.trim();
            if n.is_empty() {
                "Untitled".to_owned()
            } else {
                n.to_owned()
            }
        });

        let deleted_open = self
            .frame
            .selected_snippet
            .is_some_and(|sel| ids.contains(&sel));
        self.refresh_library_snapshot();
        if deleted_open {
            self.frame.selected_snippet = None;
            self.blur_all_inputs();
            self.frame.focus.move_to(MainPane::List);
        }
        self.observe_breadcrumbs();

        let title = match single_name {
            Some(name) => format!("Deleted “{name}”"),
            None => format!("Deleted {count} snippets"),
        };
        let slot = Rc::new(RefCell::new(Some(deleted)));
        let now = self.frame.clock_secs;
        let spec = ToastSpec::new(title)
            .duration_secs(UNDO_TOAST_SECS)
            .action(ToastAction::new("Undo", true, move |st: &mut MainState| {
                if let Some(batch) = slot.borrow_mut().take() {
                    if let Ok((restored, _)) = st.svc.library.restore(batch) {
                        st.refresh_library_snapshot();
                        if let Some(&first) = restored.first() {
                            st.select_snippet(Some(first));
                        }
                    }
                }
            }));
        let key = format!("undo-delete:{}", ids[0]);
        self.frame.toasts.show(key, spec, now);
        true
    }

    /// Executes a [`MainAction`] invoked by shortcut or command menu (`LIB-14`).
    pub fn dispatch_action(&mut self, action: MainAction) {
        match action {
            MainAction::CommandMenu => {
                self.frame.command_open = !self.frame.command_open;
            }
            MainAction::NewSnippet => {
                let group_id = match self.frame.view {
                    MainView::Library(LibraryFilter::Group(gid)) => Some(gid),
                    _ => None,
                };
                if !self.frame.view.in_library() {
                    self.navigate_to(MainView::Library(LibraryFilter::All));
                }
                let draft = SnippetDraft {
                    group_id,
                    ..SnippetDraft::default()
                };
                if let Ok((created, _)) = self.svc.library.create_snippet(draft) {
                    self.refresh_library_snapshot();
                    self.select_snippet(Some(created.id));
                }
            }
            MainAction::NewGroup => {
                let parent_id = match self.frame.view {
                    MainView::Library(LibraryFilter::Group(gid)) => Some(gid),
                    _ => None,
                };
                let existing = self.model.library.groups.len();
                let name = if existing == 0 {
                    "New group".to_owned()
                } else {
                    format!("New group {}", existing + 1)
                };
                if let Ok((group, _)) = self.svc.library.create_group(name, parent_id) {
                    self.refresh_library_snapshot();
                    self.navigate_to(MainView::Library(LibraryFilter::Group(group.id)));
                }
            }
            MainAction::FocusFilter => {
                if !matches!(
                    self.frame.view,
                    MainView::Library(_) | MainView::Clipboard(_)
                ) {
                    self.navigate_to(MainView::Library(LibraryFilter::All));
                }
            }
            MainAction::FocusList | MainAction::EditContent => {}
            MainAction::DuplicateSnippet => {
                if let Some(id) = self.frame.selected_snippet {
                    if let Ok((copy, _)) = self.svc.library.duplicate_snippet(id) {
                        self.refresh_library_snapshot();
                        self.select_snippet(Some(copy.id));
                    }
                }
            }
            MainAction::ToggleFavorite => {
                if let Some(id) = self.frame.selected_snippet {
                    if let Some(mut sn) = self
                        .model
                        .library
                        .snippets
                        .iter()
                        .find(|s| s.id == id)
                        .cloned()
                    {
                        sn.is_favorite = !sn.is_favorite;
                        if self.svc.library.update_snippet(sn).is_ok() {
                            self.refresh_library_snapshot();
                        }
                    }
                }
            }
            MainAction::ToggleEnabled => {
                if let Some(id) = self.frame.selected_snippet {
                    if let Some(mut sn) = self
                        .model
                        .library
                        .snippets
                        .iter()
                        .find(|s| s.id == id)
                        .cloned()
                    {
                        sn.is_enabled = !sn.is_enabled;
                        if self.svc.library.update_snippet(sn).is_ok() {
                            self.refresh_library_snapshot();
                        }
                    }
                }
            }
            MainAction::DeleteSnippet => {
                if let Some(id) = self.frame.selected_snippet {
                    self.delete_snippets_with_undo(&[id]);
                }
            }
            MainAction::TogglePause => {
                let paused = !self.svc.expansion.status().get().paused;
                let _ = self.svc.expansion.set_paused(paused);
            }
            MainAction::ViewAll => self.navigate_to(MainView::Library(LibraryFilter::All)),
            MainAction::ViewFavorites => {
                self.navigate_to(MainView::Library(LibraryFilter::Favorites));
            }
            MainAction::ViewRecent => self.navigate_to(MainView::Library(LibraryFilter::Recent)),
            MainAction::ViewInsights => self.navigate_to(MainView::Insights),
            MainAction::ViewClipboard => self.navigate_to(MainView::Clipboard(ClipFilter::All)),
            MainAction::OpenSettings => {
                let target = match self.frame.view {
                    MainView::Settings(_) => self.frame.prior_non_settings.clone(),
                    _ => MainView::Settings(SettingsPane::General),
                };
                self.navigate_to(target);
            }
            MainAction::OpenShortcuts => {
                self.frame.shortcuts_open = !self.frame.shortcuts_open;
            }
            MainAction::Back => {
                if self.frame.go_back() {
                    self.observe_breadcrumbs();
                }
            }
            MainAction::Forward => {
                if self.frame.go_forward() {
                    self.observe_breadcrumbs();
                }
            }
            MainAction::ToggleSidebar => {
                self.toggle_sidebar();
            }
        }
    }
}

fn view_for_route(route: &Route) -> MainView {
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

fn selected_for_route(route: &Route) -> Option<Uuid> {
    match route {
        Route::Snippet(id) => Some(*id),
        _ => None,
    }
}

/// 1180×760, minimum 900×560, full-size content with inset traffic lights. Windowless when
/// launched `--hidden` after the first run.
pub(crate) fn app(identity: super::super::Identity, show: bool) -> rune::App<MainState> {
    let spec = rune::AppSpec {
        min_size: Some((900.0, 560.0)),
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        scenes: crate::main::scenes::SCENES,
        poll: Some(super::super::lifecycle::poll),
        on_key: Some(super::super::lifecycle::on_key),
        ..rune::AppSpec::new(
            identity.app_name,
            (1180.0, 760.0),
            super::unreachable_init,
            crate::main::view,
        )
    };
    let app = rune::App::new(spec)
        .keep_running()
        .standard_menus()
        .chrome(rune::Chrome {
            full_size_content: true,
            traffic_lights: Some((18.0, 22.0)),
            drag_regions: true,
        })
        .on_focus(|s: &mut MainState, focused, _| s.shell.set_main_focused(focused))
        .on_close(super::super::lifecycle::on_close)
        .on_quit(super::super::lifecycle::on_quit)
        .on_reopen(super::super::lifecycle::on_reopen)
        .on_visibility(|s: &mut MainState, visible, _| {
            s.frame
                .toasts
                .set_window_visible(visible, s.frame.clock_secs);
            s.shell.push_windows();
        });
    if show {
        app
    } else {
        app.windowless()
    }
}

pub(super) fn spec(app: rune::App<MainState>, state: MainState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    super::finish("main", app, &shell, state)
        .options(rune::WindowOptions {
            transparent: true,
            ..Default::default()
        })
        .into()
}
