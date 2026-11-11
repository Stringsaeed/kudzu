use super::*;
use crate::fixtures::engine::FakeEngine;
use std::sync::Arc;

#[test]
fn overlapping_guards_keep_matching_suppressed_until_the_last_owner_leaves() {
    let engine = Arc::new(FakeEngine::default());
    let suppression = Suppression::new(engine.clone());
    let first = suppression.acquire();
    let second = suppression.acquire();
    assert!(engine.state.lock().unwrap().suppressed);
    drop(first);
    assert!(engine.state.lock().unwrap().suppressed);
    drop(second);
    assert!(!engine.state.lock().unwrap().suppressed);
}

use super::testing::{until, Harness};
use kudzu_core::library::model::{KudzuLibrary, SnippetRecord};

#[test]
fn engine_feed_reacts_to_matching_edits_but_not_usage_or_unrelated_settings() {
    let snippet = SnippetRecord::new("Test", vec![";test".into()], "body");
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet.clone()],
        ..Default::default()
    });
    let service = h.expansion();
    until(&service.feed(), |f| {
        f.library == h.library.watch().revision()
    });
    assert_eq!(h.engine.state.lock().unwrap().reloads, 1);
    h.library.record_expansion(snippet.id, 4).unwrap();
    h.settings.update(|s| s.typing_speed_wpm = 80).unwrap();
    until(&service.feed(), |f| {
        f.library == h.library.watch().revision() && f.settings == h.settings.watch().revision()
    });
    assert_eq!(h.engine.state.lock().unwrap().reloads, 1);
    let mut changed = snippet;
    changed.content = "edited body".into();
    h.library.update_snippet(changed).unwrap();
    until(&service.feed(), |f| {
        f.library == h.library.watch().revision()
    });
    assert_eq!(h.engine.state.lock().unwrap().reloads, 2);
    service.set_paused(true).unwrap();
    until(&service.feed(), |f| {
        f.settings == h.settings.watch().revision()
    });
    assert!(h.engine.state.lock().unwrap().status.paused);
    assert!(service.status().get().paused);
    assert_eq!(h.engine.state.lock().unwrap().reloads, 2);
    h.writer.shutdown().unwrap();
    assert_eq!(
        h.settings.saved().get().durable,
        h.settings.watch().revision()
    );
}

#[test]
fn quick_search_captures_only_needed_selection_before_returning_a_session() {
    use super::testing::finish;
    use kudzu_core::system::AppInfo;
    let h = Harness::new(KudzuLibrary::default());
    let target = AppInfo {
        bundle_id: "com.example.target".into(),
        name: "Target".into(),
        pid: Some(42),
        path: None,
    };
    *h.apps.frontmost.lock().unwrap() = Some(target.clone());
    *h.apps.selection.lock().unwrap() = Some("selected text".into());
    let service = h.expansion();
    let session = finish(service.begin_quick_search().unwrap()).unwrap();
    assert_eq!(session.target(), Some(&target));
    assert_eq!(session.selection(), None);
    assert_eq!(*h.apps.selection_reads.lock().unwrap(), 0);
    assert!(h.engine.state.lock().unwrap().suppressed);
    drop(session);
    assert!(!h.engine.state.lock().unwrap().suppressed);
    h.library
        .create_snippet(crate::library::SnippetDraft {
            content: "{{selection}}".into(),
            ..Default::default()
        })
        .unwrap();
    let session = finish(service.begin_quick_search().unwrap()).unwrap();
    assert_eq!(session.selection(), Some("selected text"));
    assert_eq!(*h.apps.selection_reads.lock().unwrap(), 1);
    *h.apps.frontmost.lock().unwrap() = None;
    finish(session.restore_target().unwrap()).unwrap();
    assert_eq!(*h.apps.frontmost.lock().unwrap(), Some(target));
    assert!(!h.engine.state.lock().unwrap().suppressed);
    *h.apps.failure.lock().unwrap() = Some(crate::error::PortError::TargetGone);
    assert!(finish(service.begin_quick_search().unwrap()).is_err());
    assert!(!h.engine.state.lock().unwrap().suppressed);
}

#[test]
fn previews_resolve_clipboard_history_selection_forms_and_copy_plain_plus_html() {
    use super::testing::finish;
    use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};
    let mut nested = SnippetRecord::new(
        "Nested",
        vec![],
        "{{selection}} {{field name=who default=Guest}}",
    );
    nested.id = uuid::Uuid::from_u128(17);
    let h = Harness::new(KudzuLibrary {
        snippets: vec![nested],
        ..Default::default()
    });
    h.pasteboard.external_copy(vec![vec![(
        "public.utf8-plain-text".into(),
        b"current copy".to_vec(),
    )]]);
    let service = h.expansion();
    let source = "{{clipboard}} / {{clipboard history=1}} / {{snippet id=00000000-0000-0000-0000-000000000011}}";
    let rendered = finish(
        service
            .preview(
                source.into(),
                PreviewContext {
                    selection: Some("selection".into()),
                    form_values: std::collections::HashMap::from([("who".into(), "Ada".into())]),
                    ..Default::default()
                },
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(rendered.text, "current copy / earlier copy / selection Ada");
    let rendered = finish(
        service
            .preview(
                "Bold".into(),
                PreviewContext {
                    formatting: Some(Formatting {
                        source: "Bold".into(),
                        spans: vec![FormatSpan {
                            start: 0,
                            end: 4,
                            style: FormatStyle::Bold,
                        }],
                    }),
                    ..Default::default()
                },
            )
            .unwrap(),
    )
    .unwrap();
    finish(service.copy_rendered(rendered).unwrap()).unwrap();
    let snapshot = h.pasteboard.snapshot.lock().unwrap();
    assert_eq!(
        snapshot.items[0][0],
        ("public.utf8-plain-text".into(), b"Bold".to_vec())
    );
    assert_eq!(snapshot.items[0][1].0, "public.html");
    assert!(String::from_utf8(snapshot.items[0][1].1.clone())
        .unwrap()
        .contains("<strong>Bold</strong>"));
}

#[test]
fn explicit_insertion_preserves_rul9_and_reports_outcomes_after_admission() {
    use super::testing::finish;
    use kudzu_core::{
        library::model::{AppRule, AppRuleMode, GroupRecord},
        system::{AppInfo, TapMode},
    };
    let mut group = GroupRecord::new("Off group");
    group.is_enabled = false;
    let mut snippet = SnippetRecord::new("Off snippet", vec![], "body");
    snippet.group_id = Some(group.id);
    snippet.is_enabled = false;
    snippet.rule = Some(AppRule {
        mode: AppRuleMode::Allow,
        bundle_ids: vec![],
        insertion: None,
    });
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        groups: vec![group],
        ..Default::default()
    });
    {
        let mut engine = h.engine.state.lock().unwrap();
        engine.status.can_post_events = true;
        engine.status.tap = TapMode::Active;
    }
    let service = h.expansion();
    until(&service.feed(), |f| {
        f.library == h.library.watch().revision()
    });
    let target = AppInfo {
        bundle_id: "com.example.target".into(),
        name: "Target".into(),
        pid: Some(42),
        path: None,
    };
    finish(
        service
            .insert(id, Some(target.clone()), Some("selection".into()))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(h.engine.state.lock().unwrap().insertions.len(), 1);
    assert!(
        h.library.watch().get().usage.is_empty(),
        "accepting an insert is not proof of insertion"
    );
    h.wire_outcomes(&service);
    h.engine.resolve_insert(0);
    assert_eq!(
        service.outcomes().get().as_ref().unwrap().outcome,
        InsertionOutcome::Expanded
    );
    h.settings
        .update(|s| s.denied_bundle_ids.push(target.bundle_id.clone()))
        .unwrap();
    until(&service.feed(), |f| {
        f.settings == h.settings.watch().revision()
    });
    assert_eq!(
        finish(service.insert(id, Some(target), None).unwrap()),
        Ok(())
    );
    h.engine.resolve_insert(1);
    assert_eq!(
        service.outcomes().get().as_ref().unwrap().outcome,
        InsertionOutcome::Blocked
    );
    service.set_paused(true).unwrap();
    until(&service.feed(), |f| {
        f.settings == h.settings.watch().revision()
    });
    assert_eq!(finish(service.insert(id, None, None).unwrap()), Ok(()));
    h.engine.resolve_insert(2);
    assert_eq!(
        service.outcomes().get().as_ref().unwrap().outcome,
        InsertionOutcome::Blocked
    );
}

#[test]
fn sidebar_and_tray_health_keep_reference_priority_and_wording() {
    use kudzu_core::system::{PermissionStatus, TapMode};
    let engine = FakeEngine::default();
    let mut status = engine.state.lock().unwrap().status.clone();
    assert_eq!(sidebar_health(&status).label, "Needs permission");
    assert_eq!(tray_summary(&status), "Needs permission to work");
    status.tap = TapMode::ListenOnly;
    status.permissions.input_monitoring = true;
    assert_eq!(sidebar_health(&status).label, "Limited");
    status.permissions = PermissionStatus {
        accessibility: true,
        input_monitoring: true,
    };
    status.tap = TapMode::Active;
    // D5 preserves the reference's visible wording even when posting is separately denied.
    assert_eq!(sidebar_health(&status).label, "Expanding");
    status.secure_input = true;
    status.secure_input_owner = Some("Terminal".into());
    assert_eq!(sidebar_health(&status).label, "Secure Input on");
    assert_eq!(
        sidebar_health(&status).detail,
        "Terminal has Secure Input on, so macOS blocks expansion."
    );
    assert_eq!(tray_summary(&status), "Blocked — Secure Input in Terminal");
    status.paused = true;
    assert_eq!(sidebar_health(&status).label, "Paused");
    assert_eq!(tray_summary(&status), "Paused");
}

mod events;

mod recovery;

#[test]
fn resync_reloads_library_and_pause_after_restart_without_a_store_edit() {
    let snippet = SnippetRecord::new("Fixture", vec![";f".into()], "body");
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet.clone()],
        ..Default::default()
    });
    h.settings.update(|s| s.paused = true).unwrap();
    let service = h.expansion();
    until(&service.feed(), |f| {
        f.settings == h.settings.watch().revision()
    });
    {
        let mut engine = h.engine.state.lock().unwrap();
        engine.library = Default::default();
        engine.status.paused = false;
    }
    let generation = service.resync();
    until(&service.feed(), |f| f.generation == generation);
    let engine = h.engine.state.lock().unwrap();
    assert_eq!(engine.library.snippets, [snippet]);
    assert!(engine.status.paused);
    assert_eq!(engine.reloads, 2);
}

#[test]
fn preview_does_not_read_unneeded_clipboard_and_read_errors_mean_empty_text() {
    use super::testing::finish;
    let h = Harness::new(KudzuLibrary::default());
    let service = h.expansion();
    *h.pasteboard.failure.lock().unwrap() = Some(crate::error::PortError::Unavailable);
    assert_eq!(
        finish(
            service
                .preview("plain text".into(), PreviewContext::default())
                .unwrap()
        )
        .unwrap()
        .text,
        "plain text"
    );
    assert_eq!(
        finish(
            service
                .preview("x{{clipboard}}y".into(), PreviewContext::default())
                .unwrap()
        )
        .unwrap()
        .text,
        "xy"
    );
}

#[test]
fn nested_clipboard_templates_use_only_text_reads_and_escaped_tags_do_not_read() {
    use super::testing::finish;
    let mut nested = SnippetRecord::new("Nested", vec![], "{{clipboard}}");
    nested.id = uuid::Uuid::from_u128(81);
    let h = Harness::new(KudzuLibrary {
        snippets: vec![nested],
        ..Default::default()
    });
    h.pasteboard.external_copy(vec![vec![
        ("public.png".into(), vec![0; 1024]),
        ("public.utf8-plain-text".into(), b"clipboard text".to_vec()),
    ]]);
    let service = h.expansion();
    finish(
        service
            .preview(r"\{{clipboard}}".into(), PreviewContext::default())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(*h.pasteboard.text_reads.lock().unwrap(), 0);
    let result = finish(
        service
            .preview(
                "{{snippet id=00000000-0000-0000-0000-000000000051}}".into(),
                PreviewContext::default(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result.text, "clipboard text");
    assert_eq!(*h.pasteboard.text_reads.lock().unwrap(), 1);
    assert_eq!(*h.pasteboard.reads.lock().unwrap(), 0);
}

mod admission;
