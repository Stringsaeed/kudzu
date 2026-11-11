use super::*;
use crate::error::PortError;
use crate::fixtures::clock::FakeClock;
use crate::fixtures::engine::FakeEngine;
use crate::fixtures::relaunch::FakeRelauncher;
use crate::fixtures::settings_panes::FakeSystemSettings;
use crate::ports::{PermissionKind, SettingsPane};
use crate::schedule::Scheduler;
use kudzu_core::system::{EngineStatus, PermissionStatus, TapMode};
use std::sync::{mpsc, Arc};
use std::time::Duration;
use uuid::Uuid;

struct Folder(std::path::PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-perm-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Harness {
    permissions: Permissions,
    engine: Arc<FakeEngine>,
    system: Arc<FakeSystemSettings>,
    relauncher: Arc<FakeRelauncher>,
    clock: Arc<FakeClock>,
    folder: Folder,
}

impl Harness {
    fn new(os_major: u32) -> Self {
        let folder = Folder::new();
        let clock = Arc::new(FakeClock::default());
        let engine = Arc::new(FakeEngine::default());
        let system = Arc::new(FakeSystemSettings::default());
        let relauncher = Arc::new(FakeRelauncher::default());
        let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
        let permissions = Permissions::new(
            engine.clone(),
            system.clone(),
            relauncher.clone(),
            scheduler,
            PermissionsConfig {
                app_name: "Kudzu Next".into(),
                os_major,
                marker: folder.0.join("permission-setup-pending"),
            },
        );
        Self {
            permissions,
            engine,
            system,
            relauncher,
            clock,
            folder,
        }
    }

    fn set_status(&self, accessibility: bool, post: bool, tap: TapMode) {
        let mut status = EngineStatus {
            can_post_events: post,
            can_expand: accessibility && post && tap != TapMode::Off,
            paused: false,
            tap,
            permissions: PermissionStatus {
                accessibility,
                input_monitoring: false,
            },
            secure_input: false,
            secure_input_owner: None,
            summary: String::new(),
        };
        if accessibility && post && tap != TapMode::Off {
            status.summary = "Ready".into();
        }
        self.engine.state.lock().unwrap().status = status;
    }
}

#[test]
fn starts_checking_with_the_macos_27_name() {
    let h = Harness::new(27);
    let state = h.permissions.state();
    assert_eq!(state.phase, Phase::Checking);
    assert!(!state.ready);
    assert_eq!(state.accessibility_name, DEVICE_CONTROL_NAME);
    assert_eq!(state.app_name, "Kudzu Next");
    assert_eq!(
        Harness::new(26).permissions.state().accessibility_name,
        ACCESSIBILITY_NAME
    );
}

#[test]
fn ready_needs_accessibility_posting_and_an_attached_tap() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Off);
    h.permissions.refresh();
    assert!(!h.permissions.state().ready);
    assert_eq!(h.permissions.state().phase, Phase::Connecting);
    h.set_status(true, false, TapMode::Active);
    h.permissions.refresh();
    assert!(!h.permissions.state().ready);
    h.set_status(true, true, TapMode::ListenOnly);
    h.permissions.refresh();
    assert!(h.permissions.state().ready);
    assert_eq!(h.permissions.state().phase, Phase::Allowed);
}

#[test]
fn a_settings_launch_never_counts_as_a_grant() {
    let h = Harness::new(27);
    let (tx, rx) = mpsc::channel();
    let watch = h.permissions.watch();
    let observed = watch.clone();
    let _sub = watch.subscribe(move || {
        let _ = tx.send(observed.get().phase);
    });
    h.permissions.begin(PermissionKind::Accessibility).unwrap();
    assert_eq!(
        *h.system.opened.lock().unwrap(),
        [SettingsPane::Accessibility]
    );
    assert_eq!(
        *h.system.requested.lock().unwrap(),
        [PermissionKind::Accessibility]
    );
    assert!(!h.system.permissions.lock().unwrap().accessibility);
    assert_eq!(h.permissions.state().phase, Phase::Waiting);
    assert!(!h.permissions.state().ready);
    let mut saw_opening = false;
    let mut saw_checking = false;
    while let Ok(phase) = rx.try_recv() {
        if phase == Phase::Opening {
            saw_opening = true;
        }
        if phase == Phase::Checking {
            saw_checking = true;
        }
    }
    assert!(saw_opening);
    assert!(!saw_checking);
}

#[test]
fn begin_failure_is_error_not_a_grant() {
    let h = Harness::new(27);
    *h.system.failure.lock().unwrap() = Some(PortError::Unavailable);
    let error = h
        .permissions
        .begin(PermissionKind::InputMonitoring)
        .unwrap_err();
    assert_eq!(error.to_string(), OPEN_SETTINGS_ERROR);
    assert_eq!(h.permissions.state().phase, Phase::Error);
    assert!(!h.permissions.state().ready);
}

#[test]
fn revocation_follows_a_lost_grant() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Active);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Allowed);
    h.set_status(false, false, TapMode::Off);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Revoked);
    assert!(!h.permissions.state().ready);
}

#[test]
fn restart_is_offered_after_five_seconds_without_a_listener() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Off);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Connecting);
    assert!(!h.permissions.state().restart_offered);
    let (tx, rx) = mpsc::channel();
    let watch = h.permissions.watch();
    let observed = watch.clone();
    let _sub = watch.subscribe(move || {
        let _ = tx.send(observed.get().restart_offered);
    });
    h.clock.advance(Duration::from_secs(4));
    assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
    assert!(!h.permissions.state().restart_offered);
    h.clock.advance(Duration::from_secs(1));
    assert!(rx.recv_timeout(Duration::from_secs(2)).unwrap());
    assert!(h.permissions.state().restart_offered);
}

#[test]
fn leaving_connecting_cancels_the_restart_offer() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Off);
    h.permissions.refresh();
    h.set_status(true, true, TapMode::Active);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Allowed);
    h.clock.advance(Duration::from_secs(5));
    assert!(!h.permissions.state().restart_offered);
}

#[test]
fn restart_marker_is_written_and_consumed_once() {
    let h = Harness::new(27);
    assert!(!h.permissions.take_pending_restart());
    h.permissions.request_restart().unwrap();
    assert_eq!(*h.relauncher.calls.lock().unwrap(), 1);
    assert!(h.folder.0.join("permission-setup-pending").exists());
    assert!(h.permissions.take_pending_restart());
    assert!(!h.folder.0.join("permission-setup-pending").exists());
    assert!(!h.permissions.take_pending_restart());
}

#[test]
fn a_failed_marker_write_does_not_relaunch() {
    let h = Harness::new(27);
    std::fs::remove_dir_all(&h.folder.0).unwrap();
    std::fs::write(&h.folder.0, b"file").unwrap();
    assert!(h.permissions.request_restart().is_err());
    assert_eq!(*h.relauncher.calls.lock().unwrap(), 0);
}

#[test]
fn select_treats_the_new_row_as_a_fresh_check() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Active);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Allowed);
    h.permissions.select(PermissionKind::InputMonitoring);
    assert_eq!(h.permissions.state().phase, Phase::Checking);
    assert_eq!(
        h.permissions.state().selected,
        PermissionKind::InputMonitoring
    );
    assert!(h.permissions.state().error.is_none());
}

#[test]
fn select_clears_a_blank_error() {
    let h = Harness::new(27);
    *h.system.failure.lock().unwrap() = Some(PortError::Unavailable);
    assert!(h
        .permissions
        .begin(PermissionKind::InputMonitoring)
        .is_err());
    assert_eq!(h.permissions.state().phase, Phase::Error);
    h.permissions.select(PermissionKind::Accessibility);
    assert_eq!(h.permissions.state().phase, Phase::Checking);
    assert!(h.permissions.state().error.is_none());
}

#[test]
fn revoked_holds_until_begin_or_select() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Active);
    h.permissions.refresh();
    h.set_status(false, false, TapMode::Off);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Revoked);
    h.permissions.refresh();
    assert_eq!(h.permissions.state().phase, Phase::Revoked);
    h.permissions.select(PermissionKind::Accessibility);
    assert_eq!(h.permissions.state().phase, Phase::Checking);
}

#[test]
fn begin_lands_on_allowed_when_the_row_is_already_granted() {
    let h = Harness::new(27);
    h.set_status(true, true, TapMode::Active);
    h.permissions.begin(PermissionKind::Accessibility).unwrap();
    assert_eq!(h.permissions.state().phase, Phase::Allowed);
}

#[test]
fn a_failed_relaunch_removes_the_restart_marker() {
    let h = Harness::new(27);
    *h.relauncher.failure.lock().unwrap() = Some(PortError::Unavailable);
    assert!(h.permissions.request_restart().is_err());
    assert_eq!(*h.relauncher.calls.lock().unwrap(), 0);
    assert!(!h.folder.0.join("permission-setup-pending").exists());
}
