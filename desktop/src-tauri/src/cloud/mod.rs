//! iCloud sync. The model and merge rules live in `kudzu_core::sync` (pure, unit-tested); this
//! module owns the lifecycle, status for the UI and, on macOS, the CloudKit layer (`apple.rs`).
//!
//! Sync only runs in builds signed by Kudzu's team with the iCloud entitlement (release builds).
//! Development and ad-hoc builds report themselves as unable to sync and stay local-only.

#[cfg(target_os = "macos")]
mod apple;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Emitted whenever sync status changes; payload is [`SyncStatus`].
pub const EV_SYNC: &str = "kudzu://sync";

/// Sync status as the UI shows it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SyncStatus {
    /// Turned off in Settings.
    #[default]
    Off,
    /// This build can't use iCloud (unsigned / dev build, or not macOS).
    Unavailable { reason: String },
    /// No iCloud account, or iCloud Drive / CloudKit restricted on this Mac.
    NoAccount { reason: String },
    Starting,
    Syncing,
    UpToDate {
        #[serde(rename = "lastSync")]
        last_sync: String,
    },
    /// Another Mac uses a newer sync format; this one stopped sending until it updates.
    NeedsUpdate,
    Error { message: String },
}

pub(crate) fn publish(app: &AppHandle, status: SyncStatus) {
    #[cfg(target_os = "macos")]
    apple::set_status(app, status.clone());
    let _ = app.emit(EV_SYNC, &status);
}

/// Current status (for windows that open later).
pub fn status(app: &AppHandle) -> SyncStatus {
    #[cfg(target_os = "macos")]
    {
        apple::status(app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        SyncStatus::Unavailable { reason: "iCloud sync is only available on macOS.".into() }
    }
}

/// Starts (or restarts) sync according to settings. Safe to call repeatedly.
pub fn start(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    apple::start(app);
    #[cfg(not(target_os = "macos"))]
    publish(app, status(app));
}

/// Stops syncing (the switch was turned off). Local data and sync state are kept.
pub fn stop(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    apple::stop(app);
    publish(app, SyncStatus::Off);
}

/// Called after any local library edit: queues the changed records for upload.
/// `include_usage` is false for edits (usage is batched by a timer, not sent per expansion).
pub fn local_changed(app: &AppHandle, include_usage: bool) {
    #[cfg(target_os = "macos")]
    apple::local_changed(app, include_usage);
    #[cfg(not(target_os = "macos"))]
    let _ = (app, include_usage);
}

/// Fetches and sends now ("Sync now", window focus).
pub fn sync_now(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    apple::sync_now(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}
