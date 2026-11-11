//! Sync status and the sentences the settings screen shows.
//!
//! S63 displays `reason` / `message` as written here. It supplies its own line for Off
//! ("Off. Your snippets stay on this Mac."), Starting and Syncing ("Syncing…"), UpToDate
//! (relative time from `last_sync`), and NeedsUpdate ("Another Mac uses a newer Kudzu.
//! Update this one to keep syncing; nothing here is lost meanwhile."). Sync now is offered
//! only for UpToDate and Error. NeedsUpdate's button opens About; this service does not.

use std::time::{Duration, SystemTime};

/// How often a running session fetches. Usage is checked on this tick, not on its own timer.
pub const FETCH_INTERVAL: Duration = Duration::from_secs(3 * 60);
/// Nominal usage period. Four fetch ticks cross it at 12 minutes.
pub const USAGE_INTERVAL: Duration = Duration::from_secs(10 * 60);

/// CloudKit container. Zone and record types live in `kudzu_core::sync`.
pub const CONTAINER_ID: &str = "iCloud.sh.saeed.kudzu";

pub const DEV_UNAVAILABLE: &str =
    "iCloud sync is unavailable in Kudzu Dev. Data stays on this Mac.";
pub const UNENTITLED: &str = "This copy of Kudzu isn't signed for iCloud. Release builds sync; development builds stay on this Mac.";
pub const NO_ACCOUNT: &str = "Sign in to iCloud in System Settings to sync your snippets.";
pub const RESTRICTED: &str = "iCloud is restricted on this Mac.";
pub const TEMPORARILY_UNAVAILABLE: &str =
    "iCloud is temporarily unavailable. Kudzu will try again.";
pub const SIGNED_OUT: &str = "Signed out of iCloud. Your snippets stay on this Mac.";
pub const QUOTA: &str = "Your iCloud storage is full, so new changes aren't syncing.";
pub const PERMANENT_FAILURE: &str =
    "iCloud couldn't save these changes. Your snippets stay on this Mac.";
pub const START_FAILED: &str = "iCloud couldn't start. Kudzu will try again.";
pub const SAVE_FAILED: &str =
    "Kudzu couldn't record sync progress. Your snippets stay on this Mac.";
pub const WORKER_STOPPED: &str = "Sync stopped unexpectedly. Your snippets stay on this Mac.";

/// What Settings shows. `last_sync` is an instant; S63 formats it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Off,
    Unavailable { reason: String },
    NoAccount { reason: String },
    Starting,
    Syncing,
    UpToDate { last_sync: SystemTime },
    NeedsUpdate,
    Error { message: String },
}

/// Dev and Kudzu Next never sync. `None` means a production bundle may start the port.
pub fn dev_gate(production: bool, enabled: bool) -> Option<SyncStatus> {
    if production {
        return None;
    }
    Some(if enabled {
        SyncStatus::Unavailable {
            reason: DEV_UNAVAILABLE.into(),
        }
    } else {
        SyncStatus::Off
    })
}
