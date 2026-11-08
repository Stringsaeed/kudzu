//! Which copy of Kudzu this is: production (`sh.saeed.kudzu`) or the side-by-side development
//! variant (`sh.saeed.kudzu.dev`), and where each keeps its data and logs.
//!
//! Tauri read the identifier from `tauri.conf.json`. A Rune app reads it from its bundle's
//! `Info.plist`; a bare `cargo run` binary has no bundle, so debug builds are the dev variant
//! and release builds are production. `KUDZU_DEV=1` / `KUDZU_DEV=0` overrides either.

use std::path::PathBuf;
use std::sync::OnceLock;

pub const PROD_ID: &str = "sh.saeed.kudzu";
pub const DEV_ID: &str = "sh.saeed.kudzu.dev";

#[derive(Debug, Clone)]
pub struct Identity {
    pub id: String,
    pub dev: bool,
    /// "Kudzu" or "Kudzu Dev".
    pub name: &'static str,
    /// The app version from `apps/desktop/package.json` (bumped by release-please).
    pub version: String,
}

static IDENTITY: OnceLock<Identity> = OnceLock::new();

pub fn is_dev_identifier(identifier: &str) -> bool {
    identifier == DEV_ID
}

pub fn get() -> &'static Identity {
    IDENTITY.get_or_init(|| {
        let id = match std::env::var("KUDZU_DEV").ok().as_deref() {
            Some("1") => DEV_ID.to_string(),
            Some("0") => PROD_ID.to_string(),
            _ => bundle_identifier().unwrap_or_else(|| if cfg!(debug_assertions) { DEV_ID } else { PROD_ID }.to_string()),
        };
        let dev = is_dev_identifier(&id);
        Identity { name: if dev { "Kudzu Dev" } else { "Kudzu" }, version: package_version(), dev, id }
    })
}

fn package_version() -> String {
    const PACKAGE: &str = include_str!("../../../../../package.json");
    serde_json::from_str::<serde_json::Value>(PACKAGE)
        .ok()
        .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "0.0.0".into())
}

#[cfg(target_os = "macos")]
fn bundle_identifier() -> Option<String> {
    use objc2_foundation::NSBundle;
    let bundle = NSBundle::mainBundle();
    // A bare binary's main bundle is its folder, with no identifier.
    bundle.bundleIdentifier().map(|s| s.to_string()).filter(|s| s.starts_with(PROD_ID))
}

#[cfg(not(target_os = "macos"))]
fn bundle_identifier() -> Option<String> {
    None
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// `~/Library/Application Support/<id>` (what Tauri's `app_data_dir` returned).
pub fn data_dir() -> PathBuf {
    // A fixture folder (`scripts/rune-fixture.mjs`) for screenshots and tests.
    if let Some(dir) = std::env::var_os("KUDZU_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if cfg!(target_os = "macos") {
        home().join("Library/Application Support").join(&get().id)
    } else {
        home().join(".local/share").join(&get().id)
    }
}

/// `~/Library/Logs/<id>` (what tauri-plugin-log wrote to).
pub fn log_dir() -> PathBuf {
    if cfg!(target_os = "macos") {
        home().join("Library/Logs").join(&get().id)
    } else {
        data_dir().join("logs")
    }
}

/// The `.app` folder this binary runs from, when it runs from one.
pub fn bundle_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    (app.extension().is_some_and(|e| e == "app")).then(|| app.to_path_buf())
}

/// `Kudzu.app/Contents/Resources`, when bundled.
pub fn resource_dir() -> Option<PathBuf> {
    bundle_dir().map(|b| b.join("Contents/Resources"))
}
