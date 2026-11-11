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

struct TrackedTask<T> {
    task: Task<T>,
    _sub: kudzu_app::watch::Subscription,
}

impl<T: Send + 'static> TrackedTask<T> {
    fn spawn(
        name: &str,
        waker: &Arc<dyn Fn() + Send + Sync>,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<Self, kudzu_app::tasks::TaskError> {
        let task = Task::spawn(name, work)?;
        let ready = task.ready();
        let on_ready = waker.clone();
        let sub = ready.subscribe(move || on_ready());
        if *ready.get() {
            waker();
        }
        Ok(Self { task, _sub: sub })
    }

    fn take(&self) -> Option<Result<T, kudzu_app::tasks::TaskError>> {
        self.task.take()
    }

    #[cfg(test)]
    fn ready(&self) -> kudzu_app::watch::Watch<bool> {
        self.task.ready()
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
    restart: Option<TrackedTask<bool>>,
    refreshing: Option<TrackedTask<()>>,
    refresh_queued: bool,
    opening: Option<TrackedTask<(PermissionKind, Result<(), String>)>>,
    relaunching: Option<TrackedTask<Result<(), String>>>,
    pending_guide_wake: Option<Duration>,
    last_refresh: Option<Instant>,
    waker: Arc<dyn Fn() + Send + Sync>,
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
        let handle = shell.handles().permissions.clone();
        let waker = Arc::new(move || handle.wake());
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
            waker,
        }
    }

    /// Configures a mock `.app` bundle path in headless UI tests.
    pub fn set_bundle_for_test(&mut self, bundle: Option<PathBuf>) {
        self.model.drag_available = bundle
            .as_deref()
            .is_some_and(kudzu_appkit::app_bundle::is_app_bundle_path);
        self.model.bundle_path = bundle.clone();
        self.guide.set_bundle(bundle);
    }

    pub fn focused(&mut self, focused: bool) {
        if focused && self.model.open && !self.model.scene_locked {
            self.spawn_refresh();
        }
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
        match TrackedTask::spawn("kudzu-permission-refresh", &self.waker, move || {
            permissions.refresh();
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
        match TrackedTask::spawn("kudzu-permission-begin", &self.waker, move || {
            let res = permissions.begin(kind).map_err(|e| e.to_string());
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
        match TrackedTask::spawn("kudzu-permission-relaunch", &self.waker, move || {
            permissions.request_restart().map_err(|e| e.to_string())
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

        let (rev, snap) = self.svc.permissions.watch().snapshot();
        if self.seen_permissions != Some(rev) {
            self.seen_permissions = Some(rev);
            self.model.snap = (*snap).clone();
            self.model.loaded = true;
            if self.model.selected_granted() && self.guide.is_active() {
                self.stop_guide();
            }
            changed = true;
        }

        if !self.started {
            self.started = true;
            let permissions = self.svc.permissions.clone();
            match TrackedTask::spawn("kudzu-permission-restart", &self.waker, move || {
                permissions.take_pending_restart()
            }) {
                Ok(task) => self.restart = Some(task),
                Err(error) => log::warn!("the permission restart marker was not read: {error}"),
            }
        }

        if let Some(result) = self.restart.as_ref().and_then(TrackedTask::take) {
            self.restart = None;
            if result.unwrap_or(false) {
                self.show();
                changed = true;
            }
        }

        if let Some(result) = self.refreshing.as_ref().and_then(TrackedTask::take) {
            self.refreshing = None;
            let _ = result;
            if self.refresh_queued {
                self.spawn_refresh();
            }
        }

        if let Some(result) = self.opening.as_ref().and_then(TrackedTask::take) {
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

        if let Some(result) = self.relaunching.as_ref().and_then(TrackedTask::take) {
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
        .on_focus(|s: &mut PermissionsState, focused, _| s.focused(focused))
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{mpsc, Arc, Mutex};
    use std::time::Duration;

    use kudzu_app::assembly::{Config, Host, Kudzu};
    use kudzu_app::error::PortError;
    use kudzu_app::fixtures::Fixtures;
    use kudzu_app::paths::Paths;
    use kudzu_app::ports::{PermissionKind, SettingsPane, SystemSettings};
    use kudzu_app::settings::Variant;
    use kudzu_core::system::PermissionStatus;

    use super::PermissionsState;
    use crate::shell::{Identity, Launch};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("kudzu-perm-task-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct GateSettings {
        gate: Mutex<Option<mpsc::Receiver<()>>>,
        panic_on_open: AtomicBool,
    }

    impl SystemSettings for GateSettings {
        fn permissions(&self) -> PermissionStatus {
            PermissionStatus::default()
        }

        fn request(&self, _: PermissionKind) -> Result<(), PortError> {
            Ok(())
        }

        fn open(&self, _: SettingsPane) -> Result<(), PortError> {
            if let Some(wait) = self.gate.lock().unwrap().take() {
                let _ = wait.recv();
            }
            assert!(
                !self.panic_on_open.load(Ordering::SeqCst),
                "simulated open panic"
            );
            Ok(())
        }
    }

    #[test]
    fn task_wakes_only_after_result_is_ready_and_wakes_on_panic() {
        let scratch = Scratch::new();
        let fixtures = Fixtures::new().unwrap();
        let controlled = Arc::new(GateSettings::default());
        let mut host = Host::fixtures(&fixtures, scratch.0.clone());
        host.ports.settings = controlled.clone();

        let kudzu = Kudzu::new(Config {
            paths: Paths {
                data: scratch.0.clone(),
                logs: scratch.0.join("logs"),
            },
            host,
            variant: Variant::Dev,
            version: "0.7.0".into(),
            app_name: "Kudzu".into(),
            bundle_id: "sh.saeed.kudzu".into(),
            os_major: 26,
            mcp_name: "kudzu".into(),
            executable: scratch.0.join("Kudzu"),
            agent_args: vec!["--mcp".into()],
            bundled_skill_dir: None,
            production: false,
            cloud_and_updates: false,
            debounce: Duration::from_millis(50),
            retry: Duration::from_millis(50),
        })
        .unwrap();

        let (_shell, mut state) = PermissionsState::headless_for_test(
            &kudzu,
            Launch {
                identity: Identity {
                    app_name: "Kudzu",
                    dev: true,
                },
                native: false,
                hidden: false,
            },
        );
        state.started = true;
        let _ = state.sync();

        // 1. Wakeup must fire strictly AFTER `Task::ready()` / `Task::take()` is populated.
        let (release_tx, release_rx) = mpsc::channel();
        *controlled.gate.lock().unwrap() = Some(release_rx);

        let shared_watch: Arc<Mutex<Option<kudzu_app::watch::Watch<bool>>>> =
            Arc::new(Mutex::new(None));
        let (wake_tx, wake_rx) = mpsc::channel();
        let watch_for_wake = shared_watch.clone();
        state.waker = Arc::new(move || {
            let ready = watch_for_wake
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|w| *w.get());
            let _ = wake_tx.send(ready);
        });

        state.spawn_begin(PermissionKind::Accessibility);
        *shared_watch.lock().unwrap() = state.opening.as_ref().map(|t| t.ready());
        release_tx.send(()).unwrap();

        let ready_at_wake = wake_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("background task completion must wake the companion");
        assert!(
            ready_at_wake,
            "wakeup fired before Task stored its result (race with Task::take())"
        );
        assert!(state.sync());
        assert!(
            !state.is_opening(),
            "first sync() after wakeup must consume the finished task"
        );

        // 2. A panicking task must still wake the companion so `opening` never gets wedged.
        controlled.panic_on_open.store(true, Ordering::SeqCst);
        let (panic_tx, panic_rx) = mpsc::channel();
        state.waker = Arc::new(move || {
            let _ = panic_tx.send(());
        });
        state.spawn_begin(PermissionKind::Accessibility);
        panic_rx
            .recv_timeout(Duration::from_milliseconds_compat())
            .expect("panicking background task must still wake the companion");
        assert!(state.sync());
        assert!(!state.is_opening());
        assert_eq!(
            state.model.notice.as_deref(),
            Some("The operation stopped unexpectedly.")
        );
    }

    trait DurationCompat {
        fn from_milliseconds_compat() -> Duration;
    }
    impl DurationCompat for Duration {
        fn from_milliseconds_compat() -> Duration {
            Duration::from_millis(500)
        }
    }
}
