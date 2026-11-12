//! `main`: the library, editor and settings window. Closes to hidden and drops the Dock icon.
//! It also owns the menu-bar item, the app menu patch and the sidebar vibrancy backdrop.
//!
//! This file holds the services, the window state and the window spec only. Each screen's
//! logic lives in its module under `crate::main` (`nav`, `keys`, `library`, `sidebar`, …).

use std::sync::Arc;

use kudzu_app::assembly::Kudzu;
use kudzu_app::clipboard::ClipboardService;
use kudzu_app::expansion::ExpansionService;
use kudzu_app::hotkeys::{HotkeyError, HotkeySlots};
use kudzu_app::library::{LibraryService, SnippetDraft};
use kudzu_app::ports::Clock;
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

use super::super::{routes::Route, theme_sync::ThemeSync, tray::Tray, Shell};
use crate::main::nav::{selected_for_route, view_for_route};
use crate::main::{clipboard, editor, insights, library, onboarding, settings, sidebar, MainFrame};

/// The services `main` reads. S41 onward add what their screens need, here only.
pub struct MainServices {
    pub onboarding: Arc<kudzu_app::onboarding::Onboarding>,
    pub data: kudzu_app::data::DataService,
    pub settings: Arc<SettingsService>,
    pub library: Arc<LibraryService>,
    pub expansion: Arc<ExpansionService>,
    pub updates: Updates,
    pub hotkeys: Arc<HotkeySlots>,
    pub clipboard: Arc<ClipboardService>,
    pub clock: Arc<dyn Clock>,
}

impl From<&Arc<Kudzu>> for MainServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            onboarding: kudzu.onboarding.clone(),
            data: kudzu.data.clone(),
            settings: kudzu.settings.clone(),
            library: kudzu.library.clone(),
            expansion: kudzu.expansion.clone(),
            updates: kudzu.updates.clone(),
            hotkeys: kudzu.hotkeys.clone(),
            clipboard: kudzu.clipboard.clone(),
            clock: kudzu.clock.clone(),
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
            route,
            library: library_snap,
            ..MainModel::default()
        };
        let mut state = Self {
            svc,
            shell,
            model,
            frame: MainFrame::new(initial_view),
            sidebar: sidebar::State::default(),
            library: library::State::default(),
            editor: editor::State::default(),
            clipboard: clipboard::State::default(),
            settings: settings::State::default(),
            insights: insights::State::default(),
            onboarding: onboarding::State::default(),
        };
        state.frame.selected_snippet = initial_selected;
        state.observe_breadcrumbs();
        state
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
