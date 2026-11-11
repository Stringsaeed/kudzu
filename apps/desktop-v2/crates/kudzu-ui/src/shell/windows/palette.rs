//! `palette`: Quick Search, a launcher panel (760×480, radius 18). S56 builds its content.
//! The shell part (QS-1): a `QuickSearchSession` captures the target before the panel takes
//! keys and holds suppression; every way of hiding goes through `dismiss`, which drops it.
use super::super::{lifecycle::closes_windows, theme_sync::ThemeSync, Shell};
use kudzu_app::assembly::Kudzu;
use kudzu_app::expansion::{ExpansionError, ExpansionService, QuickSearchSession};
use kudzu_app::library::LibraryService;
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use rune::{Key, KeyEvent, Tx};
use std::sync::Arc;

pub struct PaletteServices {
    pub settings: Arc<SettingsService>,
    pub library: Arc<LibraryService>,
    pub expansion: Arc<ExpansionService>,
}

impl From<&Arc<Kudzu>> for PaletteServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            library: kudzu.library.clone(),
            expansion: kudzu.expansion.clone(),
        }
    }
}

type Capture = Task<Result<QuickSearchSession, ExpansionError>>;

#[derive(Default)]
pub struct PaletteModel {
    pub theme: ThemeSync,
    pub open: bool,
    /// The capture started on open; it becomes `session` in `poll`.
    pub(crate) capture: Option<Capture>,
    pub session: Option<QuickSearchSession>,
}

pub struct PaletteState {
    pub svc: PaletteServices,
    pub shell: Shell,
    pub model: PaletteModel,
}

impl PaletteState {
    pub fn new(svc: PaletteServices, shell: Shell) -> Self {
        Self {
            svc,
            shell,
            model: PaletteModel::default(),
        }
    }

    /// The Quick Search shortcut and the tray row.
    pub fn toggle(&mut self) {
        if self.model.open {
            self.dismiss(true);
        } else {
            self.open();
        }
    }

    /// Start the capture (frontmost app, and the selection only when a snippet needs it),
    /// then show the panel on the next turn, after the hidden panel drew this state.
    fn open(&mut self) {
        match self.svc.expansion.begin_quick_search() {
            Ok(task) => self.model.capture = Some(task),
            Err(error) => log::warn!("Quick Search could not start: {error}"),
        }
        self.model.open = true;
        let palette = self.shell.handles().palette.clone();
        self.shell.handles().palette.update(move |s, _| {
            if s.model.open {
                palette.show();
            }
        });
    }

    /// The one way the palette hides. `restore` returns focus to the target (Escape, the
    /// shortcut again); a blur or `main` taking over just drops the session.
    pub fn dismiss(&mut self, restore: bool) {
        // Dropping an unfinished capture drops its session, and so its guard, when it lands.
        self.model.capture = None;
        if let Some(session) = self.model.session.take() {
            if restore {
                match session.restore_target() {
                    Ok(task) => self.shell.settle("palette restore", task),
                    Err(error) => log::warn!("the target app was not restored: {error}"),
                }
            }
        }
        if self.model.open {
            self.model.open = false;
            self.shell.handles().palette.hide();
        }
    }

    pub fn sync(&mut self) -> bool {
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());
        if let Some(result) = self.model.capture.as_ref().and_then(|task| task.take()) {
            self.model.capture = None;
            match result {
                Ok(Ok(session)) if self.model.open => self.model.session = Some(session),
                Ok(Ok(_)) => {}
                Ok(Err(error)) => log::warn!("Quick Search capture failed: {error}"),
                Err(error) => log::warn!("Quick Search capture failed: {error}"),
            }
            changed = true;
        }
        changed
    }
}

pub(super) fn app() -> rune::App<PaletteState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        poll: Some(|s: &mut PaletteState, _, _| s.sync()),
        on_key: Some(on_key),
        ..rune::AppSpec::new(
            "Quick Search",
            (760.0, 480.0),
            super::unreachable_init,
            super::super::placeholder::palette,
        )
    };
    rune::App::new(spec)
        .windowless()
        .on_focus(|s: &mut PaletteState, focused, _| {
            if !focused {
                s.dismiss(false);
            }
        })
        .on_visibility(|s: &mut PaletteState, _, _| s.shell.push_windows())
}

fn on_key(s: &mut PaletteState, key: &KeyEvent, _: &mut Tx) -> bool {
    if closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    if key.key == Key::Escape {
        s.dismiss(true);
        return true;
    }
    false
}

pub(super) fn spec(app: rune::App<PaletteState>, state: PaletteState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    super::finish("palette", app, &shell, state)
        .configured_options(rune::WindowOptions::launcher_panel().macos(|n| n.corner_radius(18.0)))
        .into()
}
