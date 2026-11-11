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
        ActivityOutcome::TriggerLost,
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

#[test]
fn lost_trigger_has_a_content_free_failure_explanation() {
    let explanation = explain(&typed(ActivityOutcome::TriggerLost));
    assert_eq!(explanation.label, "Trigger removed");
    assert_eq!(explanation.tone, Tone::Failed);
    assert_eq!(
        explanation.sentence.as_deref(),
        Some("The trigger was removed; the snippet couldn't be inserted.")
    );
    assert_eq!(explanation.fix, None);
    assert!(!ActivityOutcome::TriggerLost.is_health());
}
