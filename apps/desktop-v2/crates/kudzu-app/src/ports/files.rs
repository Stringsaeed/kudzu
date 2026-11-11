//! Opening and revealing user-selected files.
use crate::error::PortError;
use std::path::Path;
pub trait FileOpener: Send + Sync {
    fn open(&self, path: &Path) -> Result<(), PortError>;
    fn reveal(&self, path: &Path) -> Result<(), PortError>;
}
