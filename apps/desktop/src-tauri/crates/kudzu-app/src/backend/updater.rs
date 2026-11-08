//! Self-updates, reading the same feed tauri-plugin-updater did, so copies installed from the
//! Tauri builds and this app update from one release pipeline.
//!
//! Flow: check → download quietly (minisign signature verified against the release key) →
//! hold the verified archive on disk → install when the user quits, or immediately via
//! "Restart to Update". Kudzu never restarts on its own: a text expander restarting
//! mid-sentence would drop the trigger being typed.
//!
//! Feeds: stable is `latest.json` on the newest published release. Beta users read `beta.json`
//! from the rolling `update-feed` release, which CI keeps pointing at whichever is newer, the
//! latest beta or the latest stable, so beta users never fall behind stable.
//!
//! The manifest is `{ version, notes, pub_date, platforms: { "darwin-aarch64": { url, signature } } }`;
//! `url` is a `.app.tar.gz` and `signature` the base64 of its minisign signature file.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use base64::Engine as _;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use super::core::{core, Event};
use crate::settings::UpdateChannel;

const STABLE_FEED: &str = "https://github.com/Stringsaeed/kudzu/releases/latest/download/latest.json";
const BETA_FEED: &str = "https://github.com/Stringsaeed/kudzu/releases/download/update-feed/beta.json";
/// The release signing key (`plugins.updater.pubkey` in tauri.conf.json).
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDZFNTIyNjlGNDI0MUYwMDcKUldRSDhFRkNueVpTYnVNNUJHYWt0dWV4Ti83MHA5UTFaUEN3d050NWlsdlBrRG5uNTdrZUFJT0MK";
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);

fn update_allowed(identifier: &str) -> Result<(), &'static str> {
    if super::identity::is_dev_identifier(identifier) {
        Err("Updates are unavailable in Kudzu Dev.")
    } else {
        Ok(())
    }
}

/// What the UI shows about updates.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate {
        #[serde(rename = "checkedAt")]
        checked_at: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    /// Downloaded and verified; installs on quit or on "Restart to Update".
    Ready {
        version: String,
        notes: Option<String>,
        date: Option<String>,
    },
    /// Installing; the app relaunches in a moment.
    Restarting { version: String },
    Error {
        message: String,
        /// Set when the failure is about where the app lives, not the network.
        #[serde(rename = "needsMove")]
        needs_move: bool,
    },
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: std::collections::HashMap<String, Platform>,
}

#[derive(Deserialize)]
struct Platform {
    url: String,
    signature: String,
}

struct Pending {
    version: String,
    archive: PathBuf,
}

static STATE: Mutex<UpdateState> = Mutex::new(UpdateState::Idle);
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// Serialises checks so the timer and a "Check now" click can't download twice.
static BUSY: AtomicBool = AtomicBool::new(false);

fn set(next: UpdateState) {
    *STATE.lock() = next.clone();
    if let Some(core) = super::core::try_core() {
        core.shell().tray(super::core::TrayUpdate::Update(next.clone()));
        core.emit(Event::Update(next));
    }
}

/// Current state, for windows that open after a check finished.
pub fn state() -> UpdateState {
    STATE.lock().clone()
}

/// Kudzu can only replace itself when it runs from a writable, non-quarantined location.
/// Running straight from the DMG, or from App Translocation's randomized read-only copy
/// (a quarantined app that was never moved), makes installs fail.
pub fn location_problem() -> Option<&'static str> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.to_string_lossy();
    if path.contains("/AppTranslocation/") || path.starts_with("/Volumes/") {
        return Some("Move Kudzu to your Applications folder so it can update itself.");
    }
    // The .app bundle's parent folder must be writable.
    let bundle: PathBuf = exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app"))?.to_path_buf();
    let probe = bundle.parent()?.join(".kudzu-write-test");
    match std::fs::write(&probe, b"") {
        Ok(()) => {
            let _ = std::fs::remove_file(probe);
            None
        }
        Err(_) => Some("Kudzu can't write to the folder it's in. Move it to Applications to keep it updated."),
    }
}

fn feed(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => STABLE_FEED,
        UpdateChannel::Beta => BETA_FEED,
    }
}

fn platform_key() -> &'static str {
    if cfg!(target_arch = "aarch64") { "darwin-aarch64" } else { "darwin-x86_64" }
}

/// SemVer precedence (as `scripts/release/feed.mjs`): pre-releases sort before their release.
pub fn newer(a: &str, b: &str) -> bool {
    fn parse(v: &str) -> (Vec<u64>, Vec<String>) {
        let v = v.trim_start_matches('v').split('+').next().unwrap_or("");
        let (core, pre) = v.split_once('-').map_or((v, ""), |(c, p)| (c, p));
        (core.split('.').map(|n| n.parse().unwrap_or(0)).collect(), if pre.is_empty() { vec![] } else { pre.split('.').map(String::from).collect() })
    }
    let (x, xp) = parse(a);
    let (y, yp) = parse(b);
    for i in 0..3 {
        let (p, q) = (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0));
        if p != q {
            return p > q;
        }
    }
    if xp.is_empty() || yp.is_empty() {
        return xp.is_empty() && !yp.is_empty();
    }
    for i in 0..xp.len().max(yp.len()) {
        match (xp.get(i), yp.get(i)) {
            (None, _) => return false,
            (_, None) => return true,
            (Some(p), Some(q)) if p == q => continue,
            (Some(p), Some(q)) => {
                return match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(p), Ok(q)) => p > q,
                    (Ok(_), Err(_)) => false,
                    (Err(_), Ok(_)) => true,
                    _ => p > q,
                };
            }
        }
    }
    false
}

fn curl(url: &str) -> Command {
    let mut c = Command::new("/usr/bin/curl");
    c.args(["--fail", "--silent", "--show-error", "--location", "--max-time", "600", "--user-agent"])
        .arg(format!("Kudzu/{}", super::identity::get().version))
        .arg(url);
    c
}

/// Verifies `data` against the release key and a base64 minisign signature file.
fn verify(data: &[u8], signature_b64: &str) -> Result<(), String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let decode = |s: &str| b64.decode(s.trim()).ok().and_then(|v| String::from_utf8(v).ok());
    let key = decode(PUBKEY).ok_or("bad release key")?;
    let key = minisign_verify::PublicKey::decode(&key).map_err(|e| e.to_string())?;
    let sig = decode(signature_b64).ok_or("The update's signature is unreadable.")?;
    let sig = minisign_verify::Signature::decode(&sig).map_err(|e| e.to_string())?;
    key.verify(data, &sig, true).map_err(|_| "The update's signature doesn't match. It wasn't installed.".to_string())
}

fn staging_dir() -> PathBuf {
    super::identity::data_dir().join("updates")
}

/// Checks the feed and, when there's a newer version, downloads and verifies it in the
/// background. `quiet` keeps scheduled checks from surfacing transient network errors.
/// Blocking (network): call it off the UI thread.
pub fn check(quiet: bool) -> Result<UpdateState, String> {
    if let Err(message) = update_allowed(&super::identity::get().id) {
        let next = UpdateState::Error { message: message.into(), needs_move: false };
        set(next.clone());
        return Ok(next);
    }
    if BUSY.swap(true, Ordering::SeqCst) {
        return Ok(state()); // a check or download is already running
    }
    struct Release;
    impl Drop for Release {
        fn drop(&mut self) {
            BUSY.store(false, Ordering::SeqCst);
        }
    }
    let _release = Release;
    if matches!(state(), UpdateState::Ready { .. } | UpdateState::Restarting { .. }) {
        return Ok(state()); // already holding an update; anything newer can wait for next launch
    }
    if let Some(problem) = location_problem() {
        let next = UpdateState::Error { message: problem.into(), needs_move: true };
        set(next.clone());
        return Ok(next);
    }
    if !quiet {
        set(UpdateState::Checking);
    }
    let fail = |message: String| {
        log::warn!("update: {message}");
        let next = if quiet { UpdateState::Idle } else { UpdateState::Error { message, needs_move: false } };
        set(next.clone());
        Ok(next)
    };

    let channel = core().store.lock().settings.update_channel;
    let manifest = match curl(feed(channel)).output() {
        Ok(out) if out.status.success() => match serde_json::from_slice::<Manifest>(&out.stdout) {
            Ok(m) => m,
            Err(e) => return fail(format!("The update feed is unreadable ({e}).")),
        },
        Ok(out) => return fail(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => return fail(e.to_string()),
    };
    if !newer(&manifest.version, &super::identity::get().version) {
        let next = UpdateState::UpToDate { checked_at: chrono::Utc::now().to_rfc3339() };
        set(next.clone());
        return Ok(next);
    }
    let Some(platform) = manifest.platforms.get(platform_key()) else {
        return fail("This update has no build for this Mac.".into());
    };

    let version = manifest.version.clone();
    set(UpdateState::Downloading { version: version.clone(), downloaded: 0, total: None });
    let dir = staging_dir();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let archive = dir.join("Kudzu.app.tar.gz");
    match curl(&platform.url).arg("--output").arg(&archive).output() {
        Ok(out) if out.status.success() => {}
        Ok(out) => return fail(String::from_utf8_lossy(&out.stderr).trim().to_string()),
        Err(e) => return fail(e.to_string()),
    }
    let bytes = std::fs::read(&archive).map_err(|e| e.to_string())?;
    set(UpdateState::Downloading { version: version.clone(), downloaded: bytes.len() as u64, total: Some(bytes.len() as u64) });
    if let Err(message) = verify(&bytes, &platform.signature) {
        let _ = std::fs::remove_dir_all(&dir);
        return fail(message);
    }
    *PENDING.lock() = Some(Pending { version: version.clone(), archive });
    let next = UpdateState::Ready { version, notes: manifest.notes, date: manifest.pub_date };
    set(next.clone());
    Ok(next)
}

/// Swaps the running bundle for the verified one: extract beside it, move the old one aside,
/// move the new one in. The old copy is removed only once the new one is in place.
fn swap_in(archive: &Path) -> Result<(), String> {
    let bundle = super::identity::bundle_dir().ok_or("Kudzu isn't running from an app bundle.")?;
    let parent = bundle.parent().ok_or("no parent")?;
    let unpack = staging_dir().join("unpacked");
    let _ = std::fs::remove_dir_all(&unpack);
    std::fs::create_dir_all(&unpack).map_err(|e| e.to_string())?;
    let status = Command::new("/usr/bin/tar").arg("-xzf").arg(archive).arg("-C").arg(&unpack).status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("The update archive couldn't be unpacked.".into());
    }
    let new_app = std::fs::read_dir(&unpack)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("The update archive has no app in it.")?;
    let aside = parent.join(format!(".{}-old", bundle.file_name().unwrap_or_default().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&aside);
    std::fs::rename(&bundle, &aside).map_err(|e| e.to_string())?;
    if let Err(e) = std::fs::rename(&new_app, &bundle) {
        let _ = std::fs::rename(&aside, &bundle);
        return Err(e.to_string());
    }
    let _ = std::fs::remove_dir_all(&aside);
    let _ = std::fs::remove_dir_all(staging_dir());
    Ok(())
}

/// Installs the waiting update. Returns true when something was installed.
fn install_pending() -> Result<bool, String> {
    update_allowed(&super::identity::get().id).map_err(str::to_string)?;
    let Some(p) = PENDING.lock().take() else { return Ok(false) };
    set(UpdateState::Restarting { version: p.version.clone() });
    match swap_in(&p.archive) {
        Ok(()) => Ok(true),
        Err(message) => {
            log::error!("update install failed: {message}");
            let needs_move = location_problem().is_some();
            set(UpdateState::Error { message: message.clone(), needs_move });
            Err(message)
        }
    }
}

/// "Restart to Update": install now and relaunch.
pub fn restart_to_update() -> Result<(), String> {
    if install_pending()? {
        let bundle = super::identity::bundle_dir().ok_or("no bundle")?;
        // A beat so the UI can show "Restarting…", then reopen once this process has exited.
        let pid = std::process::id();
        Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("while kill -0 {pid} 2>/dev/null; do sleep 0.1; done; /usr/bin/open \"$0\""))
            .arg(&bundle)
            .spawn()
            .map_err(|e| e.to_string())?;
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(400));
            core().shell().quit();
        });
    }
    Ok(())
}

/// Called on quit: if an update is waiting, swap it in so the next launch is the new version.
pub fn install_on_quit() {
    if update_allowed(&super::identity::get().id).is_err() {
        return;
    }
    match install_pending() {
        Ok(true) => log::info!("installed update on quit"),
        Ok(false) => {}
        Err(e) => log::error!("couldn't install update on quit: {e}"),
    }
}

/// Starts the background check loop, except in debug builds and Kudzu Dev.
pub fn start() {
    if cfg!(debug_assertions) || update_allowed(&super::identity::get().id).is_err() {
        return;
    }
    let _ = std::thread::Builder::new().name("kudzu-updater".into()).spawn(|| {
        std::thread::sleep(FIRST_CHECK_DELAY);
        loop {
            if core().store.lock().settings.auto_check_updates {
                let _ = check(true);
            }
            std::thread::sleep(CHECK_INTERVAL);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_variant_cannot_use_production_updates() {
        assert!(update_allowed("sh.saeed.kudzu").is_ok());
        assert!(update_allowed("sh.saeed.kudzu.dev").is_err());
    }

    #[test]
    fn versions_compare_like_the_feed_script() {
        assert!(newer("0.6.0", "0.5.0"));
        assert!(!newer("0.5.0", "0.5.0"));
        assert!(newer("0.5.0", "0.5.0-beta.2"));
        assert!(newer("0.5.0-beta.10", "0.5.0-beta.2"));
        assert!(!newer("0.5.0-beta.1", "0.5.0"));
        assert!(newer("1.0.0", "0.99.9"));
    }

    #[test]
    fn rejects_an_archive_the_release_key_did_not_sign() {
        assert!(verify(b"not the release", "bm90IGEgc2lnbmF0dXJl").is_err());
    }
}
