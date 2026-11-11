//! `SystemSettings`: the permission grants, the system prompts and the Settings panes. Opening
//! moved here from the engine in S35; the grants and prompts stay the engine's, which polls
//! them. `request` shows a system prompt and `open` launches System Settings: never call them
//! from tests.
use kudzu_app::{
    error::PortError,
    ports::{PermissionKind, SettingsPane, SystemSettings},
};
use kudzu_core::system::PermissionStatus;
use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSString, NSURL};

#[derive(Default)]
pub struct SettingsPanes;

impl SystemSettings for SettingsPanes {
    /// Cheap; safe to poll.
    fn permissions(&self) -> PermissionStatus {
        kudzu_engine::system::permissions()
    }

    fn request(&self, kind: PermissionKind) -> Result<(), PortError> {
        match kind {
            PermissionKind::Accessibility => kudzu_engine::system::request_accessibility(),
            PermissionKind::InputMonitoring => kudzu_engine::system::request_input_monitoring(),
        }
        Ok(())
    }

    /// `Unavailable` when Launch Services refuses the URL.
    fn open(&self, pane: SettingsPane) -> Result<(), PortError> {
        let opened = autoreleasepool(|_| {
            NSURL::URLWithString(&NSString::from_str(pane_url(pane)))
                .is_some_and(|url| NSWorkspace::sharedWorkspace().openURL(&url))
        });
        opened.then_some(()).ok_or(PortError::Unavailable)
    }
}

fn pane_url(pane: SettingsPane) -> &'static str {
    match pane {
        SettingsPane::Accessibility => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        }
        SettingsPane::InputMonitoring => {
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
        }
        SettingsPane::LoginItems => {
            "x-apple.systempreferences:com.apple.LoginItems-Settings.extension"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_panes_pass_the_settings_contract() {
        kudzu_app::fixtures::contract::settings_panes::reads(&SettingsPanes);
    }

    #[test]
    fn each_pane_has_the_tauri_apps_deep_link() {
        assert!(pane_url(SettingsPane::Accessibility).ends_with("?Privacy_Accessibility"));
        assert!(pane_url(SettingsPane::InputMonitoring).ends_with("?Privacy_ListenEvent"));
        assert!(pane_url(SettingsPane::LoginItems).ends_with("LoginItems-Settings.extension"));
        for pane in [
            SettingsPane::Accessibility,
            SettingsPane::InputMonitoring,
            SettingsPane::LoginItems,
        ] {
            assert!(autoreleasepool(|_| NSURL::URLWithString(
                &NSString::from_str(pane_url(pane))
            )
            .is_some()));
        }
    }
}
