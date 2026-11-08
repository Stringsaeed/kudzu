//! What tauri-plugin-opener and tauri-plugin-autostart did: open links and files, reveal files
//! in Finder, and open at login.

use std::path::{Path, PathBuf};
use std::process::Command;

type Res<T> = Result<T, String>;

fn run(cmd: &mut Command) -> Res<()> {
    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("{cmd:?} failed")) }
}

/// Opens a web link (http, https or mailto only: clipboard text is other apps' input).
pub fn open_url(url: &str) -> Res<()> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:")) {
        return Err("Only web links can be opened.".into());
    }
    run(Command::new("/usr/bin/open").arg(url))
}

/// Opens a file in its default app.
pub fn open_path(path: impl AsRef<Path>) -> Res<()> {
    run(Command::new("/usr/bin/open").arg(path.as_ref()))
}

/// Shows a file in Finder.
pub fn reveal(path: impl AsRef<Path>) -> Res<()> {
    run(Command::new("/usr/bin/open").arg("-R").arg(path.as_ref()))
}

// MARK: Open at login

/// A per-user LaunchAgent, as tauri-plugin-autostart's `MacosLauncher::LaunchAgent` wrote it
/// (same file name, so turning it off removes what the Tauri build installed).
fn launch_agent() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library/LaunchAgents").join(format!("{}.plist", super::identity::get().name))
}

/// auto-launch's `build_launch_agent_plist`, byte for byte for any real path (it doesn't XML-escape,
/// so a path with `&` broke its plist; this escapes). Its `AssociatedBundleIdentifiers` array is
/// empty because the plugin set none.
fn launch_agent_plist(name: &str, path: &str, args: &[&str]) -> String {
    let section: String = std::iter::once(path).chain(args.iter().copied()).map(|x| format!("<string>{}</string>", xml_escape(x))).collect();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n  <dict>\n  <key>Label</key>\n  <string>{}</string>\n  <key>AssociatedBundleIdentifiers</key>\n  <array></array>\n  <key>ProgramArguments</key>\n  <array>{}</array>\n  <key>RunAtLoad</key>\n  <true/>\n  </dict>\n</plist>",
        xml_escape(name),
        section
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn set_launch_at_login(on: bool) -> Res<()> {
    let path = launch_agent();
    if !on {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    }
    // As auto-launch 0.6 (under tauri-plugin-autostart 2.7) wrote it, so a login item the Tauri
    // build installed and one this build installs are the same file with the same contents.
    let exe = std::env::current_exe().and_then(|p| p.canonicalize()).map_err(|e| e.to_string())?;
    let label = super::identity::get().name;
    let plist = launch_agent_plist(label, &exe.to_string_lossy(), &["--hidden"]);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    crate::store::write_atomic(&path, plist.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_login_item_matches_what_tauri_plugin_autostart_wrote() {
        let plist = launch_agent_plist("Kudzu", "/Applications/Kudzu.app/Contents/MacOS/kudzu-desktop", &["--hidden"]);
        assert_eq!(
            plist,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n  <dict>\n  <key>Label</key>\n  <string>Kudzu</string>\n  <key>AssociatedBundleIdentifiers</key>\n  <array></array>\n  <key>ProgramArguments</key>\n  <array><string>/Applications/Kudzu.app/Contents/MacOS/kudzu-desktop</string><string>--hidden</string></array>\n  <key>RunAtLoad</key>\n  <true/>\n  </dict>\n</plist>"
        );
    }
}
