//! `main`: the library, editor and settings window. Closes to hidden and drops the Dock icon.
//! It also owns the menu-bar item and the app menu patch, which are main-thread AppKit objects.
use super::super::{routes::Route, theme_sync::ThemeSync, tray::Tray, Shell};
use kudzu_app::assembly::Kudzu;
use kudzu_app::expansion::ExpansionService;
use kudzu_app::hotkeys::{HotkeyError, HotkeySlots};
use kudzu_app::library::{LibraryService, SnippetDraft};
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use kudzu_app::updates::Updates;
use kudzu_appkit::app_menu::AppMenu;
use std::sync::Arc;

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
    pub(crate) hotkeys_seen: Option<HotkeyInputs>,
    pub(crate) started: bool,
    /// The appearance last sent to `NSApp.appearance` (recorded headless too).
    pub applied_appearance: Option<crate::theme::Appearance>,
    /// `main` opens at launch (not `--hidden`, or the first run).
    pub(crate) open_at_launch: bool,
    pub(crate) tray: Option<Tray>,
    pub(crate) menu: Option<AppMenu>,
}

pub struct MainState {
    pub svc: MainServices,
    pub shell: Shell,
    pub model: MainModel,
}

impl MainState {
    pub fn new(svc: MainServices, shell: Shell, open_at_launch: bool) -> Self {
        let onboarded = svc.settings.watch().get().onboarding_completed;
        let model = MainModel {
            open_at_launch,
            route: if onboarded {
                Route::Library
            } else {
                Route::Onboarding
            },
            ..MainModel::default()
        };
        Self { svc, shell, model }
    }
}

/// 1180×760, minimum 900×560, full-size content with inset traffic lights. Windowless when
/// launched `--hidden` after the first run.
pub(super) fn app(identity: super::super::Identity, show: bool) -> rune::App<MainState> {
    let spec = rune::AppSpec {
        min_size: Some((900.0, 560.0)),
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        poll: Some(super::super::lifecycle::poll),
        on_key: Some(super::super::lifecycle::on_key),
        ..rune::AppSpec::new(
            identity.app_name,
            (1180.0, 760.0),
            super::unreachable_init,
            super::super::placeholder::main,
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
        .on_close(super::super::lifecycle::on_close)
        .on_quit(super::super::lifecycle::on_quit)
        .on_reopen(super::super::lifecycle::on_reopen)
        .on_visibility(|s: &mut MainState, _, _| s.shell.push_windows());
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
