//! Self-updates through tauri-plugin-updater.
//!
//! Flow: check → download quietly (signature verified) → hold the verified package in memory →
//! install when the user quits, or immediately via "Restart to Update". Kudzu never restarts on
//! its own: a text expander restarting mid-sentence would drop the trigger being typed.
//!
//! Feeds: stable is `latest.json` on the newest published release. Beta users read `beta.json`
//! from the rolling `update-feed` release, which CI keeps pointing at whichever is newer, the
//! latest beta or the latest stable, so beta users never fall behind stable.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Url};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::settings::UpdateChannel;
use crate::state::core;

/// Emitted whenever the update state changes; payload is [`UpdateState`].
pub const EV_UPDATE: &str = "kudzu://update";

const STABLE_FEED: &str = "https://github.com/Stringsaeed/kudzu/releases/latest/download/latest.json";
const BETA_FEED: &str = "https://github.com/Stringsaeed/kudzu/releases/download/update-feed/beta.json";
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(20);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);

/// What the UI shows about updates.
#[derive(Debug, Clone, Serialize, Default)]
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

struct Pending {
    update: Update,
    bytes: Vec<u8>,
}

/// Managed state: the last result plus the verified package waiting to be installed.
#[derive(Default)]
pub struct Updates {
    state: Mutex<UpdateState>,
    pending: Mutex<Option<Pending>>,
    /// Serialises checks so the timer and a "Check now" click can't download twice.
    busy: tokio::sync::Mutex<()>,
}

fn updates(app: &AppHandle) -> Arc<Updates> {
    app.state::<Arc<Updates>>().inner().clone()
}

fn set(app: &AppHandle, next: UpdateState) {
    *updates(app).state.lock() = next.clone();
    crate::tray::set_update(app, &next);
    let _ = app.emit(EV_UPDATE, &next);
}

/// Current state, for windows that open after a check finished.
pub fn state(app: &AppHandle) -> UpdateState {
    app.try_state::<Arc<Updates>>().map(|u| u.state.lock().clone()).unwrap_or_default()
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

fn feed(channel: UpdateChannel) -> Url {
    Url::parse(match channel {
        UpdateChannel::Stable => STABLE_FEED,
        UpdateChannel::Beta => BETA_FEED,
    })
    .expect("feed url")
}

/// Checks the feed and, when there's a newer version, downloads and verifies it in the
/// background. `quiet` keeps scheduled checks from surfacing transient network errors.
pub async fn check(app: AppHandle, quiet: bool) -> Result<UpdateState, String> {
    let u = updates(&app);
    let Ok(_guard) = u.busy.try_lock() else {
        return Ok(state(&app)); // a check or download is already running
    };
    if matches!(*u.state.lock(), UpdateState::Ready { .. } | UpdateState::Restarting { .. }) {
        return Ok(state(&app)); // already holding an update; anything newer can wait for next launch
    }
    if let Some(problem) = location_problem() {
        let next = UpdateState::Error { message: problem.into(), needs_move: true };
        set(&app, next.clone());
        return Ok(next);
    }
    if !quiet {
        set(&app, UpdateState::Checking);
    }

    let channel = core(&app).store.lock().settings.update_channel;
    let found = async {
        app.updater_builder()
            .endpoints(vec![feed(channel)])
            .map_err(|e| e.to_string())?
            .build()
            .map_err(|e| e.to_string())?
            .check()
            .await
            .map_err(|e| e.to_string())
    }
    .await;

    let update = match found {
        Ok(Some(update)) => update,
        Ok(None) => {
            let next = UpdateState::UpToDate { checked_at: chrono::Utc::now().to_rfc3339() };
            set(&app, next.clone());
            return Ok(next);
        }
        Err(message) => {
            log::warn!("update check failed: {message}");
            let next = if quiet { state(&app) } else { UpdateState::Error { message, needs_move: false } };
            set(&app, next.clone());
            return Ok(next);
        }
    };

    // Download (and verify) in the background.
    let version = update.version.clone();
    let mut downloaded = 0u64;
    let mut last_percent = u64::MAX;
    let progress_app = app.clone();
    let progress_version = version.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                // One event per whole percent, not per network chunk.
                let percent = total.map(|t| downloaded * 100 / t.max(1)).unwrap_or(downloaded >> 20);
                if percent != last_percent {
                    last_percent = percent;
                    set(&progress_app, UpdateState::Downloading { version: progress_version.clone(), downloaded, total });
                }
            },
            || {},
        )
        .await;
    match bytes {
        Ok(bytes) => {
            let next = UpdateState::Ready { version, notes: update.body.clone(), date: update.date.map(|d| d.to_string()) };
            *u.pending.lock() = Some(Pending { update, bytes });
            set(&app, next.clone());
            Ok(next)
        }
        Err(e) => {
            let message = e.to_string();
            log::warn!("update download failed: {message}");
            let next = if quiet { UpdateState::Idle } else { UpdateState::Error { message, needs_move: false } };
            set(&app, next.clone());
            Ok(next)
        }
    }
}

/// Installs the waiting update. Returns true when something was installed.
fn install_pending(app: &AppHandle) -> Result<bool, String> {
    let u = updates(app);
    let Some(p) = u.pending.lock().take() else { return Ok(false) };
    set(app, UpdateState::Restarting { version: p.update.version.clone() });
    match p.update.install(&p.bytes) {
        Ok(()) => Ok(true),
        Err(e) => {
            let message = e.to_string();
            log::error!("update install failed: {message}");
            let needs_move = location_problem().is_some();
            set(app, UpdateState::Error { message: message.clone(), needs_move });
            Err(message)
        }
    }
}

/// "Restart to Update": install now and relaunch.
pub async fn restart_to_update(app: AppHandle) -> Result<(), String> {
    if install_pending(&app)? {
        // A beat so the UI can show "Restarting…".
        tokio::time::sleep(Duration::from_millis(400)).await;
        app.restart();
    }
    Ok(())
}

/// Called on quit: if an update is waiting, swap it in so the next launch is the new version.
pub fn install_on_quit(app: &AppHandle) {
    if app.try_state::<Arc<Updates>>().is_none() {
        return;
    }
    match install_pending(app) {
        Ok(true) => log::info!("installed update on quit"),
        Ok(false) => {}
        Err(e) => log::error!("couldn't install update on quit: {e}"),
    }
}

/// Starts the background check loop. Debug builds skip it: there's no feed for dev versions.
pub fn start(app: &AppHandle) {
    app.manage(Arc::new(Updates::default()));
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            if core(&app).store.lock().settings.auto_check_updates {
                let _ = check(app.clone(), true).await;
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}
