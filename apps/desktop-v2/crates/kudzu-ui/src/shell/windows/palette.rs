//! `palette`: Quick Search, a launcher panel (760×480, radius 18).
//! A `QuickSearchSession` captures the target before the panel takes keys and holds
//! suppression. `dismiss` is how the panel hides, except an insert, which moves the
//! session into the insert task before the window hides (a blur from hiding would
//! otherwise drop the guard).
use super::super::{theme_sync::ThemeSync, Shell};
use crate::palette::model::SearchModel;
use kudzu_app::assembly::Kudzu;
use kudzu_app::expansion::{ExpansionError, ExpansionService, QuickSearchSession};
use kudzu_app::library::LibraryService;
use kudzu_app::ports::Clock;
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use std::sync::Arc;

pub struct PaletteServices {
    pub settings: Arc<SettingsService>,
    pub library: Arc<LibraryService>,
    pub expansion: Arc<ExpansionService>,
    pub clock: Arc<dyn Clock>,
}

impl From<&Arc<Kudzu>> for PaletteServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            library: kudzu.library.clone(),
            expansion: kudzu.expansion.clone(),
            clock: kudzu.clipboard.clock(),
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
    pub search: SearchModel,
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
    /// An insert that has not settled, and is still inside its hold, keeps the panel hidden.
    pub fn toggle(&mut self) {
        if self.model.search.insert_in_flight(self.svc.clock.now()) {
            return;
        }
        if self.model.open {
            self.dismiss(true);
        } else {
            self.open();
        }
    }

    /// Start the capture (frontmost app, and the selection only when a snippet needs it),
    /// then show the panel on the next turn, after the hidden panel drew this state.
    fn open(&mut self) {
        self.model.search.reset(self.svc.clock.now());
        match self.svc.expansion.begin_quick_search() {
            Ok(task) => self.model.capture = Some(task),
            Err(error) => log::warn!("Quick Search could not start: {error}"),
        }
        self.model.open = true;
        let palette = self.shell.handles().palette.clone();
        self.shell.handles().palette.update(move |s, _| {
            if s.model.open {
                crate::palette::place(s);
                palette.show();
            }
        });
    }

    /// The way the palette hides, other than an insert. `restore` returns focus to the
    /// target (Escape, the shortcut again) unless `main` is focused. A blur just drops
    /// the session.
    pub fn dismiss(&mut self, restore: bool) {
        // `main` resigns as soon as this panel is key, so `main_focused` is false whenever
        // a key dismisses the palette. The flag stays as an S71 live check: a focused main
        // window still has to be able to suppress restoring the target.
        let restore = restore && !self.shell.main_focused();
        self.model.search.keys.blur();
        self.model.search.copy_due = None;
        self.model.search.pending_pick = None;
        self.model.search.held_list = None;
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
                Ok(Ok(session)) if self.model.open => {
                    self.model.session = Some(session);
                    self.model.search.restart_preview = true;
                }
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
        scenes: crate::palette::scenes::SCENES,
        setup: Some(crate::palette::setup),
        poll: Some(crate::palette::poll),
        on_key: Some(crate::palette::on_key),
        ..rune::AppSpec::new(
            "Quick Search",
            (760.0, 480.0),
            super::unreachable_init,
            crate::palette::view::view,
        )
    };
    rune::App::new(spec)
        .windowless()
        .modifier_keys()
        .on_focus(|s: &mut PaletteState, focused, _| {
            if !focused {
                s.model.search.keys.blur();
                s.dismiss(false);
            }
        })
        .on_visibility(|s: &mut PaletteState, _, _| s.shell.push_windows())
}

pub(super) fn spec(app: rune::App<PaletteState>, state: PaletteState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    super::finish("palette", app, &shell, state)
        .configured_options(rune::WindowOptions::launcher_panel().macos(|n| n.corner_radius(18.0)))
        .into()
}
