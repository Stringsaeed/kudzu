//! Raw pasteboard operations; services own history policy. All reads/pastes run off the UI.
use crate::error::PortError;
use kudzu_core::{clipboard::PasteItem, system::AppInfo};
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PasteSnapshot {
    pub change_count: i64,
    pub own: bool,
    pub items: Vec<PasteItem>,
}
pub trait Pasteboard: Send + Sync {
    fn change_count(&self) -> i64;
    fn read(&self) -> Result<PasteSnapshot, PortError>;
    fn write(&self, items: &[PasteItem]) -> Result<i64, PortError>;
    fn can_paste(&self) -> bool;
    /// Copy first; this paste never restores previous clipboard contents.
    fn paste(&self, target: Option<&AppInfo>) -> Result<(), PortError>;
}
