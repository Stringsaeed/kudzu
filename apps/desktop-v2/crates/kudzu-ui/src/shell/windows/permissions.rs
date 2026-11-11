//! `permissions`: the permission companion (420×660, radius 18, borderless, on top, on every
//! Space). S66 builds it and its guide. It reopens itself after an explicit restart.
use super::super::{lifecycle::closes_windows, theme_sync::ThemeSync, Shell};
use kudzu_app::assembly::Kudzu;
use kudzu_app::permissions::Permissions;
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use rune::{Key, KeyEvent, Tx};
use std::sync::Arc;

pub struct PermissionsServices {
    pub settings: Arc<SettingsService>,
    pub permissions: Arc<Permissions>,
}

impl From<&Arc<Kudzu>> for PermissionsServices {
    fn from(kudzu: &Arc<Kudzu>) -> Self {
        Self {
            settings: kudzu.settings.clone(),
            permissions: kudzu.permissions.clone(),
        }
    }
}

#[derive(Default)]
pub struct PermissionsModel {
    pub theme: ThemeSync,
    pub open: bool,
    /// The restart marker is a file: it is read once, on a worker.
    pub(crate) restart: Option<Task<bool>>,
    started: bool,
}

pub struct PermissionsState {
    pub svc: PermissionsServices,
    pub shell: Shell,
    pub model: PermissionsModel,
}

impl PermissionsState {
    pub fn new(svc: PermissionsServices, shell: Shell) -> Self {
        Self {
            svc,
            shell,
            model: PermissionsModel::default(),
        }
    }

    pub fn show(&mut self) {
        self.model.open = true;
        self.shell.handles().permissions.open();
    }

    pub fn hide(&mut self) {
        if self.model.open {
            self.model.open = false;
            self.shell.handles().permissions.hide();
        }
    }

    /// Escape or close: hide and show `main`, as the Tauri companion did.
    fn close(&mut self) {
        self.hide();
        self.shell.show_main();
    }

    pub fn sync(&mut self) -> bool {
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());
        if !self.model.started {
            self.model.started = true;
            let permissions = self.svc.permissions.clone();
            match Task::spawn("kudzu-permission-restart", move || {
                permissions.take_pending_restart()
            }) {
                Ok(task) => self.model.restart = Some(task),
                Err(error) => log::warn!("the permission restart marker was not read: {error}"),
            }
        }
        if let Some(result) = self.model.restart.as_ref().and_then(|task| task.take()) {
            self.model.restart = None;
            if result.unwrap_or(false) {
                self.show();
                changed = true;
            }
        }
        changed
    }
}

pub(super) fn app() -> rune::App<PermissionsState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        poll: Some(|s: &mut PermissionsState, _, _| s.sync()),
        on_key: Some(on_key),
        ..rune::AppSpec::new(
            "Permissions",
            (420.0, 660.0),
            super::unreachable_init,
            super::super::placeholder::permissions,
        )
    };
    rune::App::new(spec)
        .windowless()
        .keep_running()
        .on_close(|s: &mut PermissionsState, _| {
            s.model.open = false;
            s.shell.show_main();
        })
        .on_visibility(|s: &mut PermissionsState, _, _| s.shell.push_windows())
}

fn on_key(s: &mut PermissionsState, key: &KeyEvent, _: &mut Tx) -> bool {
    if closes_windows(key) {
        s.shell.close_windows();
        return true;
    }
    if key.key == Key::Escape {
        s.close();
        return true;
    }
    false
}

/// NSWindowCollectionBehaviorCanJoinAllSpaces | FullScreenAuxiliary.
const ALL_SPACES: usize = (1 << 0) | (1 << 8);

pub(super) fn spec(
    app: rune::App<PermissionsState>,
    state: PermissionsState,
) -> rune::AnyWindowSpec {
    let shell = state.shell.clone();
    let options = rune::ConfiguredWindowOptions::from(rune::WindowOptions {
        borderless: true,
        transparent: true,
        always_on_top: true,
        resizable: false,
        visible: false,
        ..Default::default()
    })
    .macos(|n| n.corner_radius(18.0).collection_behavior(ALL_SPACES));
    super::finish("permissions", app, &shell, state)
        .configured_options(options)
        .into()
}
