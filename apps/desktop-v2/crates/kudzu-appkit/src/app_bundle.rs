//! Main-thread AppKit helpers for the running `.app` bundle, Finder reveal, and frontmost app
//! relevance during permission guidance.

use std::path::{Path, PathBuf};

use objc2_app_kit::NSWorkspace;
use objc2_foundation::{MainThreadMarker, NSArray, NSBundle, NSString, NSURL};

const SETTINGS_BUNDLE_ID: &str = "com.apple.systempreferences";

/// Validates that `path` looks like a macOS `.app` bundle path.
pub fn is_app_bundle_path(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "app")
        && path.file_stem().is_some_and(|stem| !stem.is_empty())
}

/// Returns `true` when `path` is a valid `.app` bundle path that exists on disk as a directory.
pub fn is_existing_app_bundle(path: &Path) -> bool {
    is_app_bundle_path(path) && path.is_dir()
}

/// Returns the `.app` bundle path when the current process launched from inside a bundle.
pub fn running_bundle(_: MainThreadMarker) -> Option<PathBuf> {
    let url = NSBundle::mainBundle().bundleURL();
    let ns_path = url.path()?;
    let path = PathBuf::from(ns_path.to_string());
    is_existing_app_bundle(&path).then_some(path)
}

/// Selects `bundle` in Finder (`NSWorkspace.activateFileViewerSelectingURLs`).
pub fn reveal_in_finder(_: MainThreadMarker, bundle: &Path) -> Result<(), &'static str> {
    if !is_existing_app_bundle(bundle) {
        return Err("application bundle is not available on disk");
    }
    let text = bundle.to_str().ok_or("bundle path is not UTF-8")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(text));
    let urls = NSArray::from_retained_slice(&[url]);
    NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&urls);
    Ok(())
}

/// Classifies the frontmost app for the permission guide.
/// No frontmost app counts as this process, matching the previous relevant-app check.
pub fn front_app(_: MainThreadMarker) -> crate::guide_panel::FrontApp {
    use crate::guide_panel::FrontApp;
    let Some(front) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return FrontApp::ThisApp;
    };
    if front.processIdentifier() == std::process::id() as i32 {
        return FrontApp::ThisApp;
    }
    if front
        .bundleIdentifier()
        .is_some_and(|id| id.to_string() == SETTINGS_BUNDLE_ID)
    {
        return FrontApp::SystemSettings;
    }
    FrontApp::Other
}

#[cfg(test)]
mod tests {
    use super::is_app_bundle_path;
    use std::path::Path;

    #[test]
    fn recognizes_valid_app_bundle_paths_only() {
        assert!(is_app_bundle_path(Path::new("/Applications/Kudzu.app")));
        assert!(!is_app_bundle_path(Path::new("/usr/local/bin/kudzu")));
        assert!(!is_app_bundle_path(Path::new("/tmp/.app")));
    }
}
