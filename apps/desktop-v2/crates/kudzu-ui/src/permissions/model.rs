//! Pure view-model and derived copy for the permission companion (`S66`).

use std::path::PathBuf;

use kudzu_app::permissions::{
    accessibility_name, granted, ready, PermissionsState as AppPermissionsState, Phase,
    ACCESSIBILITY_NAME,
};
use kudzu_app::ports::PermissionKind;
use kudzu_core::system::EngineStatus;

use crate::shell::theme_sync::ThemeSync;

/// Message shown when the current process is a loose dev binary rather than a `.app` bundle.
pub const UNBUNDLED_REVEAL_NOTICE: &str =
    "Running outside a .app bundle — build or install the app bundle to reveal or drag it into System Settings.";

/// Primary action button state at the bottom of the companion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrimaryAction {
    OpenSettings { enabled: bool },
    Opening,
    SelectAccessibility,
    Restart { label: String },
    Done,
}

/// Live status region rendered between the permission cards and the primary button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusBanner {
    Error(String),
    Ready {
        subtitle: String,
    },
    Granted {
        subtitle: String,
    },
    Waiting {
        app_name: String,
        permission_name: &'static str,
    },
    Idle(&'static str),
}

/// Companion view-model synchronized from `Permissions` + `SettingsService` watches.
pub struct PermissionsModel {
    pub theme: ThemeSync,
    pub open: bool,
    pub snap: AppPermissionsState,
    pub loaded: bool,
    pub help_open: bool,
    pub notice: Option<String>,
    pub bundle_path: Option<PathBuf>,
    pub drag_available: bool,
    /// True when `--scene` configured a deterministic parity fixture.
    pub scene_locked: bool,
    /// True in `--scene` parity captures where the reference `<main class="p-4">` inset applies.
    pub scene_frame: bool,
}

impl Default for PermissionsModel {
    fn default() -> Self {
        let status = EngineStatus {
            can_post_events: false,
            can_expand: false,
            paused: false,
            tap: kudzu_core::system::TapMode::Off,
            permissions: kudzu_core::system::PermissionStatus::default(),
            secure_input: false,
            secure_input_owner: None,
            summary: String::new(),
        };
        Self {
            theme: ThemeSync::default(),
            open: false,
            snap: AppPermissionsState {
                selected: PermissionKind::Accessibility,
                phase: Phase::Checking,
                ready: ready(&status),
                restart_offered: false,
                accessibility_name: ACCESSIBILITY_NAME,
                app_name: "Kudzu".to_owned(),
                status,
                error: None,
            },
            loaded: false,
            help_open: false,
            notice: None,
            bundle_path: None,
            drag_available: false,
            scene_locked: false,
            scene_frame: false,
        }
    }
}

impl PermissionsModel {
    pub fn app_name(&self) -> &str {
        if self.snap.app_name.is_empty() {
            "Kudzu"
        } else {
            self.snap.app_name.as_str()
        }
    }

    pub fn permission_name(&self, kind: PermissionKind) -> &'static str {
        match kind {
            PermissionKind::Accessibility => self.snap.accessibility_name,
            PermissionKind::InputMonitoring => "Input Monitoring",
        }
    }

    pub fn permission_description(kind: PermissionKind) -> &'static str {
        match kind {
            PermissionKind::Accessibility => "Insert snippets into other apps.",
            PermissionKind::InputMonitoring => "Additional keyboard monitoring access.",
        }
    }

    pub fn card_granted(&self, kind: PermissionKind) -> bool {
        granted(&self.snap.status, kind)
    }

    pub fn selected_granted(&self) -> bool {
        self.card_granted(self.snap.selected)
    }

    pub fn needs_connection(&self) -> bool {
        self.snap.status.permissions.accessibility && !self.snap.ready
    }

    pub fn drag_hint(&self) -> &'static str {
        if self.drag_available {
            "Drag into System Settings to add the app"
        } else if self.scene_frame {
            "Your app. Your permission."
        } else {
            UNBUNDLED_REVEAL_NOTICE
        }
    }

    pub fn headline(&self) -> &'static str {
        if self.snap.ready {
            "You’re all set."
        } else if self.needs_connection() {
            "Access is allowed."
        } else {
            "Let Kudzu do the typing."
        }
    }

    pub fn detail(&self) -> &'static str {
        if !self.loaded {
            "Checking access on this Mac…"
        } else if self.snap.ready {
            if self.snap.status.can_expand {
                "Your shortcuts can now become full sentences."
            } else if self.snap.status.paused {
                "Permissions are set. Expansion is currently paused."
            } else {
                "Permissions are set. Close the secure input field to expand."
            }
        } else if self.needs_connection() {
            "One last step to connect Kudzu to your keyboard."
        } else {
            "A little permission to turn your triggers into text, wherever you write."
        }
    }

    pub fn status_banner(&self) -> StatusBanner {
        if let Some(msg) = self.notice.as_deref().or(self.snap.error.as_deref()) {
            return StatusBanner::Error(msg.to_owned());
        }
        let selected_granted = self.selected_granted();
        if self.snap.ready && selected_granted {
            let subtitle = if self.snap.status.can_expand {
                "Ready to expand in your other apps.".to_owned()
            } else {
                self.snap.status.summary.clone()
            };
            return StatusBanner::Ready { subtitle };
        }
        if selected_granted {
            let subtitle = if self.needs_connection() {
                if self.snap.restart_offered {
                    format!(
                        "macOS may need a restart of {} to apply access. We’ll bring you back here.",
                        self.app_name()
                    )
                } else {
                    "Connecting to your keyboard…".to_owned()
                }
            } else {
                format!(
                    "Next, allow {} so Kudzu can insert text.",
                    self.snap.accessibility_name
                )
            };
            return StatusBanner::Granted { subtitle };
        }
        if self.snap.phase == Phase::Waiting {
            return StatusBanner::Waiting {
                app_name: self.app_name().to_owned(),
                permission_name: self.permission_name(self.snap.selected),
            };
        }
        StatusBanner::Idle(
            "We’ll open the right page and stay beside you. macOS may ask you to confirm with Touch ID or your password.",
        )
    }

    pub fn primary_action(&self) -> PrimaryAction {
        let selected_granted = self.selected_granted();
        if self.snap.ready && selected_granted {
            PrimaryAction::Done
        } else if self.needs_connection() && self.snap.restart_offered {
            PrimaryAction::Restart {
                label: format!("Restart {}", self.app_name()),
            }
        } else if self.snap.selected == PermissionKind::InputMonitoring && selected_granted {
            PrimaryAction::SelectAccessibility
        } else if self.snap.phase == Phase::Opening {
            PrimaryAction::Opening
        } else {
            PrimaryAction::OpenSettings {
                enabled: self.loaded,
            }
        }
    }

    /// Helper used by tests and OS-version checks.
    pub fn with_os_major(mut self, os_major: u32) -> Self {
        self.snap.accessibility_name = accessibility_name(os_major);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::system::{PermissionStatus, TapMode};

    fn loaded_model() -> PermissionsModel {
        PermissionsModel {
            loaded: true,
            ..PermissionsModel::default()
        }
    }

    #[test]
    fn os_version_switches_accessibility_name_at_macos_27() {
        let m26 = loaded_model().with_os_major(26);
        assert_eq!(
            m26.permission_name(PermissionKind::Accessibility),
            "Accessibility"
        );

        let m27 = loaded_model().with_os_major(27);
        assert_eq!(
            m27.permission_name(PermissionKind::Accessibility),
            "Device Control and Data Access"
        );
        assert_eq!(
            m27.permission_name(PermissionKind::InputMonitoring),
            "Input Monitoring"
        );
    }

    #[test]
    fn missing_ready_paused_secure_and_restart_copy_match_contract() {
        let mut m = loaded_model();
        assert_eq!(m.headline(), "Let Kudzu do the typing.");
        assert_eq!(
            m.primary_action(),
            PrimaryAction::OpenSettings { enabled: true }
        );
        assert!(matches!(m.status_banner(), StatusBanner::Idle(_)));

        // Granted accessibility while tap is still connecting:
        m.snap.status.permissions.accessibility = true;
        m.snap.ready = false;
        assert_eq!(m.headline(), "Access is allowed.");
        assert_eq!(
            m.status_banner(),
            StatusBanner::Granted {
                subtitle: "Connecting to your keyboard…".to_owned()
            }
        );

        // After 5s restart_offered threshold:
        m.snap.restart_offered = true;
        assert_eq!(
            m.primary_action(),
            PrimaryAction::Restart {
                label: "Restart Kudzu".to_owned()
            }
        );
        assert_eq!(
            m.status_banner(),
            StatusBanner::Granted {
                subtitle:
                    "macOS may need a restart of Kudzu to apply access. We’ll bring you back here."
                        .to_owned()
            }
        );

        // Ready & active:
        m.snap.ready = true;
        m.snap.status.can_post_events = true;
        m.snap.status.can_expand = true;
        m.snap.status.tap = TapMode::Active;
        assert_eq!(m.headline(), "You’re all set.");
        assert_eq!(m.detail(), "Your shortcuts can now become full sentences.");
        assert_eq!(m.primary_action(), PrimaryAction::Done);
        assert_eq!(
            m.status_banner(),
            StatusBanner::Ready {
                subtitle: "Ready to expand in your other apps.".to_owned()
            }
        );

        // Paused vs secure input:
        m.snap.status.can_expand = false;
        m.snap.status.paused = true;
        m.snap.status.summary = "Paused".to_owned();
        assert_eq!(
            m.detail(),
            "Permissions are set. Expansion is currently paused."
        );
        assert_eq!(
            m.status_banner(),
            StatusBanner::Ready {
                subtitle: "Paused".to_owned()
            }
        );

        m.snap.status.paused = false;
        m.snap.status.secure_input = true;
        assert_eq!(
            m.detail(),
            "Permissions are set. Close the secure input field to expand."
        );
    }

    #[test]
    fn input_monitoring_selected_prompts_accessibility_when_already_granted() {
        let mut m = loaded_model();
        m.snap.selected = PermissionKind::InputMonitoring;
        m.snap.status.permissions = PermissionStatus {
            accessibility: false,
            input_monitoring: true,
        };
        assert_eq!(m.primary_action(), PrimaryAction::SelectAccessibility);
        assert_eq!(
            m.status_banner(),
            StatusBanner::Granted {
                subtitle: "Next, allow Accessibility so Kudzu can insert text.".to_owned()
            }
        );

        m.snap.status.permissions.input_monitoring = false;
        assert_eq!(
            m.primary_action(),
            PrimaryAction::OpenSettings { enabled: true }
        );
    }

    #[test]
    fn unbundled_drag_hint_explains_disabled_drag_before_interaction() {
        let mut m = loaded_model();
        assert_eq!(m.drag_hint(), UNBUNDLED_REVEAL_NOTICE);

        m.scene_frame = true;
        assert_eq!(m.drag_hint(), "Your app. Your permission.");

        m.scene_frame = false;
        m.drag_available = true;
        assert_eq!(m.drag_hint(), "Drag into System Settings to add the app");

        m.notice = Some(UNBUNDLED_REVEAL_NOTICE.to_owned());
        assert_eq!(
            m.status_banner(),
            StatusBanner::Error(UNBUNDLED_REVEAL_NOTICE.to_owned())
        );
    }
}
