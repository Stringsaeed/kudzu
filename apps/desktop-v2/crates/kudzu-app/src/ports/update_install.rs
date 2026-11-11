//! Bundle validation and replacement after signature verification by the update service.
//!
//! `install` must enforce all of the following before swapping:
//! - The extracted `CFBundleShortVersionString` equals `UpdatePackage.version` (the
//!   version taken from the signed minisign trusted comment) and is strictly greater
//!   than the **running** bundle's version.
//! - `CFBundleIdentifier` equals `UpdatePackage.bundle_id`.
//! - `codesign --verify --deep --strict` passes against a designated requirement
//!   pinned to team `V3HN8HXZYK` and that bundle id.
//! - The staged archive's BLAKE3 digest matches `UpdatePackage.digest`.
//! - The archive contains no symlinks or hardlinks escaping the bundle, no device
//!   files, and no setuid/setgid bits.
//! - Total extracted size and entry count are capped.
//! - Extraction happens on the same volume as the running bundle, and the swap is a
//!   `rename`.
use crate::error::PortError;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InstallProbe {
    pub needs_move: bool,
}
#[derive(Clone)]
pub struct UpdatePackage {
    pub archive: PathBuf,
    /// Expected `CFBundleIdentifier` of the extracted app.
    pub bundle_id: String,
    /// Signed version from the minisign trusted comment (`file:Kudzu_<version>_…`).
    pub version: String,
    /// BLAKE3 of the staged archive bytes (D16).
    pub digest: [u8; 32],
}
pub trait UpdateInstaller: Send + Sync {
    fn probe(&self) -> Result<InstallProbe, PortError>;
    /// Validate archive paths, bundle identity, version, and code signature before swapping.
    fn install(&self, package: &UpdatePackage) -> Result<(), PortError>;
}
