use crate::release::{command, root, Result};
use std::{path::Path, process::Command};

pub const CREATE_DMG_VERSION: &str = "1.2.3";
pub fn layout() -> Vec<&'static str> {
    vec![
        "--volname",
        "Kudzu",
        "--window-size",
        "660",
        "440",
        "--icon-size",
        "128",
        "--icon",
        "Kudzu.app",
        "170",
        "220",
        "--app-drop-link",
        "490",
        "220",
        "--hide-extension",
        "Kudzu.app",
    ]
}
pub fn build(source: &Path, output: &Path) -> Result<()> {
    let version = Command::new("create-dmg")
        .arg("--version")
        .output()
        .map_err(|_| format!("create-dmg {CREATE_DMG_VERSION} required (not found on PATH)"))?;
    if !version.status.success()
        || String::from_utf8_lossy(&version.stdout)
            .trim()
            .trim_start_matches("create-dmg ")
            != CREATE_DMG_VERSION
    {
        return Err(format!("create-dmg {CREATE_DMG_VERSION} required").into());
    }
    command(
        Command::new("create-dmg")
            .args(layout())
            .arg("--background")
            .arg(root().join("design/dmg/background.png"))
            .arg("--volicon")
            .arg(root().join("assets/icons/icon.icns"))
            .arg(output)
            .arg(source),
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn finder_layout_is_fixed() {
        let args = super::layout();
        assert!(args
            .windows(3)
            .any(|v| v == ["--window-size", "660", "440"]));
        assert!(args
            .windows(4)
            .any(|v| v == ["--icon", "Kudzu.app", "170", "220"]));
        assert!(args
            .windows(3)
            .any(|v| v == ["--app-drop-link", "490", "220"]));
    }
}
