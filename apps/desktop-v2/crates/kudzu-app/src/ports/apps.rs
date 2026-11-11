//! Catalogue and target-app access, called from workers.
use crate::error::PortError;
use kudzu_core::system::{AppInfo, Rect};
pub trait AppDirectory: Send + Sync {
    fn installed(&self) -> Result<Vec<AppInfo>, PortError>;
    fn icon(&self, bundle_id: &str, size: u32) -> Result<Option<Vec<u8>>, PortError>;
    fn frontmost(&self) -> Result<Option<AppInfo>, PortError>;
    fn activate(&self, app: &AppInfo) -> Result<(), PortError>;
    fn selection(&self) -> Result<Option<String>, PortError>;
    fn caret(&self) -> Result<Option<Rect>, PortError>;
}
