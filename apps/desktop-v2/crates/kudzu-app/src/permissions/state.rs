//! Permission companion snapshot. A Settings launch or a drop is never a grant.

use crate::ports::PermissionKind;
use kudzu_core::system::EngineStatus;

pub const ACCESSIBILITY_NAME: &str = "Accessibility";
pub const DEVICE_CONTROL_NAME: &str = "Device Control and Data Access";
pub const OPEN_SETTINGS_ERROR: &str =
    "Could not open System Settings. Open Privacy & Security from the Apple menu.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Checking,
    Opening,
    Waiting,
    Allowed,
    Connecting,
    Revoked,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionsState {
    pub selected: PermissionKind,
    pub phase: Phase,
    pub ready: bool,
    pub restart_offered: bool,
    pub accessibility_name: &'static str,
    pub app_name: String,
    pub status: EngineStatus,
    pub error: Option<String>,
}

pub fn accessibility_name(os_major: u32) -> &'static str {
    if os_major >= 27 {
        DEVICE_CONTROL_NAME
    } else {
        ACCESSIBILITY_NAME
    }
}

pub fn ready(status: &EngineStatus) -> bool {
    status.permissions.accessibility
        && status.can_post_events
        && status.tap != kudzu_core::system::TapMode::Off
}

pub fn granted(status: &EngineStatus, kind: PermissionKind) -> bool {
    match kind {
        PermissionKind::Accessibility => status.permissions.accessibility,
        PermissionKind::InputMonitoring => status.permissions.input_monitoring,
    }
}
