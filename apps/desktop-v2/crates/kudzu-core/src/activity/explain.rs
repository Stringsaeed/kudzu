//! Why an event did or didn't expand, in words: the label, tone, sentence and fix the
//! Activity pane shows for each outcome.

#[cfg(test)]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    ActivityEvent, ActivityOutcome, InsertFailure, Permission, RuleLevel, TapDisabledCause,
};

/// How an outcome reads at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum Tone {
    /// It expanded.
    Ok,
    /// Nothing is wrong (undo, cancelled form, a listener reset).
    Info,
    /// A setting of the user's skipped it.
    Skipped,
    /// The system blocked it.
    Blocked,
    /// It was attempted and failed.
    Failed,
}

/// The action that removes the cause of a skip.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(
    test,
    serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )
)]
pub enum Fix {
    /// Resume expanding.
    Resume,
    TurnOnSnippet,
    /// Open the snippet, where its app rule is edited.
    OpenSnippet,
    /// Open the group's settings (on/off switch and app rule).
    OpenGroup {
        group_id: Uuid,
    },
    /// Open Settings ▸ Apps.
    OpenAppSettings,
    /// Open the permission companion.
    OpenPermissions {
        permission: Permission,
    },
    /// Explain what Secure Input is and how to release it.
    ExplainSecureInput,
}

/// An outcome in words: a badge label, one plain sentence and a suggested fix.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub struct Explanation {
    pub label: String,
    pub tone: Tone,
    /// `None` when there is nothing to explain (it expanded).
    pub sentence: Option<String>,
    pub fix: Option<Fix>,
}

/// Explains `event` for the Activity pane.
pub fn explain(event: &ActivityEvent) -> Explanation {
    let app = event
        .app
        .as_ref()
        .map(|a| a.name.as_str())
        .filter(|n| !n.is_empty());
    let this_app = app.unwrap_or("this app");
    let (label, tone, sentence, fix): (&str, Tone, Option<String>, Option<Fix>) = match &event.outcome {
        ActivityOutcome::Expanded => ("Expanded", Tone::Ok, None, None),
        ActivityOutcome::Undone => (
            "Undone",
            Tone::Info,
            Some("You pressed Backspace right after it expanded, so Kudzu put the trigger back.".into()),
            None,
        ),
        ActivityOutcome::Paused => ("Paused", Tone::Skipped, Some("Kudzu was paused, so nothing expanded.".into()), Some(Fix::Resume)),
        ActivityOutcome::SnippetOff => ("Snippet off", Tone::Skipped, Some("This snippet is turned off.".into()), Some(Fix::TurnOnSnippet)),
        ActivityOutcome::GroupOff { group_id } => (
            "Group off",
            Tone::Skipped,
            Some("The snippet's group is turned off, which turns off everything inside it.".into()),
            Some(Fix::OpenGroup { group_id: *group_id }),
        ),
        ActivityOutcome::AppRule { level, group_id } => {
            let (sentence, fix) = match (level, group_id) {
                (RuleLevel::Snippet, _) => (format!("This snippet's app rule doesn't allow it in {this_app}."), Fix::OpenSnippet),
                (RuleLevel::Group, Some(id)) => (
                    format!("The snippet inherits an app rule from its group, and that rule doesn't allow {this_app}."),
                    Fix::OpenGroup { group_id: *id },
                ),
                (RuleLevel::Group, None) => (
                    format!("The snippet inherits an app rule from its group, and that rule doesn't allow {this_app}."),
                    Fix::OpenSnippet,
                ),
                (RuleLevel::AppSetting, _) => (
                    format!("{} is on your list of apps where nothing expands.", app.unwrap_or("This app")),
                    Fix::OpenAppSettings,
                ),
                (RuleLevel::Default, _) => (
                    format!("{} is excluded by default, like other password managers and the login window.", app.unwrap_or("This app")),
                    Fix::OpenAppSettings,
                ),
            };
            ("App rule", Tone::Skipped, Some(sentence), Some(fix))
        }
        ActivityOutcome::SecureInput { owner } => {
            let holder = owner.as_deref().filter(|o| !o.is_empty()).unwrap_or("A password field or app");
            (
                "Secure Input",
                Tone::Blocked,
                Some(format!("{holder} has Secure Input on, so macOS blocks every text expander until it is released.")),
                Some(Fix::ExplainSecureInput),
            )
        }
        ActivityOutcome::MissingPermission { permission } => {
            let sentence = match permission {
                Permission::Accessibility => "Kudzu doesn't have Accessibility access, so macOS won't let it type the expansion.",
                Permission::InputMonitoring => "Kudzu doesn't have Input Monitoring access, so it can't see triggers being typed.",
            };
            ("Needs permission", Tone::Blocked, Some(sentence.into()), Some(Fix::OpenPermissions { permission: *permission }))
        }
        ActivityOutcome::FormCancelled => ("Form cancelled", Tone::Info, Some("You closed the fill-in form, so nothing was inserted.".into()), None),
        ActivityOutcome::InsertionFailed { error } => {
            let (sentence, fix) = match error {
                InsertFailure::NotAllowed => (
                    "macOS refused Kudzu's keystrokes: Accessibility access was removed or Secure Input came on mid-insertion.".to_string(),
                    Some(Fix::OpenPermissions { permission: Permission::Accessibility }),
                ),
                InsertFailure::EventCreation => ("macOS couldn't create the keystrokes for this expansion.".to_string(), None),
                InsertFailure::PasteUnavailable => ("The clipboard couldn't be used to paste this expansion.".to_string(), None),
                InsertFailure::TargetGone => (format!("{} didn't come back to the front, so nothing was inserted.", app.unwrap_or("The app")), None),
            };
            ("Failed", Tone::Failed, Some(sentence), fix)
        }
        ActivityOutcome::Interrupted => (
            "Interrupted",
            Tone::Info,
            Some("You kept typing, clicked or switched apps before Kudzu could finish, so it left your text alone.".into()),
            None,
        ),
        ActivityOutcome::TapDisabled { cause, reenabled } => {
            let why = match cause {
                TapDisabledCause::Timeout => "macOS switched off Kudzu's keyboard listener because it answered too slowly.",
                TapDisabledCause::UserInput => "macOS switched off Kudzu's keyboard listener.",
            };
            let then = if *reenabled {
                "Kudzu turned it back on; a trigger typed in that moment was missed."
            } else {
                "Triggers won't expand until it is back."
            };
            ("Listener reset", if *reenabled { Tone::Info } else { Tone::Blocked }, Some(format!("{why} {then}")), None)
        }
        ActivityOutcome::ListenerLost => (
            "Not listening",
            Tone::Blocked,
            Some("Kudzu lost permission to watch the keyboard, so no trigger can expand.".into()),
            Some(Fix::OpenPermissions { permission: Permission::Accessibility }),
        ),
        ActivityOutcome::EngineStopped => ("Stopped", Tone::Blocked, Some("The expansion engine was stopped, so no trigger can expand.".into()), None),
    };
    Explanation {
        label: label.into(),
        tone,
        sentence,
        fix,
    }
}

/// An event with its explanation: what the Activity pane renders.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub struct ActivityEntry {
    #[cfg_attr(test, serde(flatten))]
    pub event: ActivityEvent,
    pub explanation: Explanation,
}

impl From<ActivityEvent> for ActivityEntry {
    fn from(event: ActivityEvent) -> Self {
        let explanation = explain(&event);
        Self { event, explanation }
    }
}
