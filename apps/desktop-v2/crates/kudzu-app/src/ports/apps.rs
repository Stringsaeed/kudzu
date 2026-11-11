//! Catalogue and target-app access, called from workers.
use crate::error::PortError;
use kudzu_core::system::{AppInfo, Rect};
pub trait AppCatalog: Send + Sync {
    fn installed(&self) -> Result<Vec<AppInfo>, PortError>;
    fn icon(&self, bundle_id: &str, size: u32) -> Result<Option<Vec<u8>>, PortError>;
}
pub trait TargetApp: Send + Sync {
    fn frontmost(&self) -> Result<Option<AppInfo>, PortError>;
    fn activate(&self, app: &AppInfo) -> Result<(), PortError>;
    fn selection(&self) -> Result<Option<String>, PortError>;
    fn caret(&self) -> Result<Option<Rect>, PortError>;
}

/// Finder-style icons for filesystem paths. S35 implements this with NSWorkspace.
pub trait FileIcons: Send + Sync {
    fn file_icon(&self, path: &str, px: u32) -> Result<Option<Vec<u8>>, PortError>;
}
