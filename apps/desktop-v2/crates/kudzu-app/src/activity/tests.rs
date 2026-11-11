use super::*;
use crate::expansion::testing::Harness;
use kudzu_core::{
    activity::{ActivityEvent, ActivityOutcome, ActivitySource},
    library::model::{KudzuLibrary, SnippetRecord},
};

#[test]
fn ring_is_newest_first_capped_and_ids_survive_clear_without_content_or_disk_access() {
    let snippet = SnippetRecord::new("Snippet", vec![";saved".into()], "private snippet body");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let expansion = h.expansion();
    let service = ActivityService::new(
        h.library.clone(),
        h.settings.clone(),
        expansion.status(),
        h.clock.clone(),
    );
    for _ in 0..105 {
        service.record(ActivityEvent::attempt(
            ActivitySource::Typed,
            id,
            Some(";saved".into()),
            None,
            ActivityOutcome::Expanded,
        ));
    }
    let entries = service.watch().get();
    assert_eq!(entries.len(), 100);
    assert_eq!(entries[0].event.id, 105);
    assert_eq!(entries[99].event.id, 6);
    assert_eq!(entries[0].event.trigger.as_deref(), Some(";saved"));
    service.clear();
    assert!(service.watch().get().is_empty());
    service.record(ActivityEvent::attempt(
        ActivitySource::Typed,
        id,
        Some("typed or clipboard content".into()),
        None,
        ActivityOutcome::Interrupted,
    ));
    let entries = service.watch().get();
    assert_eq!(entries[0].event.id, 106);
    assert_eq!(entries[0].event.trigger, None);
    assert!(!format!("{entries:?}").contains("private snippet body"));
    assert!(!format!("{entries:?}").contains("typed or clipboard content"));
}

#[test]
fn remedies_recheck_current_state_apply_only_typed_edits_and_do_not_resurrect_records() {
    use kudzu_core::activity::Fix;
    let mut snippet = SnippetRecord::new("Off", vec![], "body");
    snippet.is_enabled = false;
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let expansion = h.expansion();
    let service = ActivityService::new(
        h.library.clone(),
        h.settings.clone(),
        expansion.status(),
        h.clock.clone(),
    );
    service.record(ActivityEvent::attempt(
        ActivitySource::Typed,
        id,
        None,
        None,
        ActivityOutcome::SnippetOff,
    ));
    let entry = service.watch().get()[0].clone();
    let remedy = service.remedy(&entry).unwrap();
    assert_eq!(remedy.state, RemedyState::Action(Fix::TurnOnSnippet));
    assert!(matches!(
        service.apply(remedy).unwrap(),
        AppliedRemedy::Changed(_)
    ));
    assert!(h.library.watch().get().snippets[0].is_enabled);
    assert_eq!(
        service.remedy(&entry).unwrap().state,
        RemedyState::FixedSince
    );
    assert_eq!(service.remedy(&entry).unwrap().label, "Fixed since");
    h.settings.update(|s| s.paused = true).unwrap();
    service.record(ActivityEvent::attempt(
        ActivitySource::QuickSearch,
        id,
        None,
        None,
        ActivityOutcome::Paused,
    ));
    let paused = service.watch().get()[0].clone();
    let remedy = service.remedy(&paused).unwrap();
    h.settings.update(|s| s.paused = false).unwrap();
    assert_eq!(service.apply(remedy).unwrap(), AppliedRemedy::FixedSince);
    let (_, _) = h.library.delete_snippets(&[id]).unwrap();
    assert!(service.remedy(&entry).is_none());
    service.clear();
    assert_eq!(
        service.apply(service.remedy(&paused).unwrap()).unwrap(),
        AppliedRemedy::Unavailable
    );
}

#[test]
fn every_outcome_retains_its_explanation_fix_and_current_state_resolution() {
    use kudzu_core::{
        activity::{ActivityApp, Fix, InsertFailure, Permission, RuleLevel, TapDisabledCause},
        library::model::{AppRule, AppRuleMode, GroupRecord},
        system::{PermissionStatus, TapMode},
    };
    let mut group = GroupRecord::new("Off group");
    group.is_enabled = false;
    group.rule = Some(AppRule {
        mode: AppRuleMode::Allow,
        bundle_ids: vec![],
        insertion: None,
    });
    let group_id = group.id;
    let mut snippet = SnippetRecord::new("Off", vec![], "private");
    snippet.is_enabled = false;
    snippet.group_id = Some(group_id);
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        groups: vec![group],
        ..Default::default()
    });
    h.settings
        .update(|s| {
            s.paused = true;
            s.denied_bundle_ids.push("target.app".into());
        })
        .unwrap();
    h.engine.state.lock().unwrap().status.secure_input = true;
    let expansion = h.expansion();
    let service = ActivityService::new(
        h.library.clone(),
        h.settings.clone(),
        expansion.status(),
        h.clock.clone(),
    );
    let permission = Fix::OpenPermissions {
        permission: Permission::Accessibility,
    };
    let cases = vec![
        (ActivityOutcome::Expanded, None, "Expanded"),
        (ActivityOutcome::Undone, None, "Undone"),
        (ActivityOutcome::Paused, Some(Fix::Resume), "Paused"),
        (
            ActivityOutcome::SnippetOff,
            Some(Fix::TurnOnSnippet),
            "Snippet off",
        ),
        (
            ActivityOutcome::GroupOff { group_id },
            Some(Fix::OpenGroup { group_id }),
            "Group off",
        ),
        (
            ActivityOutcome::AppRule {
                level: RuleLevel::Snippet,
                group_id: None,
            },
            Some(Fix::OpenSnippet),
            "App rule",
        ),
        (
            ActivityOutcome::AppRule {
                level: RuleLevel::Group,
                group_id: Some(group_id),
            },
            Some(Fix::OpenGroup { group_id }),
            "App rule",
        ),
        (
            ActivityOutcome::AppRule {
                level: RuleLevel::Group,
                group_id: None,
            },
            Some(Fix::OpenSnippet),
            "App rule",
        ),
        (
            ActivityOutcome::AppRule {
                level: RuleLevel::AppSetting,
                group_id: None,
            },
            Some(Fix::OpenAppSettings),
            "App rule",
        ),
        (
            ActivityOutcome::AppRule {
                level: RuleLevel::Default,
                group_id: None,
            },
            Some(Fix::OpenAppSettings),
            "App rule",
        ),
        (
            ActivityOutcome::SecureInput {
                owner: Some("Terminal".into()),
            },
            Some(Fix::ExplainSecureInput),
            "Secure Input",
        ),
        (
            ActivityOutcome::MissingPermission {
                permission: Permission::Accessibility,
            },
            Some(permission.clone()),
            "Needs permission",
        ),
        (
            ActivityOutcome::MissingPermission {
                permission: Permission::InputMonitoring,
            },
            Some(Fix::OpenPermissions {
                permission: Permission::InputMonitoring,
            }),
            "Needs permission",
        ),
        (ActivityOutcome::FormCancelled, None, "Form cancelled"),
        (
            ActivityOutcome::InsertionFailed {
                error: InsertFailure::NotAllowed,
            },
            Some(permission.clone()),
            "Failed",
        ),
        (
            ActivityOutcome::InsertionFailed {
                error: InsertFailure::EventCreation,
            },
            None,
            "Failed",
        ),
        (
            ActivityOutcome::InsertionFailed {
                error: InsertFailure::PasteUnavailable,
            },
            None,
            "Failed",
        ),
        (
            ActivityOutcome::InsertionFailed {
                error: InsertFailure::TargetGone,
            },
            None,
            "Failed",
        ),
        (ActivityOutcome::Interrupted, None, "Interrupted"),
        (
            ActivityOutcome::TapDisabled {
                cause: TapDisabledCause::Timeout,
                reenabled: true,
            },
            None,
            "Listener reset",
        ),
        (
            ActivityOutcome::TapDisabled {
                cause: TapDisabledCause::UserInput,
                reenabled: false,
            },
            None,
            "Listener reset",
        ),
        (
            ActivityOutcome::ListenerLost,
            Some(permission),
            "Not listening",
        ),
        (ActivityOutcome::EngineStopped, None, "Stopped"),
    ];
    let mut actionable = Vec::new();
    for (outcome, fix, label) in cases {
        h.library
            .transact(|lib| {
                lib.snippets[0].rule = matches!(
                    outcome,
                    ActivityOutcome::AppRule {
                        level: RuleLevel::Snippet,
                        ..
                    }
                )
                .then(|| AppRule {
                    mode: AppRuleMode::Allow,
                    bundle_ids: vec![],
                    insertion: None,
                });
                Ok(())
            })
            .unwrap();
        service.record(ActivityEvent::attempt(
            ActivitySource::Typed,
            id,
            None,
            Some(ActivityApp {
                bundle_id: "target.app".into(),
                name: "Target".into(),
                path: None,
            }),
            outcome,
        ));
        let entry = service.watch().get()[0].clone();
        assert_eq!(entry.explanation.label, label);
        assert_eq!(entry.explanation.fix, fix);
        assert_eq!(
            service.remedy(&entry).map(|r| r.state),
            fix.map(RemedyState::Action),
            "{}",
            entry.explanation.label
        );
        if let Some(remedy) = service.remedy(&entry) {
            if !matches!(
                remedy.state,
                RemedyState::Action(Fix::Resume | Fix::TurnOnSnippet)
            ) {
                assert!(matches!(
                    service.apply(remedy).unwrap(),
                    AppliedRemedy::Show { .. }
                ));
            }
            actionable.push(entry);
        }
    }
    h.library
        .transact(|lib| {
            lib.snippets[0].is_enabled = true;
            lib.snippets[0].rule = None;
            lib.groups[0].is_enabled = true;
            lib.groups[0].rule = None;
            Ok(())
        })
        .unwrap();
    h.settings
        .update(|s| {
            s.paused = false;
            s.denied_bundle_ids.clear();
        })
        .unwrap();
    crate::expansion::testing::until(&expansion.feed(), |f| {
        f.settings == h.settings.watch().revision()
    });
    let mut ready = (*expansion.status().get()).clone();
    ready.secure_input = false;
    ready.can_post_events = true;
    ready.tap = TapMode::Active;
    ready.permissions = PermissionStatus {
        accessibility: true,
        input_monitoring: true,
    };
    expansion.status_changed(ready);
    for entry in actionable {
        assert_eq!(
            service.remedy(&entry).unwrap().state,
            RemedyState::FixedSince,
            "{}",
            entry.explanation.label
        );
    }
}
