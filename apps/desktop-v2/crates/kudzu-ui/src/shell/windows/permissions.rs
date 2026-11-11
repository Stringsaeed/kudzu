//! `permissions`: the permission companion (420×660, radius 18, borderless, on top, on every
//! Space) and its click-through `GuidePanel` coordinator (`S66`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kudzu_app::assembly::Kudzu;
use kudzu_app::permissions::Permissions;
use kudzu_app::ports::PermissionKind;
use kudzu_app::settings::SettingsService;
use kudzu_app::tasks::Task;
use kudzu_app::watch::Revision;
use objc2_foundation::MainThreadMarker;
use rune::{Key, KeyEvent, Tx};

use super::super::{lifecycle::closes_windows, Shell};
use crate::guide::Guide;
use crate::permissions::model::PermissionsModel;

const STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);

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

pub struct PermissionsState {
    pub svc: PermissionsServices,
    pub shell: Shell,
    pub model: PermissionsModel,
    pub guide: Guide,
    /// Recorded `.app` paths revealed via click/Help for deterministic headless verification.
    pub revealed_paths: Vec<PathBuf>,
    seen_permissions: Option<Revision>,
    started: bool,
    restart: Option<Task<bool>>,
    refreshing: Option<Task<()>>,
    refresh_queued: bool,
    opening: Option<Task<(PermissionKind, Result<(), String>)>>,
    relaunching: Option<Task<Result<(), String>>>,
    pending_guide_wake: Option<Duration>,
    last_refresh: Option<Instant>,
}

impl PermissionsState {
    pub fn new(svc: PermissionsServices, shell: Shell) -> Self {
        let bundle = if shell.native() {
            MainThreadMarker::new().and_then(kudzu_appkit::app_bundle::running_bundle)
        } else {
            None
        };
        let model = PermissionsModel {
            drag_available: bundle.is_some(),
            bundle_path: bundle.clone(),
            ..PermissionsModel::default()
        };
        let guide = Guide::new(shell.native(), bundle);
        Self {
            svc,
            shell,
            model,
            guide,
            revealed_paths: Vec::new(),
            seen_permissions: None,
            started: false,
            restart: None,
            refreshing: None,
            refresh_queued: false,
            opening: None,
            relaunching: None,
            pending_guide_wake: None,
            last_refresh: None,
        }
    }

    /// Configures a mock `.app` bundle path in headless UI tests.
    pub fn set_bundle_for_test(&mut self, bundle: Option<PathBuf>) {
        self.model.drag_available = bundle
            .as_deref()
            .is_some_and(kudzu_appkit::app_bundle::is_app_bundle_path);
        self.model.bundle_path = bundle;
    }

    pub fn show(&mut self) {
        self.model.open = true;
        self.model.notice = None;
        self.spawn_refresh();
        self.shell.handles().permissions.open();
    }

    pub fn show_for(&mut self, kind: PermissionKind) {
        self.svc.permissions.select(kind);
        let _ = self.sync();
        self.show();
    }

    pub fn hide(&mut self) {
        self.stop_guide();
        if self.model.open {
            self.model.open = false;
            self.model.help_open = false;
            self.shell.handles().permissions.hide();
        }
    }

    /// Escape, header X, or "Back to Kudzu": hide companion and return focus to `main`.
    pub fn close(&mut self) {
        if self.model.help_open {
            self.model.help_open = false;
            return;
        }
        self.hide();
        self.shell.show_main();
    }

    pub fn stop_guide(&mut self) {
        self.guide.stop();
        self.pending_guide_wake = None;
        self.shell.set_guide_window(None);
    }

    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    pub fn spawn_refresh(&mut self) {
        if self.model.scene_locked {
            return;
        }
        if self.refreshing.is_some() {
            self.refresh_queued = true;
            return;
        }
        self.refresh_queued = false;
        self.last_refresh = Some(Instant::now());
        let permissions = self.svc.permissions.clone();
        let wake = self.shell.handles().permissions.clone();
        match Task::spawn("kudzu-permission-refresh", move || {
            permissions.refresh();
            wake.update(|_, _| {});
        }) {
            Ok(task) => self.refreshing = Some(task),
            Err(error) => log::warn!("permission status refresh failed to spawn: {error}"),
        }
    }

    pub fn spawn_begin(&mut self, kind: PermissionKind) {
        if self.model.scene_locked || self.opening.is_some() {
            return;
        }
        let permissions = self.svc.permissions.clone();
        let wake = self.shell.handles().permissions.clone();
        match Task::spawn("kudzu-permission-begin", move || {
            let res = permissions.begin(kind).map_err(|e| e.to_string());
            wake.update(|_, _| {});
            (kind, res)
        }) {
            Ok(task) => self.opening = Some(task),
            Err(error) => self.model.notice = Some(error.to_string()),
        }
    }

    pub fn spawn_restart(&mut self) {
        if self.model.scene_locked || self.relaunching.is_some() {
            return;
        }
        let permissions = self.svc.permissions.clone();
        let wake = self.shell.handles().permissions.clone();
        match Task::spawn("kudzu-permission-relaunch", move || {
            let res = permissions.request_restart().map_err(|e| e.to_string());
            wake.update(|_, _| {});
            res
        }) {
            Ok(task) => self.relaunching = Some(task),
            Err(error) => self.model.notice = Some(error.to_string()),
        }
    }

    pub fn sync(&mut self) -> bool {
        if self.model.scene_locked {
            return false;
        }
        let mut changed = self.model.theme.sync(&self.svc.settings.watch());

        let watch = self.svc.permissions.watch();
        let rev = watch.revision();
        if self.seen_permissions != Some(rev) {
            self.seen_permissions = Some(rev);
            self.model.snap = (*watch.get()).clone();
            self.model.loaded = true;
            if self.model.selected_granted() && self.guide.is_active() {
                self.stop_guide();
            }
            changed = true;
        }

        if !self.started {
            self.started = true;
            let permissions = self.svc.permissions.clone();
            let wake = self.shell.handles().permissions.clone();
            match Task::spawn("kudzu-permission-restart", move || {
                let pending = permissions.take_pending_restart();
                if pending {
                    wake.update(|_, _| {});
                }
                pending
            }) {
                Ok(task) => self.restart = Some(task),
                Err(error) => log::warn!("the permission restart marker was not read: {error}"),
            }
        }

        if let Some(result) = self.restart.as_ref().and_then(Task::take) {
            self.restart = None;
            if result.unwrap_or(false) {
                self.show();
                changed = true;
            }
        }

        if let Some(result) = self.refreshing.as_ref().and_then(Task::take) {
            self.refreshing = None;
            let _ = result;
            if self.refresh_queued {
                self.spawn_refresh();
            }
        }

        if let Some(result) = self.opening.as_ref().and_then(Task::take) {
            self.opening = None;
            changed = true;
            match result {
                Ok((kind, Ok(()))) => {
                    if self.model.open
                        && self.model.snap.selected == kind
                        && !self.model.selected_granted()
                    {
                        self.pending_guide_wake = self.guide.begin();
                    }
                }
                Ok((_, Err(err))) => self.model.notice = Some(err),
                Err(err) => self.model.notice = Some(err.to_string()),
            }
        }

        if let Some(result) = self.relaunching.as_ref().and_then(Task::take) {
            self.relaunching = None;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    self.model.notice = Some(err);
                    changed = true;
                }
                Err(err) => {
                    self.model.notice = Some(err.to_string());
                    changed = true;
                }
            }
        }

        if self.model.open
            && self.refreshing.is_none()
            && self
                .last_refresh
                .is_none_or(|t| t.elapsed() >= Duration::from_millis(950))
        {
            self.spawn_refresh();
        }

        changed
    }

    fn poll_frame(&mut self, tx: &mut Tx) -> bool {
        let changed = self.sync();
        if self.model.scene_locked {
            return changed;
        }

        let mut next_wake = self.pending_guide_wake.take();
        if self.guide.is_native() {
            let handle = self.shell.window("permissions");
            let outcome = self.guide.poll(
                self.model.open,
                self.model.selected_granted(),
                handle.as_ref(),
            );
            self.shell.set_guide_window(outcome.guide_window);
            if let Some(plan) = outcome.placement {
                let app_handle = self.shell.handles().permissions.clone();
                app_handle.update(move |state, inner_tx| {
                    if !state.guide.is_active() {
                        return;
                    }
                    Guide::request_rune_place(&state.shell.handles().permissions, plan.rune_rect);
                    let nested = state.shell.handles().permissions.clone();
                    nested.update(move |s2, tx2| {
                        let win = s2.shell.window("permissions");
                        if let Some(d) = s2.guide.finish_placement(plan, win.as_ref()) {
                            tx2.wake_after(d);
                        }
                    });
                    inner_tx.wake_after(Duration::from_millis(
                        kudzu_appkit::guide_panel::Lifecycle::PLACEMENT_TIMEOUT_MS,
                    ));
                });
            }
            next_wake = min_opt(next_wake, outcome.wake_after);
        }

        if self.model.open {
            next_wake = min_opt(next_wake, Some(STATUS_POLL_INTERVAL));
        }

        if let Some(delay) = next_wake {
            tx.wake_after(delay);
        }

        changed
    }
}

fn min_opt(a: Option<Duration>, b: Option<Duration>) -> Option<Duration> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (Some(x), None) => Some(x),
        (None, Some(y)) => Some(y),
        (None, None) => None,
    }
}

pub(super) fn app() -> rune::App<PermissionsState> {
    let spec = rune::AppSpec {
        fonts: crate::theme::fonts::FILES,
        families: crate::theme::fonts::families(),
        scenes: crate::permissions::scenes::SCENES,
        poll: Some(|s: &mut PermissionsState, _, tx: &mut Tx| s.poll_frame(tx)),
        on_key: Some(on_key),
        ..rune::AppSpec::new(
            "Permissions",
            (420.0, 660.0),
            super::unreachable_init,
            crate::permissions::view::render,
        )
    };
    rune::App::new(spec)
        .windowless()
        .keep_running()
        .on_close(|s: &mut PermissionsState, _| {
            s.stop_guide();
            s.model.open = false;
            s.model.help_open = false;
            s.shell.show_main();
        })
        .on_visibility(|s: &mut PermissionsState, _, _| s.shell.push_windows())
}

impl PermissionsState {
    /// Builds a headless `(Shell, PermissionsState)` pair for `rune::Runtime` integration tests.
    pub fn headless_for_test(kudzu: &Arc<Kudzu>, launch: super::Launch) -> (Shell, Self) {
        let p = super::parts(kudzu, launch, None);
        (p.shell, p.permissions)
    }

    /// Handles window-level shortcuts (`Escape`, `⌘Q` / `⌘W` window close).
    pub fn handle_key(&mut self, key: &KeyEvent) -> bool {
        if closes_windows(key) {
            self.stop_guide();
            self.shell.close_windows();
            return true;
        }
        if key.key == Key::Escape {
            self.close();
            return true;
        }
        false
    }
}

fn on_key(s: &mut PermissionsState, key: &KeyEvent, _: &mut Tx) -> bool {
    s.handle_key(key)
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
