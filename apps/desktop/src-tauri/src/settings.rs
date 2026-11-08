//! App settings persisted as `settings.json`. Field names match the TypeScript `Settings` type.

use std::collections::HashMap;

use kudzu_core::matching::MatchingConfig;
use kudzu_core::InsertionMethod;
use serde::{Deserialize, Serialize};

/// Appearance preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

/// Which update feed to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    /// Pre-releases too (and stable releases whenever they're newer).
    Beta,
}

/// Everything the user can configure outside the library itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    #[serde(rename = "typingSpeedWPM")]
    pub typing_speed_wpm: u32,
    #[serde(rename = "defaultTriggerPrefix")]
    pub default_trigger_prefix: String,
    #[serde(rename = "paletteHotkey")]
    pub palette_hotkey: String,
    pub paused: bool,
    #[serde(rename = "defaultInsertion")]
    pub default_insertion: InsertionMethod,
    #[serde(rename = "deniedBundleIDs")]
    pub denied_bundle_ids: Vec<String>,
    #[serde(rename = "appInsertions")]
    pub app_insertions: HashMap<String, InsertionMethod>,
    #[serde(rename = "launchAtLogin")]
    pub launch_at_login: bool,
    #[serde(rename = "onboardingCompleted")]
    pub onboarding_completed: bool,
    pub theme: Theme,
    #[serde(rename = "showInDock")]
    pub show_in_dock: bool,
    /// Look for new versions in the background (release builds only).
    #[serde(rename = "autoCheckUpdates")]
    pub auto_check_updates: bool,
    #[serde(rename = "updateChannel")]
    pub update_channel: UpdateChannel,
    /// Sync the library through iCloud (release builds signed for the container).
    #[serde(rename = "icloudSync")]
    pub icloud_sync: bool,
    /// The app version that last ran with this data; a change means "just updated".
    #[serde(rename = "lastRunVersion")]
    pub last_run_version: Option<String>,
    /// Record clipboard history.
    #[serde(rename = "clipboardEnabled")]
    pub clipboard_enabled: bool,
    /// Days to keep unpinned clipboard history; 0 keeps it until the size cap.
    #[serde(rename = "clipboardRetentionDays")]
    pub clipboard_retention_days: u32,
    /// Global shortcut for the clipboard history picker.
    #[serde(rename = "clipboardHotkey")]
    pub clipboard_hotkey: String,
    /// Apps whose copies are never recorded.
    #[serde(rename = "clipboardIgnoredApps")]
    pub clipboard_ignored_apps: Vec<String>,
    /// Paste formatted text as plain text by default (⇧ flips it).
    #[serde(rename = "clipboardPastePlain")]
    pub clipboard_paste_plain: bool,
    /// Answer AI agents (MCP) on the local socket.
    #[serde(rename = "agentAccess")]
    pub agent_access: bool,
    /// Agents may create and edit snippets.
    #[serde(rename = "agentWrite")]
    pub agent_write: bool,
    /// Agents may search clipboard history (secrets are always withheld).
    #[serde(rename = "agentClipboard")]
    pub agent_clipboard: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut denied: Vec<String> = MatchingConfig::default_denied_bundle_ids().into_iter().collect();
        denied.sort();
        Self {
            typing_speed_wpm: 40,
            default_trigger_prefix: ";".into(),
            palette_hotkey: "Control+Alt+Space".into(),
            paused: false,
            default_insertion: InsertionMethod::Automatic,
            denied_bundle_ids: denied,
            app_insertions: MatchingConfig::default_app_insertions(),
            launch_at_login: false,
            onboarding_completed: false,
            theme: Theme::Dark,
            show_in_dock: true,
            auto_check_updates: true,
            update_channel: UpdateChannel::Stable,
            icloud_sync: true,
            last_run_version: None,
            clipboard_enabled: true,
            clipboard_retention_days: kudzu_core::clipboard::DEFAULT_RETENTION_DAYS,
            clipboard_hotkey: "Control+Alt+V".into(),
            clipboard_ignored_apps: kudzu_core::clipboard::default_ignored_apps(),
            clipboard_paste_plain: false,
            agent_access: false,
            agent_write: true,
            agent_clipboard: true,
        }
    }
}

impl Settings {
    /// The engine's view of these settings.
    pub fn matching_config(&self) -> MatchingConfig {
        MatchingConfig {
            default_insertion: self.default_insertion,
            denied_bundle_ids: self.denied_bundle_ids.iter().cloned().collect(),
            app_insertions: self.app_insertions.clone(),
            separators: MatchingConfig::default_separators(),
        }
    }
}
