//! Headless UI integration tests for the `permissions` companion (`S66`).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kudzu_app::assembly::{Config, Host, Kudzu};
use kudzu_app::fixtures::Fixtures;
use kudzu_app::paths::Paths;
use kudzu_app::permissions::RESTART_WAIT;
use kudzu_app::ports::{PermissionKind, SettingsPane};
use kudzu_app::settings::Variant;
use kudzu_core::system::{EngineStatus, PermissionStatus, TapMode};
use kudzu_ui::permissions::model::{PrimaryAction, StatusBanner, UNBUNDLED_REVEAL_NOTICE};
use kudzu_ui::permissions::view::{render, ICON_KEY};
use kudzu_ui::shell::windows::PermissionsState;
use kudzu_ui::shell::{Identity, Launch};
use rune::drag_out::DRAG_THRESHOLD;
use rune::{Env, Key, KeyEvent, Mods, Runtime};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-ui-perm-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const LAUNCH: Launch = Launch {
    identity: Identity {
        app_name: "Kudzu",
        dev: true,
    },
    native: false,
    hidden: false,
};

fn missing_status() -> EngineStatus {
    EngineStatus {
        can_post_events: false,
        can_expand: false,
        paused: false,
        tap: TapMode::Off,
        permissions: PermissionStatus {
            accessibility: false,
            input_monitoring: false,
        },
        secure_input: false,
        secure_input_owner: None,
        summary: "Grant Accessibility to expand".into(),
    }
}

fn build_kudzu(dir: &std::path::Path, fixtures: &Fixtures) -> Arc<Kudzu> {
    Kudzu::new(Config {
        paths: Paths {
            data: dir.to_path_buf(),
            logs: dir.join("logs"),
        },
        host: Host::fixtures(fixtures, dir.to_path_buf()),
        variant: Variant::Dev,
        version: "0.7.0".into(),
        app_name: "Kudzu".into(),
        bundle_id: "sh.saeed.kudzu".into(),
        os_major: 26,
        mcp_name: "kudzu".into(),
        executable: dir.join("Kudzu"),
        agent_args: vec!["--mcp".into()],
        bundled_skill_dir: None,
        production: false,
        cloud_and_updates: false,
        debounce: Duration::from_millis(50),
        retry: Duration::from_millis(50),
    })
    .unwrap()
}

struct Harness {
    _scratch: Scratch,
    dir: PathBuf,
    fixtures: Fixtures,
    rt: Runtime<PermissionsState>,
}

impl Harness {
    fn new() -> Self {
        let scratch = Scratch::new();
        let dir = scratch.0.clone();
        let fixtures = Fixtures::new().unwrap();
        let status = missing_status();
        fixtures.engine.state.lock().unwrap().status = status.clone();
        *fixtures.settings.permissions.lock().unwrap() = status.permissions;

        let kudzu = build_kudzu(&dir, &fixtures);
        let (_shell, mut state) = PermissionsState::headless_for_test(&kudzu, LAUNCH);
        state.show();
        wait_until(|| {
            let _ = state.sync();
            state.model.loaded
        });

        let mut rt = Runtime::new(state, render, (420.0, 660.0), Env::new(true, 1.0));
        rt.frame(0.0);
        Self {
            _scratch: scratch,
            dir,
            fixtures,
            rt,
        }
    }

    fn sync_and_frame(&mut self) {
        self.rt.update(|state, _| {
            let _ = state.sync();
        });
        self.rt.frame(0.0);
    }

    fn wait_sync(&mut self, mut cond: impl FnMut(&PermissionsState) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            self.sync_and_frame();
            if cond(&self.rt.state) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out waiting for state");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn center_of(&self, key: &str) -> (f32, f32) {
        let el = self
            .rt
            .elements()
            .into_iter()
            .find(|el| el.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("missing element with key '{key}'"));
        (el.rect[0] + el.rect[2] * 0.5, el.rect[1] + el.rect[3] * 0.5)
    }

    fn click(&mut self, key: &str) {
        let (x, y) = self.center_of(key);
        self.rt.pointer_move(x, y);
        self.rt.pointer_down(x, y);
        self.rt.pointer_up(x, y);
        self.rt.frame(0.0);
    }
}

fn wait_until(mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for condition");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn card_selection_and_set_up_app_control_cta_switch_selected_permission() {
    let mut h = Harness::new();
    assert_eq!(
        h.rt.state.model.snap.selected,
        PermissionKind::Accessibility
    );

    // Grant Input Monitoring while Accessibility remains missing, then select Input Monitoring.
    let mut st = missing_status();
    st.permissions.input_monitoring = true;
    h.fixtures.engine.state.lock().unwrap().status = st.clone();
    *h.fixtures.settings.permissions.lock().unwrap() = st.permissions;

    h.click("card-input-monitoring");
    h.wait_sync(|s| {
        s.model.snap.selected == PermissionKind::InputMonitoring
            && s.model.primary_action() == PrimaryAction::SelectAccessibility
    });

    // Clicking "Set up app control" switches selection back to Accessibility.
    h.click("primary-cta");
    assert_eq!(
        h.rt.state.model.snap.selected,
        PermissionKind::Accessibility
    );
}

#[test]
fn open_settings_transitions_to_waiting_and_grant_allows_back_to_kudzu() {
    let mut h = Harness::new();

    h.click("primary-cta");
    h.wait_sync(|s| matches!(s.model.status_banner(), StatusBanner::Waiting { .. }));
    assert_eq!(
        *h.fixtures.settings.opened.lock().unwrap(),
        vec![SettingsPane::Accessibility]
    );

    // Simulate macOS granting Accessibility and activating the event tap.
    let ready_status = EngineStatus {
        can_post_events: true,
        can_expand: true,
        paused: false,
        tap: TapMode::Active,
        permissions: PermissionStatus {
            accessibility: true,
            input_monitoring: true,
        },
        secure_input: false,
        secure_input_owner: None,
        summary: "Ready".into(),
    };
    h.fixtures.engine.state.lock().unwrap().status = ready_status.clone();
    *h.fixtures.settings.permissions.lock().unwrap() = ready_status.permissions;
    h.rt.update(|s, _| s.spawn_refresh());
    h.wait_sync(|s| s.model.snap.ready);

    assert_eq!(h.rt.state.model.headline(), "You’re all set.");
    assert_eq!(h.rt.state.model.primary_action(), PrimaryAction::Done);

    // Clicking "Back to Kudzu" closes the companion.
    h.click("primary-cta");
    assert!(!h.rt.state.model.open);
}

#[test]
fn connecting_tap_offers_restart_after_5s_and_marker_reopens_companion() {
    let mut h = Harness::new();

    // Accessibility granted, but event tap has not connected yet (`TapMode::Off`).
    let mut granted_no_tap = missing_status();
    granted_no_tap.permissions.accessibility = true;
    h.fixtures.engine.state.lock().unwrap().status = granted_no_tap.clone();
    *h.fixtures.settings.permissions.lock().unwrap() = granted_no_tap.permissions;
    h.rt.update(|s, _| s.spawn_refresh());
    h.wait_sync(|s| s.model.needs_connection());

    assert_eq!(h.rt.state.model.headline(), "Access is allowed.");
    assert!(!h.rt.state.model.snap.restart_offered);

    // Advance monotonic clock past RESTART_WAIT (5s) -> "Restart Kudzu" CTA appears.
    h.fixtures.clock.advance(RESTART_WAIT);
    h.wait_sync(|s| s.model.snap.restart_offered);
    assert_eq!(
        h.rt.state.model.primary_action(),
        PrimaryAction::Restart {
            label: "Restart Kudzu".into()
        }
    );

    // Clicking "Restart Kudzu" writes the restart marker and invokes Relauncher.
    h.click("primary-cta");
    wait_until(|| *h.fixtures.relauncher.calls.lock().unwrap() == 1);

    // A fresh process booting on the same data directory consumes the marker and reopens.
    let rebooted = build_kudzu(&h.dir, &h.fixtures);
    let (_shell2, mut state2) = PermissionsState::headless_for_test(&rebooted, LAUNCH);
    assert!(!state2.model.open);
    wait_until(|| {
        let _ = state2.sync();
        state2.model.open
    });
}

#[test]
fn app_icon_supports_bundle_drag_out_click_reveal_and_loose_binary_notice() {
    let mut h = Harness::new();

    // Unbundled binary: dragging stays a click and surfaces UNBUNDLED_REVEAL_NOTICE.
    let (x, y) = h.center_of(ICON_KEY);
    h.rt.pointer_move(x, y);
    h.rt.pointer_down(x, y);
    h.rt.pointer_move(x + DRAG_THRESHOLD + 4.0, y);
    h.rt.pointer_up(x + DRAG_THRESHOLD + 4.0, y);
    h.rt.frame(0.0);
    assert_eq!(
        h.rt.state.model.notice.as_deref(),
        Some(UNBUNDLED_REVEAL_NOTICE)
    );
    assert!(h.rt.state.revealed_paths.is_empty());

    // Bundled `.app`: dragging across DRAG_THRESHOLD starts a drag-out without clicking to reveal.
    let bundle = PathBuf::from("/Applications/Kudzu.app");
    h.rt.update(|s, _| {
        s.set_bundle_for_test(Some(bundle.clone()));
        s.model.notice = None;
    });
    h.sync_and_frame();

    let (x, y) = h.center_of(ICON_KEY);
    h.rt.pointer_move(x, y);
    h.rt.pointer_down(x, y);
    h.rt.pointer_move(x + DRAG_THRESHOLD + 4.0, y);
    h.rt.frame(0.0);
    assert!(
        h.rt.state.revealed_paths.is_empty(),
        "drag-out must not trigger click-to-reveal"
    );

    // Clicking the icon (without dragging) reveals the `.app` bundle in Finder.
    h.click(ICON_KEY);
    assert_eq!(h.rt.state.revealed_paths, vec![bundle]);
}

#[test]
fn escape_closes_help_sheet_before_closing_companion() {
    let mut h = Harness::new();
    assert!(h.rt.state.model.open);
    assert!(!h.rt.state.model.help_open);

    h.click("help-toggle");
    assert!(h.rt.state.model.help_open);
    assert!(
        h.rt.elements()
            .iter()
            .any(|el| el.key.as_deref() == Some("help-sheet")),
        "help-sheet popover must mount when help_open is true"
    );

    let esc = KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    };
    let mut handled = false;
    h.rt.update(|s, _| handled = s.handle_key(&esc));
    assert!(handled);
    h.rt.frame(0.0);
    assert!(!h.rt.state.model.help_open);
    assert!(
        h.rt.state.model.open,
        "first Escape closes Help while keeping companion open"
    );

    h.rt.update(|s, _| handled = s.handle_key(&esc));
    assert!(handled);
    assert!(!h.rt.state.model.open, "second Escape closes the companion");
}

#[test]
fn all_six_parity_scenes_render_in_light_and_dark_themes() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let kudzu = build_kudzu(&scratch.0, &fixtures);

    assert_eq!(kudzu_ui::permissions::scenes::SCENES.len(), 6);
    for (name, configure) in kudzu_ui::permissions::scenes::SCENES {
        for dark in [false, true] {
            let (_shell, mut state) = PermissionsState::headless_for_test(&kudzu, LAUNCH);
            configure(&mut state);
            let mut rt = Runtime::new(state, render, (420.0, 660.0), Env::new(dark, 2.0));
            rt.frame(0.0);
            let elements = rt.elements();
            for required in [
                "region-card",
                "header",
                "close",
                ICON_KEY,
                "headline",
                "card-accessibility",
                "card-input-monitoring",
                "primary-cta",
                "help-toggle",
            ] {
                assert!(
                    elements
                        .iter()
                        .any(|el| el.key.as_deref() == Some(required)),
                    "scene '{name}' (dark={dark}) missing element '{required}'"
                );
            }
        }
    }
}

#[test]
fn parity_scenes_match_tauri_mock_permission_states() {
    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    let kudzu = build_kudzu(&scratch.0, &fixtures);

    let expected = [
        (
            "missing",
            false,
            false,
            "Grant Accessibility to expand",
            None,
        ),
        ("allowed", true, false, "Ready", None),
        (
            "revoked",
            false,
            false,
            "Grant Accessibility to expand",
            None,
        ),
        ("paused", true, false, "Expansion paused", None),
        (
            "secure",
            true,
            false,
            "Paused while a password field is focused",
            None,
        ),
        (
            "restart",
            true,
            false,
            "Restart Kudzu to finish connecting",
            None,
        ),
    ];

    for (name, acc, im, summary, owner) in expected {
        let (_, configure) = kudzu_ui::permissions::scenes::SCENES
            .iter()
            .copied()
            .find(|(s, _)| *s == name)
            .unwrap_or_else(|| panic!("missing scene '{name}'"));
        let (_shell, mut state) = PermissionsState::headless_for_test(&kudzu, LAUNCH);
        configure(&mut state);
        let st = &state.model.snap.status;
        assert_eq!(
            st.permissions,
            PermissionStatus {
                accessibility: acc,
                input_monitoring: im,
            },
            "scene '{name}' permissions must match Tauri previewPermission(mode)"
        );
        assert_eq!(
            st.summary, summary,
            "scene '{name}' summary must match Tauri previewPermission(mode)"
        );
        assert_eq!(
            st.secure_input_owner.as_deref(),
            owner,
            "scene '{name}' secure_input_owner must match Tauri previewPermission(mode)"
        );
    }
}

#[test]
fn unbundled_companion_renders_explanation_next_to_disabled_drag_icon() {
    let h = Harness::new();
    assert!(
        !h.rt.state.model.drag_available,
        "headless loose binary has drag disabled"
    );
    let hint =
        h.rt.elements()
            .into_iter()
            .find(|el| el.key.as_deref() == Some("drag-hint"))
            .expect("drag-hint element must be rendered next to the app icon");
    assert_eq!(
        hint.text.as_deref(),
        Some(UNBUNDLED_REVEAL_NOTICE),
        "unbundled companion must explain why dragging is unavailable without requiring icon click/drag"
    );
}

#[test]
fn focusing_open_companion_refreshes_immediately_and_coalesces_in_flight() {
    use kudzu_app::error::PortError;
    use kudzu_app::ports::engine::AdmissionError;
    use kudzu_app::ports::{EngineEvents, EngineLibrary, ExpansionEngine};
    use kudzu_core::system::AppInfo;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Mutex};
    use uuid::Uuid;

    struct CountingEngine {
        inner: Arc<dyn ExpansionEngine>,
        calls: AtomicUsize,
        gate: Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
    }

    impl ExpansionEngine for CountingEngine {
        fn start(&self, events: Arc<dyn EngineEvents>) -> Result<(), PortError> {
            self.inner.start(events)
        }
        fn stop(&self) {
            self.inner.stop();
        }
        fn reload(&self, library: EngineLibrary) {
            self.inner.reload(library);
        }
        fn status(&self) -> EngineStatus {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let blocker = self.gate.lock().unwrap().take();
            if let Some((entered, release)) = blocker {
                let _ = entered.send(());
                let _ = release.recv();
            }
            self.inner.status()
        }
        fn set_paused(&self, paused: bool) {
            self.inner.set_paused(paused);
        }
        fn set_suppressed(&self, suppressed: bool) {
            self.inner.set_suppressed(suppressed);
        }
        fn insert(
            &self,
            snippet: Uuid,
            target: Option<AppInfo>,
            selection: Option<String>,
        ) -> Result<(), AdmissionError> {
            self.inner.insert(snippet, target, selection)
        }
    }

    let scratch = Scratch::new();
    let fixtures = Fixtures::new().unwrap();
    fixtures.engine.state.lock().unwrap().status = missing_status();
    let mut host = Host::fixtures(&fixtures, scratch.0.clone());
    let engine = Arc::new(CountingEngine {
        inner: host.ports.engine.clone(),
        calls: AtomicUsize::new(0),
        gate: Mutex::new(None),
    });
    host.ports.engine = engine.clone();

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

    let (_shell, mut state) = PermissionsState::headless_for_test(&kudzu, LAUNCH);
    let initial_calls = engine.calls.load(Ordering::SeqCst);
    state.show();
    wait_until(|| {
        let _ = state.sync();
        state.model.loaded && engine.calls.load(Ordering::SeqCst) > initial_calls
    });
    std::thread::sleep(Duration::from_millis(20));
    let _ = state.sync();
    let baseline = engine.calls.load(Ordering::SeqCst);

    // Grant Accessibility & activate tap while `last_refresh` is fresh (< 950ms).
    let ready_status = EngineStatus {
        can_post_events: true,
        can_expand: true,
        paused: false,
        tap: TapMode::Active,
        permissions: PermissionStatus {
            accessibility: true,
            input_monitoring: false,
        },
        secure_input: false,
        secure_input_owner: None,
        summary: "Ready".into(),
    };
    fixtures.engine.state.lock().unwrap().status = ready_status;

    // Blurring (`focused(false)`) must not refresh.
    state.focused(false);
    std::thread::sleep(Duration::from_millis(25));
    let _ = state.sync();
    assert!(!state.model.snap.ready);

    // Focusing (`focused(true)`) while open must refresh immediately without waiting 950ms.
    state.focused(true);
    let fast_deadline = Instant::now() + Duration::from_millis(300);
    while !state.model.snap.ready {
        assert!(
            Instant::now() < fast_deadline,
            "focusing the open companion did not trigger an immediate refresh"
        );
        let _ = state.sync();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(engine.calls.load(Ordering::SeqCst) > baseline);

    // Repeated focus events while a refresh is already in flight must coalesce into one trailing refresh.
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *engine.gate.lock().unwrap() = Some((entered_tx, release_rx));
    let before_burst = engine.calls.load(Ordering::SeqCst);

    state.focused(true);
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    // Three focus bursts arrive while the first refresh is still blocked:
    state.focused(true);
    state.focused(true);
    state.focused(true);
    release_tx.send(()).unwrap();

    wait_until(|| {
        let _ = state.sync();
        engine.calls.load(Ordering::SeqCst) >= before_burst + 2
    });
    std::thread::sleep(Duration::from_millis(30));
    let _ = state.sync();
    assert_eq!(
        engine.calls.load(Ordering::SeqCst),
        before_burst + 2,
        "in-flight refresh + multiple focus bursts must coalesce into exactly 1 trailing refresh"
    );
}
