//! Typed permission requests; opening a pane never implies a grant.
use crate::error::PortError;
use kudzu_core::system::PermissionStatus;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPane {
    Accessibility,
    InputMonitoring,
    LoginItems,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionKind {
    Accessibility,
    InputMonitoring,
}
pub trait SystemSettings: Send + Sync {
    fn permissions(&self) -> PermissionStatus;
    fn request(&self, kind: PermissionKind) -> Result<(), PortError>;
    fn open(&self, pane: SettingsPane) -> Result<(), PortError>;
}
