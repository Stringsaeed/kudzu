//! Recent expansion attempts and why each did or didn't expand ("Why didn't this expand?").
//!
//! Privacy contract, which every caller must keep:
//! - an event exists only when a trigger **fully matched** (or for engine health); near-misses
//!   and surrounding keystrokes are never recorded;
//! - an event names the snippet, its **stored** trigger and the frontmost app. It never holds
//!   typed text, expanded content, form values or clipboard contents;
//! - the log lives in memory only: a bounded ring ([`CAPACITY`]) that is dropped on quit. It is
//!   deliberately not `Serialize`, so it cannot be written to disk, synced or handed to agents.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How many events the log keeps.
pub const CAPACITY: usize = 100;

/// What started an expansion attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivitySource {
    /// A trigger typed in another app.
    Typed,
    /// An insert from Quick Search.
    QuickSearch,
    /// A fill-in form that was submitted or cancelled.
    Form,
}

/// Which level of the rule chain decided (snippet ▸ group ▸ app setting ▸ default).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleLevel {
    /// The snippet's own app rule.
    Snippet,
    /// A rule inherited from a group.
    Group,
    /// An app the user excluded in Settings ▸ Apps.
    AppSetting,
    /// A built-in exclusion (password managers, the login window).
    Default,
}

/// A macOS privacy permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    Accessibility,
    InputMonitoring,
}

/// Why macOS switched the keyboard tap off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TapDisabledCause {
    /// The callback took too long.
    Timeout,
    /// `kCGEventTapDisabledByUserInput`.
    UserInput,
}

/// Why an insertion failed. Codes only: never the text being inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertFailure {
    /// macOS refused synthetic keystrokes (permission revoked or Secure Input turned on).
    NotAllowed,
    /// A keyboard event could not be created.
    EventCreation,
    /// The pasteboard could not be used.
    PasteUnavailable,
    /// The app to insert into quit or would not come to the front.
    TargetGone,
}

/// What happened to a fully matched trigger, or to the engine itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActivityOutcome {
    Expanded,
    /// Backspace right after the expansion put the trigger back.
    Undone,
    Paused,
    SnippetOff,
    /// `group_id` is the nearest group that is turned off.
    GroupOff {
        group_id: Uuid,
    },
    /// `group_id` is the group whose rule decided, at [`RuleLevel::Group`].
    AppRule {
        level: RuleLevel,
        group_id: Option<Uuid>,
    },
    /// `owner` is the app holding Secure Input, when macOS reports it.
    SecureInput {
        owner: Option<String>,
    },
    MissingPermission {
        permission: Permission,
    },
    FormCancelled,
    InsertionFailed {
        error: InsertFailure,
    },
    /// Typing, a click or an app switch got in before the insertion could start.
    Interrupted,
    /// Health: macOS disabled the keyboard tap; `reenabled` when Kudzu turned it back on.
    TapDisabled {
        cause: TapDisabledCause,
        reenabled: bool,
    },
    /// Health: the keyboard tap was removed because its permissions went away.
    ListenerLost,
    /// Health: the engine was stopped.
    EngineStopped,
}

impl ActivityOutcome {
    /// Whether this describes the engine rather than one snippet.
    pub fn is_health(&self) -> bool {
        matches!(
            self,
            Self::TapDisabled { .. } | Self::ListenerLost | Self::EngineStopped
        )
    }
}

/// The app that was frontmost.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityApp {
    pub bundle_id: String,
    pub name: String,
    /// Bundle path, for the icon.
    #[serde(default)]
    pub path: Option<String>,
}

/// One expansion attempt or health change. See the module docs for what it may hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    /// Assigned by [`ActivityLog::push`]; increases with time.
    pub id: u64,
    pub at: DateTime<Utc>,
    /// `None` for health events.
    pub source: Option<ActivitySource>,
    pub snippet_id: Option<Uuid>,
    /// The trigger as stored on the snippet (never as typed). `None` for Quick Search.
    pub trigger: Option<String>,
    pub app: Option<ActivityApp>,
    pub outcome: ActivityOutcome,
}

impl ActivityEvent {
    /// An attempt on `snippet_id`, stamped now.
    pub fn attempt(
        source: ActivitySource,
        snippet_id: Uuid,
        trigger: Option<String>,
        app: Option<ActivityApp>,
        outcome: ActivityOutcome,
    ) -> Self {
        Self {
            id: 0,
            at: Utc::now(),
            source: Some(source),
            snippet_id: Some(snippet_id),
            trigger,
            app,
            outcome,
        }
    }

    /// An engine health event, stamped now.
    pub fn health(outcome: ActivityOutcome) -> Self {
        Self {
            id: 0,
            at: Utc::now(),
            source: None,
            snippet_id: None,
            trigger: None,
            app: None,
            outcome,
        }
    }
}

/// A bounded, in-memory ring of recent events. Not serializable on purpose.
#[derive(Debug, Clone)]
pub struct ActivityLog {
    events: VecDeque<ActivityEvent>,
    capacity: usize,
    next_id: u64,
}

impl ActivityLog {
    /// A log holding [`CAPACITY`] events.
    pub fn new() -> Self {
        Self::with_capacity(CAPACITY)
    }

    /// A log holding at most `capacity` events (at least one).
    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            next_id: 1,
        }
    }

    /// Appends `event`, dropping the oldest when full. Returns it with its id.
    pub fn push(&mut self, mut event: ActivityEvent) -> ActivityEvent {
        event.id = self.next_id;
        self.next_id += 1;
        if self.events.len() == self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event.clone());
        event
    }

    /// Events, newest first.
    pub fn recent(&self) -> Vec<ActivityEvent> {
        self.events.iter().rev().cloned().collect()
    }

    /// Forgets everything. Ids keep increasing so a late listener can't confuse old and new.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self::new()
    }
}

/// How an outcome reads at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    #[serde(flatten)]
    pub event: ActivityEvent,
    pub explanation: Explanation,
}

impl From<ActivityEvent> for ActivityEntry {
    fn from(event: ActivityEvent) -> Self {
        let explanation = explain(&event);
        Self { event, explanation }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str) -> Option<ActivityApp> {
        Some(ActivityApp {
            bundle_id: format!("com.example.{}", name.to_lowercase()),
            name: name.into(),
            path: None,
        })
    }

    fn typed(outcome: ActivityOutcome) -> ActivityEvent {
        ActivityEvent::attempt(
            ActivitySource::Typed,
            Uuid::nil(),
            Some(";sig".into()),
            app("Slack"),
            outcome,
        )
    }

    fn every_outcome() -> Vec<ActivityOutcome> {
        let mut all = vec![
            ActivityOutcome::Expanded,
            ActivityOutcome::Undone,
            ActivityOutcome::Paused,
            ActivityOutcome::SnippetOff,
            ActivityOutcome::GroupOff {
                group_id: Uuid::nil(),
            },
            ActivityOutcome::SecureInput { owner: None },
            ActivityOutcome::SecureInput {
                owner: Some("Terminal".into()),
            },
            ActivityOutcome::MissingPermission {
                permission: Permission::Accessibility,
            },
            ActivityOutcome::MissingPermission {
                permission: Permission::InputMonitoring,
            },
            ActivityOutcome::FormCancelled,
            ActivityOutcome::Interrupted,
            ActivityOutcome::TapDisabled {
                cause: TapDisabledCause::Timeout,
                reenabled: true,
            },
            ActivityOutcome::TapDisabled {
                cause: TapDisabledCause::UserInput,
                reenabled: false,
            },
            ActivityOutcome::ListenerLost,
            ActivityOutcome::EngineStopped,
        ];
        for level in [
            RuleLevel::Snippet,
            RuleLevel::Group,
            RuleLevel::AppSetting,
            RuleLevel::Default,
        ] {
            all.push(ActivityOutcome::AppRule {
                level,
                group_id: (level == RuleLevel::Group).then(Uuid::nil),
            });
        }
        for error in [
            InsertFailure::NotAllowed,
            InsertFailure::EventCreation,
            InsertFailure::PasteUnavailable,
            InsertFailure::TargetGone,
        ] {
            all.push(ActivityOutcome::InsertionFailed { error });
        }
        all
    }

    // MARK: Ring buffer

    #[test]
    fn push_assigns_increasing_ids_and_lists_newest_first() {
        let mut log = ActivityLog::new();
        let first = log.push(typed(ActivityOutcome::Expanded));
        let second = log.push(typed(ActivityOutcome::Paused));
        assert_eq!((first.id, second.id), (1, 2));
        let recent = log.recent();
        assert_eq!(recent.iter().map(|e| e.id).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(recent[0].outcome, ActivityOutcome::Paused);
    }

    #[test]
    fn the_ring_is_bounded_and_drops_the_oldest() {
        let mut log = ActivityLog::with_capacity(3);
        for _ in 0..10 {
            log.push(typed(ActivityOutcome::Expanded));
        }
        assert_eq!(log.len(), 3);
        assert_eq!(
            log.recent().iter().map(|e| e.id).collect::<Vec<_>>(),
            [10, 9, 8]
        );
    }

    #[test]
    fn the_default_capacity_is_about_a_hundred() {
        let mut log = ActivityLog::new();
        for _ in 0..(CAPACITY + 50) {
            log.push(typed(ActivityOutcome::Expanded));
        }
        assert_eq!(log.len(), CAPACITY);
        assert_eq!(CAPACITY, 100);
    }

    #[test]
    fn a_zero_capacity_still_holds_one_event() {
        let mut log = ActivityLog::with_capacity(0);
        log.push(typed(ActivityOutcome::Expanded));
        log.push(typed(ActivityOutcome::Paused));
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn clear_empties_the_log_but_ids_keep_increasing() {
        let mut log = ActivityLog::new();
        log.push(typed(ActivityOutcome::Expanded));
        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.push(typed(ActivityOutcome::Expanded)).id, 2);
    }

    // MARK: Event shape

    #[test]
    fn an_event_serializes_only_the_fields_the_privacy_contract_allows() {
        let mut event = typed(ActivityOutcome::AppRule {
            level: RuleLevel::Group,
            group_id: Some(Uuid::nil()),
        });
        event.id = 7;
        let json = serde_json::to_value(ActivityEntry::from(event)).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "app",
                "at",
                "explanation",
                "id",
                "outcome",
                "snippetId",
                "source",
                "trigger"
            ]
        );
        assert_eq!(json["source"], "typed");
        assert_eq!(
            json["outcome"],
            serde_json::json!({ "kind": "appRule", "level": "group", "groupId": Uuid::nil() })
        );
        assert_eq!(json["app"]["bundleId"], "com.example.slack");
    }

    #[test]
    fn outcomes_round_trip_through_json() {
        for outcome in every_outcome() {
            let json = serde_json::to_string(&outcome).unwrap();
            assert_eq!(
                serde_json::from_str::<ActivityOutcome>(&json).unwrap(),
                outcome,
                "{json}"
            );
        }
    }

    #[test]
    fn health_events_name_no_snippet_or_app() {
        let event = ActivityEvent::health(ActivityOutcome::ListenerLost);
        assert!(event.outcome.is_health());
        assert_eq!(
            (event.source, event.snippet_id, event.trigger, event.app),
            (None, None, None, None)
        );
        assert!(!ActivityOutcome::Paused.is_health());
    }

    // MARK: Explanations

    #[test]
    fn only_an_expansion_needs_no_sentence() {
        for outcome in every_outcome() {
            let explanation = explain(&typed(outcome.clone()));
            assert!(!explanation.label.is_empty());
            assert_eq!(
                explanation.sentence.is_none(),
                outcome == ActivityOutcome::Expanded,
                "{outcome:?}"
            );
            if let Some(sentence) = &explanation.sentence {
                assert!(sentence.ends_with('.'), "{sentence}");
            }
        }
        assert_eq!(explain(&typed(ActivityOutcome::Expanded)).tone, Tone::Ok);
    }

    #[test]
    fn user_settings_get_a_fix_that_undoes_them() {
        let fix = |outcome| explain(&typed(outcome)).fix;
        assert_eq!(fix(ActivityOutcome::Paused), Some(Fix::Resume));
        assert_eq!(fix(ActivityOutcome::SnippetOff), Some(Fix::TurnOnSnippet));
        let group_id = Uuid::new_v4();
        assert_eq!(
            fix(ActivityOutcome::GroupOff { group_id }),
            Some(Fix::OpenGroup { group_id })
        );
        assert_eq!(fix(ActivityOutcome::Expanded), None);
        assert_eq!(fix(ActivityOutcome::FormCancelled), None);
        assert_eq!(fix(ActivityOutcome::Interrupted), None);
    }

    #[test]
    fn each_rule_level_points_at_where_that_rule_is_edited() {
        let group_id = Uuid::new_v4();
        let rule = |level, group_id| explain(&typed(ActivityOutcome::AppRule { level, group_id }));
        let snippet = rule(RuleLevel::Snippet, None);
        assert_eq!(snippet.fix, Some(Fix::OpenSnippet));
        assert_eq!(
            snippet.sentence.as_deref(),
            Some("This snippet's app rule doesn't allow it in Slack.")
        );
        assert_eq!(
            rule(RuleLevel::Group, Some(group_id)).fix,
            Some(Fix::OpenGroup { group_id })
        );
        let setting = rule(RuleLevel::AppSetting, None);
        assert_eq!(setting.fix, Some(Fix::OpenAppSettings));
        assert_eq!(
            setting.sentence.as_deref(),
            Some("Slack is on your list of apps where nothing expands.")
        );
        let default = rule(RuleLevel::Default, None);
        assert_eq!(default.fix, Some(Fix::OpenAppSettings));
        assert!(default.sentence.unwrap().contains("excluded by default"));
        assert_eq!(default.tone, Tone::Skipped);
    }

    #[test]
    fn secure_input_names_its_owner_when_known() {
        let known = explain(&typed(ActivityOutcome::SecureInput {
            owner: Some("Terminal".into()),
        }));
        assert!(known
            .sentence
            .unwrap()
            .starts_with("Terminal has Secure Input on"));
        assert_eq!(known.fix, Some(Fix::ExplainSecureInput));
        assert_eq!(known.tone, Tone::Blocked);
        let unknown = explain(&typed(ActivityOutcome::SecureInput { owner: None }));
        assert!(unknown
            .sentence
            .unwrap()
            .starts_with("A password field or app has Secure Input on"));
    }

    #[test]
    fn missing_permissions_open_the_matching_companion() {
        for permission in [Permission::Accessibility, Permission::InputMonitoring] {
            let explanation = explain(&typed(ActivityOutcome::MissingPermission { permission }));
            assert_eq!(explanation.fix, Some(Fix::OpenPermissions { permission }));
            assert_eq!(explanation.tone, Tone::Blocked);
        }
    }

    #[test]
    fn failures_say_what_failed() {
        let failed = |error| explain(&typed(ActivityOutcome::InsertionFailed { error }));
        assert_eq!(
            failed(InsertFailure::NotAllowed).fix,
            Some(Fix::OpenPermissions {
                permission: Permission::Accessibility
            })
        );
        assert_eq!(
            failed(InsertFailure::TargetGone).sentence.as_deref(),
            Some("Slack didn't come back to the front, so nothing was inserted.")
        );
        assert_eq!(failed(InsertFailure::PasteUnavailable).tone, Tone::Failed);
    }

    #[test]
    fn sentences_read_naturally_without_an_app() {
        let mut event = typed(ActivityOutcome::AppRule {
            level: RuleLevel::AppSetting,
            group_id: None,
        });
        event.app = None;
        assert_eq!(
            explain(&event).sentence.as_deref(),
            Some("This app is on your list of apps where nothing expands.")
        );
        event.outcome = ActivityOutcome::AppRule {
            level: RuleLevel::Snippet,
            group_id: None,
        };
        assert_eq!(
            explain(&event).sentence.as_deref(),
            Some("This snippet's app rule doesn't allow it in this app.")
        );
    }

    #[test]
    fn a_tap_reset_is_informational_once_it_is_back() {
        let back = explain(&ActivityEvent::health(ActivityOutcome::TapDisabled {
            cause: TapDisabledCause::Timeout,
            reenabled: true,
        }));
        assert_eq!(back.tone, Tone::Info);
        assert!(back.sentence.unwrap().contains("answered too slowly"));
        let gone = explain(&ActivityEvent::health(ActivityOutcome::TapDisabled {
            cause: TapDisabledCause::UserInput,
            reenabled: false,
        }));
        assert_eq!(gone.tone, Tone::Blocked);
        assert_eq!(
            explain(&ActivityEvent::health(ActivityOutcome::ListenerLost)).fix,
            Some(Fix::OpenPermissions {
                permission: Permission::Accessibility
            })
        );
    }
}
