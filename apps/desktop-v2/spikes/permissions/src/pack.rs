//! Pack this binary into a signed `.app` so `NSBundle.mainBundle.bundleURL` is the running app.
//! Distinct bundle id so the spike cannot steal Kudzu Next's TCC grants.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

pub const BUNDLE_ID: &str = "sh.saeed.kudzu.next.permissions-spike";
pub const APP_NAME: &str = "Kudzu Permissions Spike";
const EXEC: &str = "permissions-spike";

pub fn already_bundled() -> bool {
    let Ok(exe) = env::current_exe() else {
        return false;
    };
    bundled_app(&exe).is_some()
}

pub fn bundled_app(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    if macos.file_name()?.to_str()? != "MacOS" {
        return None;
    }
    let contents = macos.parent()?;
    if contents.file_name()?.to_str()? != "Contents" {
        return None;
    }
    let app = contents.parent()?;
    (app.extension()?.to_str()? == "app").then(|| app.to_path_buf())
}

/// If this process is a loose binary, write a signed `.app`, re-exec into it, and exit.
pub fn ensure_bundled() {
    if already_bundled() {
        return;
    }
    match pack_and_reexec() {
        Ok(()) => {}
        Err(err) => eprintln!("permissions-spike: packing skipped: {err}"),
    }
}

fn pack_and_reexec() -> Result<(), String> {
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let app = bundle_dir()?.join(format!("{APP_NAME}.app"));
    write_bundle(&exe, &app)?;
    sign(&app)?;
    let child = app.join("Contents/MacOS").join(EXEC);
    let status = Command::new(&child)
        .args(env::args().skip(1))
        .env("KUDZU_PERMISSIONS_SPIKE_PACKED", "1")
        .status()
        .map_err(|e| e.to_string())?;
    process::exit(status.code().unwrap_or(1));
}

fn bundle_dir() -> Result<PathBuf, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace root")?;
    let dir = root.join("target/bundle");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn write_bundle(exe: &Path, app: &Path) -> Result<(), String> {
    if app.exists() {
        fs::remove_dir_all(app).map_err(|e| e.to_string())?;
    }
    let macos = app.join("Contents/MacOS");
    let resources = app.join("Contents/Resources");
    fs::create_dir_all(&macos).map_err(|e| e.to_string())?;
    fs::create_dir_all(&resources).map_err(|e| e.to_string())?;
    fs::copy(exe, macos.join(EXEC)).map_err(|e| e.to_string())?;
    fs::write(app.join("Contents/Info.plist"), plist()).map_err(|e| e.to_string())?;
    let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons/icon.icns");
    if icon.exists() {
        fs::copy(&icon, resources.join("icon.icns")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleExecutable</key><string>{EXEC}</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>
<key>CFBundleName</key><string>{APP_NAME}</string>
<key>CFBundleDisplayName</key><string>{APP_NAME}</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>0.1.0</string>
<key>LSMinimumSystemVersion</key><string>14.0</string>
<key>LSUIElement</key><true/>
</dict></plist>
"#
    )
}

fn sign(app: &Path) -> Result<(), String> {
    let identity = env::var("KUDZU_DEV_SIGN").unwrap_or_else(|_| "-".into());
    let status = Command::new("codesign")
        .args(["--force", "--sign", &identity, "--timestamp=none"])
        .arg(app)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("codesign failed: {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_macos_layout_is_a_bundle() {
        let exe =
            PathBuf::from("/tmp/Kudzu Permissions Spike.app/Contents/MacOS/permissions-spike");
        assert_eq!(
            bundled_app(&exe).as_deref(),
            Some(Path::new("/tmp/Kudzu Permissions Spike.app"))
        );
        assert!(bundled_app(Path::new("/tmp/target/debug/permissions-spike")).is_none());
    }
}
