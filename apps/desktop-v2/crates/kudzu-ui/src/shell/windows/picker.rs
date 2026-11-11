//! `picker`: clipboard history, a launcher panel (800×520, radius 18). S60 builds its content.
//! Unavailable while recording is off, and hidden if recording turns off while it is open.
use super::super::{lifecycle::closes_windows, theme_sync::ThemeSync, Shell};
use kudzu_app::assembly::Kudzu;
use kudzu_app::clipboard::{ClipboardError, ClipboardService, PickSession};
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use rune::{Key, KeyEvent, Tx};
use std::sync::Arc;

pub struct PickerServices {
    pub settings: Arc<SettingsService>,
    pub clipboard: Arc<ClipboardService>,
}

impl From<&Arc<Kudzu>> for PickerServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            clipboard: kudzu.clipboard.clone(),
        }
    }
}

#[derive(Default)]
pub struct PickerModel {
    pub theme: ThemeSync,
    pub open: bool,
    pub(crate) begin: Option<Task<Result<PickSession, ClipboardError>>>,
    pub session: Option<PickSession>,
}

pub struct PickerState {
    pub svc: PickerServices,
    pub shell: Shell,
    pub model: PickerModel,
}

impl PickerState {
    pub fn new(svc: PickerServices, shell: Shell) -> Self {
        Self {
            svc,
            shell,
            model: PickerModel::default(),
        }
    }

    fn recording(&self) -> bool {
        self.svc.settings.watch().get().clipboard_enabled
    }

    pub fn toggle(&mut self) {
        if self.model.open {
            self.dismiss();
        } else if self.recording() {
            self.open();
        }
    }

    /// `begin_pick` reads the frontmost app, so it runs on a worker.
    fn open(&mut self) {
        let clipboard = self.svc.clipboard.clone();
        match Task::spawn("kudzu-picker-begin", move || clipboard.begin_pick()) {
            Ok(task) => self.model.begin = Some(task),
            Err(error) => log::warn!("the clipboard picker could not start: {error}"),
        }
        self.model.open = true;
        let picker = self.shell.handles().picker.clone();
        self.shell.handles().picker.update(move |s, _| {
            if s.model.open {
                picker.show();
            }
        });
    }

    /// The one way the picker hides.
    pub fn dismiss(&mut self) {
        self.model.begin = None;
        self.model.session = None;
        if self.model.open {
            self.model.open = false;
            self.shell.handles().picker.hide();
        }
    }

    pub fn sync(&mut self) -> bool {
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());
        if self.model.open && !self.recording() {
            self.dismiss();
            changed = true;
        }
        if let Some(result) = self.model.begin.as_ref().and_then(|task| task.take()) {
            self.model.begin = None;
            match result {
                Ok(Ok(session)) if self.model.open => self.model.session = Some(session),
                Ok(Ok(_)) => {}
                Ok(Err(error)) => log::warn!("the clipboard picker could not start: {error}"),
                Err(error) => log::warn!("the clipboard picker could not start: {error}"),
            }
            changed = true;
        }
        changed
    }
}

pub(super) fn app() -> rune::App<PickerState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        poll: Some(|s: &mut PickerState, _, _| s.sync()),
        on_key: Some(on_key),
        ..rune::AppSpec::new(
            "Clipboard History",
            (800.0, 520.0),
            super::unreachable_init,
            super::super::placeholder::picker,
        )
    };
    rune::App::new(spec)
        .windowless()
        .on_focus(|s: &mut PickerState, focused, _| {
            if !focused {
                s.dismiss();
            }
        })
        .on_visibility(|s: &mut PickerState, _, _| s.shell.push_windows())
}

fn on_key(s: &mut PickerState, key: &KeyEvent, _: &mut Tx) -> bool {
    if closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    if key.key == Key::Escape {
        s.dismiss();
        return true;
    }
    false
}

pub(super) fn spec(app: rune::App<PickerState>, state: PickerState) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    super::finish("picker", app, &shell, state)
        .configured_options(rune::WindowOptions::launcher_panel().macos(|n| n.corner_radius(18.0)))
        .into()
}
