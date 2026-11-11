//! Bundle validation and replacement after signature verification by the update service.
use crate::error::PortError;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstallProbe {
    pub needs_move: bool,
}
#[derive(Clone)]
pub struct UpdatePackage {
    pub archive: PathBuf,
    pub bundle_id: String,
    pub version: String,
}
pub trait UpdateInstaller: Send + Sync {
    fn probe(&self) -> Result<InstallProbe, PortError>;
    /// Validate archive paths, bundle identity, version, and code signature before swapping.
    fn install(&self, package: &UpdatePackage) -> Result<(), PortError>;
}
