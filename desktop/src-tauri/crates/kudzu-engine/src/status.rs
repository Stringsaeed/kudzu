//! Engine status and its one-line summary.

use crate::{EngineStatus, PermissionStatus, TapMode};

/// Inputs to [`build`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StatusInputs {
    pub paused: bool,
    pub tap: TapMode,
    pub permissions: PermissionStatus,
    /// `CGPreflightPostEventAccess`: needed to type the expansion.
    pub post_event: bool,
    pub secure_input: bool,
    pub secure_input_owner: Option<String>,
}

/// Builds the status with its summary.
pub(crate) fn build(inputs: StatusInputs) -> EngineStatus {
    let summary = summary(&inputs);
    EngineStatus {
        can_post_events: inputs.post_event,
        can_expand: inputs.tap != TapMode::Off && inputs.post_event && !inputs.paused && !inputs.secure_input,
        paused: inputs.paused,
        tap: inputs.tap,
        permissions: inputs.permissions,
        secure_input: inputs.secure_input,
        secure_input_owner: if inputs.secure_input { inputs.secure_input_owner } else { None },
        summary,
    }
}

fn summary(s: &StatusInputs) -> String {
    if s.paused {
        return "Paused".into();
    }
    if s.secure_input {
        return match &s.secure_input_owner {
            Some(owner) => format!("Secure Input on ({owner}) — expansion blocked"),
            None => "Secure Input on — expansion blocked".into(),
        };
    }
    match s.tap {
        TapMode::Off if !s.permissions.accessibility && !s.permissions.input_monitoring => "Needs Accessibility".into(),
        TapMode::Off => "Keyboard monitoring unavailable".into(),
        _ if !s.post_event => "Needs Accessibility to type".into(),
        TapMode::ListenOnly => "Ready (limited — grant Accessibility for best reliability)".into(),
        TapMode::Active => "Ready".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> StatusInputs {
        StatusInputs {
            paused: false,
            tap: TapMode::Active,
            permissions: PermissionStatus { accessibility: true, input_monitoring: true },
            post_event: true,
            secure_input: false,
            secure_input_owner: None,
        }
    }

    #[test]
    fn ready_when_active() {
        assert_eq!(build(inputs()).summary, "Ready");
    }

    #[test]
    fn readiness_requires_a_listener_and_permission_to_insert() {
        assert!(build(inputs()).can_expand);
        assert!(!build(StatusInputs { tap: TapMode::Off, ..inputs() }).can_expand);
        let listening_only = build(StatusInputs { tap: TapMode::ListenOnly, post_event: false, ..inputs() });
        assert!(!listening_only.can_expand);
        assert!(!listening_only.can_post_events);
        assert!(build(StatusInputs { tap: TapMode::ListenOnly, ..inputs() }).can_expand);
    }

    #[test]
    fn grants_do_not_override_pause_or_secure_input() {
        assert!(!build(StatusInputs { paused: true, ..inputs() }).can_expand);
        assert!(!build(StatusInputs { secure_input: true, ..inputs() }).can_expand);
        assert!(build(StatusInputs { paused: true, ..inputs() }).can_post_events);
    }

    #[test]
    fn paused_wins() {
        let status = build(StatusInputs { paused: true, secure_input: true, ..inputs() });
        assert_eq!(status.summary, "Paused");
        assert!(status.paused);
    }

    #[test]
    fn secure_input_names_owner() {
        let s = StatusInputs { secure_input: true, secure_input_owner: Some("Terminal".into()), ..inputs() };
        let status = build(s);
        assert_eq!(status.summary, "Secure Input on (Terminal) — expansion blocked");
        assert_eq!(status.secure_input_owner.as_deref(), Some("Terminal"));
        let status = build(StatusInputs { secure_input: true, ..inputs() });
        assert_eq!(status.summary, "Secure Input on — expansion blocked");
    }

    #[test]
    fn owner_dropped_when_secure_input_off() {
        let status = build(StatusInputs { secure_input_owner: Some("X".into()), ..inputs() });
        assert_eq!(status.secure_input_owner, None);
    }

    #[test]
    fn missing_permissions() {
        let none = PermissionStatus::default();
        let s = StatusInputs { tap: TapMode::Off, permissions: none, post_event: false, ..inputs() };
        assert_eq!(build(s).summary, "Needs Accessibility");
        let s = StatusInputs { tap: TapMode::Off, ..inputs() };
        assert_eq!(build(s).summary, "Keyboard monitoring unavailable");
        let s = StatusInputs {
            tap: TapMode::ListenOnly,
            permissions: PermissionStatus { accessibility: false, input_monitoring: true },
            post_event: false,
            ..inputs()
        };
        assert_eq!(build(s).summary, "Needs Accessibility to type");
    }

    #[test]
    fn listen_only_is_limited() {
        let s = StatusInputs { tap: TapMode::ListenOnly, ..inputs() };
        assert!(build(s).summary.starts_with("Ready (limited"));
    }
}
