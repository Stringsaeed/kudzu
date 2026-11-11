//! Visible wording preserves the reference (D5). Insertion gates use can_post_events;
//! these historical sidebar/tray summaries do not prove permission or successful insertion.
use kudzu_core::system::{EngineStatus, TapMode};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Ready,
    Paused,
    Limited,
    Blocked,
    Secure,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCopy {
    pub health: Health,
    pub label: &'static str,
    pub detail: String,
}
pub fn sidebar_health(status: &EngineStatus) -> HealthCopy {
    let health = if status.paused {
        Health::Paused
    } else if status.secure_input {
        Health::Secure
    } else if status.tap == TapMode::Off
        || !status.permissions.input_monitoring && !status.permissions.accessibility
    {
        Health::Blocked
    } else if !status.permissions.accessibility || status.tap == TapMode::ListenOnly {
        Health::Limited
    } else {
        Health::Ready
    };
    let (label, detail) = match health {
        Health::Ready => (
            "Expanding",
            "Kudzu is listening for your triggers in every app.",
        ),
        Health::Paused => ("Paused", "Nothing expands until you resume."),
        Health::Limited => (
            "Limited",
            "Grant Accessibility so Kudzu can type expansions reliably.",
        ),
        Health::Blocked => (
            "Needs permission",
            "Kudzu can't see your typing yet. Open Settings → Permissions.",
        ),
        Health::Secure => (
            "Secure Input on",
            "A password field or app has Secure Input on, so macOS blocks expansion.",
        ),
    };
    let detail = if health == Health::Secure {
        status
            .secure_input_owner
            .as_deref()
            .filter(|owner| !owner.is_empty())
            .map(|owner| format!("{owner} has Secure Input on, so macOS blocks expansion."))
            .unwrap_or_else(|| detail.into())
    } else {
        detail.into()
    };
    HealthCopy {
        health,
        label,
        detail,
    }
}
pub fn tray_summary(status: &EngineStatus) -> String {
    if status.paused {
        "Paused".into()
    } else if status.secure_input {
        match &status.secure_input_owner {
            Some(owner) => format!("Blocked — Secure Input in {owner}"),
            None => "Blocked — Secure Input is on".into(),
        }
    } else if status.tap == TapMode::Off {
        "Needs permission to work".into()
    } else {
        "Expanding".into()
    }
}
