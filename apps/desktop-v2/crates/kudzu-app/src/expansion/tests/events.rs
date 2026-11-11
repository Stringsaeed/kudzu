use crate::{
    activity::ActivityService,
    expansion::{
        testing::{finish, until, Harness, Memory},
        ExpansionService,
    },
    forms::FormService,
    library::LibraryService,
    persist::Revision,
    ports::{EngineEvents, Expansion, ExpansionEngine, FormRequest},
};
use kudzu_core::{
    activity::{ActivityEvent, ActivityOutcome},
    library::model::{KudzuLibrary, SnippetRecord},
    system::EngineStatus,
    template::FormMemory,
};
use std::sync::{Arc, Weak};

// Composition owns this routing in S40. Weak owners break the engine/router/service cycle.
struct Router {
    expansion: Weak<ExpansionService>,
    library: Weak<LibraryService>,
    forms: Weak<FormService>,
    activity: Weak<ActivityService>,
}
impl EngineEvents for Router {
    fn status_changed(&self, status: EngineStatus) {
        if let Some(service) = self.expansion.upgrade() {
            service.status_changed(status);
        }
    }
    fn expanded(&self, event: Expansion) {
        if let Some(library) = self.library.upgrade() {
            library
                .record_expansion(event.snippet_id, event.chars_saved)
                .unwrap();
        }
    }
    fn form_requested(&self, request: FormRequest) {
        if let Some(forms) = self.forms.upgrade() {
            forms.requested(request);
        }
    }
    fn trigger_lost(&self, lost: crate::ports::TriggerLost) {
        if let Some(expansion) = self.expansion.upgrade() {
            let event = expansion.trigger_lost(lost);
            self.activity(event);
        }
    }
    fn activity(&self, event: ActivityEvent) {
        if let Some(activity) = self.activity.upgrade() {
            activity.record(event);
        }
    }
    fn clipboard_history(&self, _: usize, _: Option<&str>) -> Option<String> {
        None
    }
}
#[test]
fn typed_engine_callbacks_route_to_service_snapshots_and_post_insertion_usage() {
    let snippet = SnippetRecord::new("Form", vec![";f".into()], "{{field name=who}}");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let expansion = h.expansion();
    let forms = Arc::new(
        FormService::new(
            FormMemory::default(),
            h.writer.register(Memory::default(), Revision(0)).unwrap(),
            h.library.watch(),
            expansion.clone(),
            h.engine.clone(),
            h.apps.clone(),
        )
        .unwrap(),
    );
    let activity = Arc::new(ActivityService::new(
        h.library.clone(),
        h.settings.clone(),
        expansion.status(),
        h.clock.clone(),
    ));
    let router = Arc::new(Router {
        expansion: Arc::downgrade(&expansion),
        library: Arc::downgrade(&h.library),
        forms: Arc::downgrade(&forms),
        activity: Arc::downgrade(&activity),
    });
    h.engine.start(router.clone()).unwrap();
    until(&expansion.feed(), |f| {
        f.library == h.library.watch().revision()
    });
    h.engine.emit_form(FormRequest {
        token: 1,
        snippet_id: id,
        snippet_name: "Form".into(),
        fields: vec![],
        target: None,
        caret: None,
        selection: None,
    });
    assert_eq!(forms.pending().get().as_ref().as_ref().unwrap().token, 1);
    finish(forms.submit(1, Default::default()).unwrap().unwrap());
    assert!(h.library.watch().get().usage.is_empty());
    h.engine.emit_expansion(Expansion {
        snippet_id: id,
        chars_saved: 23,
    });
    let library = h.library.watch().get();
    assert_eq!(library.usage.len(), 1);
    assert_eq!(
        (library.usage[0].count, library.usage[0].chars_saved),
        (1, 23)
    );
    assert_eq!(library.usage[0].device_id, "device-s27");
    until(&expansion.feed(), |f| {
        f.library == h.library.watch().revision()
    });
    assert_eq!(h.engine.state.lock().unwrap().reloads, 1);
    router.activity(ActivityEvent::health(ActivityOutcome::ListenerLost));
    assert_eq!(activity.watch().get()[0].explanation.label, "Not listening");
    let mut status = (*expansion.status().get()).clone();
    status.secure_input = true;
    h.engine.emit_status(status);
    assert!(expansion.status().get().secure_input);
    forms.stop_pruning();
    expansion.stop_feed();
    h.engine.stop();
    h.writer.shutdown().unwrap();
}

#[test]
fn lost_trigger_routes_to_a_content_free_notice_and_memory_only_activity() {
    use crate::{fixtures::engine::FakeEngineEvents, ports::TriggerLost};
    let mut snippet =
        SnippetRecord::new("Fixture", vec![";PRIVATE-TRIGGER".into()], "PRIVATE-BODY");
    snippet.id = uuid::Uuid::from_u128(27);
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let expansion = h.expansion();
    let activity = Arc::new(ActivityService::new(
        h.library.clone(),
        h.settings.clone(),
        expansion.status(),
        h.clock.clone(),
    ));
    h.engine
        .start(Arc::new(Router {
            expansion: Arc::downgrade(&expansion),
            library: Arc::downgrade(&h.library),
            forms: Weak::new(),
            activity: Arc::downgrade(&activity),
        }))
        .unwrap();
    let library_revision = h.library.watch().revision();
    let settings_revision = h.settings.watch().revision();
    h.pasteboard.external_copy(vec![vec![(
        "public.utf8-plain-text".into(),
        b"user clipboard".to_vec(),
    )]]);
    let clipboard = h.pasteboard.snapshot.lock().unwrap().clone();
    h.engine.emit_trigger_lost(TriggerLost {
        snippet_id: id,
        trigger: ";PRIVATE-TRIGGER".into(),
    });
    let notice = expansion.lost_trigger_notice().get().as_ref().unwrap();
    assert_eq!(notice.snippet_id, id);
    assert_eq!(
        notice.message(),
        "The trigger was removed; the snippet couldn't be inserted."
    );
    assert_eq!(notice.action_label(), "Retype trigger");
    assert!(!format!("{notice:?}").contains("PRIVATE"));
    let entries = activity.watch().get();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].event.outcome, ActivityOutcome::TriggerLost);
    assert_eq!(entries[0].event.snippet_id, Some(id));
    assert_eq!(entries[0].event.trigger, None);
    assert_eq!(entries[0].event.source, None);
    assert_eq!(
        entries[0].explanation.sentence.as_deref(),
        Some(notice.message())
    );
    assert!(!format!("{entries:?}").contains("PRIVATE"));
    assert_eq!(h.library.watch().revision(), library_revision);
    assert_eq!(h.settings.watch().revision(), settings_revision);
    assert!(h.library.watch().get().usage.is_empty());
    assert!(*h.pasteboard.snapshot.lock().unwrap() == clipboard);
    let recovery = expansion.take_trigger_for_retyping(notice.id).unwrap();
    assert_eq!(recovery.trigger(), ";PRIVATE-TRIGGER");
    assert!(expansion.lost_trigger_notice().get().is_none());
    assert!(expansion.take_trigger_for_retyping(notice.id).is_none());
    assert!(h.engine.state.lock().unwrap().insertions.is_empty());
    assert!(*h.pasteboard.snapshot.lock().unwrap() == clipboard);
    h.engine.stop();

    let collector = Arc::new(FakeEngineEvents::default());
    h.engine.start(collector.clone()).unwrap();
    h.engine.emit_trigger_lost(TriggerLost {
        snippet_id: id,
        trigger: ";fixture".into(),
    });
    let lost = collector.lost_triggers.lock().unwrap();
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].snippet_id, id);
    assert_eq!(lost[0].trigger, ";fixture");
    h.engine.stop();
}
