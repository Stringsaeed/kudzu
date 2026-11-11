//! Parity scenes for `permissions` (`--window permissions --scene <name>`).
//!
//! Covers all 6 reference states in `docs/migration/reference/metrics.json`:
//! `missing`, `allowed`, `revoked`, `paused`, `secure`, and `restart`.

use kudzu_app::permissions::{
    ready, PermissionsState as AppPermissionsState, Phase, ACCESSIBILITY_NAME,
};
use kudzu_app::ports::PermissionKind;
use kudzu_core::system::{EngineStatus, PermissionStatus, TapMode};

use crate::shell::windows::PermissionsState;
use crate::theme::Appearance;

pub type Scene = (&'static str, fn(&mut PermissionsState));

pub const SCENES: &[Scene] = &[
    ("missing", missing),
    ("allowed", allowed),
    ("revoked", revoked),
    ("paused", paused),
    ("secure", secure),
    ("restart", restart),
];

fn configure(s: &mut PermissionsState, status: EngineStatus, phase: Phase, restart_offered: bool) {
    s.model.scene_locked = true;
    s.model.scene_frame = true;
    s.model.open = true;
    s.model.loaded = true;
    s.model.drag_available = false;
    s.model.bundle_path = None;
    s.model.help_open = false;
    s.model.notice = None;
    s.model.theme.set_for_scene(Appearance::System);
    s.model.snap = AppPermissionsState {
        ready: ready(&status),
        selected: PermissionKind::Accessibility,
        phase,
        restart_offered,
        accessibility_name: ACCESSIBILITY_NAME,
        app_name: "Kudzu".to_owned(),
        status,
        error: None,
    };
}

pub fn missing(s: &mut PermissionsState) {
    configure(
        s,
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
            summary: "Permissions needed".to_owned(),
        },
        Phase::Checking,
        false,
    );
}

pub fn allowed(s: &mut PermissionsState) {
    configure(
        s,
        EngineStatus {
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
            summary: "Ready".to_owned(),
        },
        Phase::Allowed,
        false,
    );
}

pub fn revoked(s: &mut PermissionsState) {
    configure(
        s,
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
            summary: "Permissions revoked".to_owned(),
        },
        Phase::Revoked,
        false,
    );
}

pub fn paused(s: &mut PermissionsState) {
    configure(
        s,
        EngineStatus {
            can_post_events: true,
            can_expand: false,
            paused: true,
            tap: TapMode::Active,
            permissions: PermissionStatus {
                accessibility: true,
                input_monitoring: true,
            },
            secure_input: false,
            secure_input_owner: None,
            summary: "Expansion paused.".to_owned(),
        },
        Phase::Allowed,
        false,
    );
}

pub fn secure(s: &mut PermissionsState) {
    configure(
        s,
        EngineStatus {
            can_post_events: true,
            can_expand: false,
            paused: false,
            tap: TapMode::Active,
            permissions: PermissionStatus {
                accessibility: true,
                input_monitoring: true,
            },
            secure_input: true,
            secure_input_owner: Some("1Password".to_owned()),
            summary: "Secure Input active.".to_owned(),
        },
        Phase::Allowed,
        false,
    );
}

pub fn restart(s: &mut PermissionsState) {
    configure(
        s,
        EngineStatus {
            can_post_events: false,
            can_expand: false,
            paused: false,
            tap: TapMode::Off,
            permissions: PermissionStatus {
                accessibility: true,
                input_monitoring: false,
            },
            secure_input: false,
            secure_input_owner: None,
            summary: "Accessibility is allowed, waiting for macOS keyboard tap.".to_owned(),
        },
        Phase::Connecting,
        false,
    );
}
