//! Settings JSON retains the reference keys; variant selection comes only from composition.
use kudzu_core::{library::model::InsertionMethod, matching::MatchingConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Production,
    Dev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[serde(rename = "autoCheckUpdates")]
    pub auto_check_updates: bool,
    #[serde(rename = "updateChannel")]
    pub update_channel: UpdateChannel,
    #[serde(rename = "icloudSync")]
    pub icloud_sync: bool,
    #[serde(rename = "lastRunVersion")]
    pub last_run_version: Option<String>,
    #[serde(rename = "clipboardEnabled")]
    pub clipboard_enabled: bool,
    #[serde(rename = "clipboardRetentionDays")]
    pub clipboard_retention_days: u32,
    #[serde(rename = "clipboardHotkey")]
    pub clipboard_hotkey: String,
    #[serde(rename = "clipboardIgnoredApps")]
    pub clipboard_ignored_apps: Vec<String>,
    #[serde(rename = "clipboardPastePlain")]
    pub clipboard_paste_plain: bool,
    #[serde(rename = "clipboardShowSecrets")]
    pub clipboard_show_secrets: bool,
    #[serde(rename = "agentAccess")]
    pub agent_access: bool,
    #[serde(rename = "agentWrite")]
    pub agent_write: bool,
    #[serde(rename = "agentClipboard")]
    pub agent_clipboard: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut denied_bundle_ids: Vec<_> = MatchingConfig::default_denied_bundle_ids()
            .into_iter()
            .collect();
        denied_bundle_ids.sort();
        Self {
            typing_speed_wpm: 40,
            default_trigger_prefix: ";".into(),
            palette_hotkey: "Control+Alt+Space".into(),
            paused: false,
            default_insertion: InsertionMethod::Automatic,
            denied_bundle_ids,
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
            clipboard_show_secrets: false,
            agent_access: false,
            agent_write: true,
            agent_clipboard: true,
        }
    }
}

impl Settings {
    pub fn for_variant(variant: Variant) -> Self {
        match variant {
            Variant::Production => Self::default(),
            Variant::Dev => Self {
                paused: true,
                palette_hotkey: "Control+Alt+Shift+Space".into(),
                clipboard_hotkey: "Control+Alt+Shift+V".into(),
                ..Self::default()
            },
        }
    }

    pub fn matching_config(&self) -> MatchingConfig {
        MatchingConfig {
            default_insertion: self.default_insertion,
            denied_bundle_ids: self.denied_bundle_ids.iter().cloned().collect(),
            app_insertions: self.app_insertions.clone(),
            separators: MatchingConfig::default_separators(),
        }
    }
}
