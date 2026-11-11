use super::*;
use crate::{
    expansion::testing::{finish, focus, form_token, Harness, Memory},
    persist::Revision,
    ports::FormRequest as EngineRequest,
};
use kudzu_core::{
    library::model::{KudzuLibrary, SnippetRecord},
    system::AppInfo,
    template::{FormFieldKind, FormFieldSpec, FormMemory},
};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

fn request(token: u64, snippet: Uuid) -> EngineRequest {
    EngineRequest {
        token: form_token(token),
        snippet_id: snippet,
        snippet_name: "Form".into(),
        fields: vec![FormFieldSpec {
            name: "who".into(),
            kind: FormFieldKind::Text,
            options: vec![],
            default_value: Some("Guest".into()),
        }],
        target: Some(AppInfo {
            bundle_id: "com.example.target".into(),
            name: "Target".into(),
            pid: Some(42),
            path: None,
        }),
        caret: None,
        selection: Some("selection".into()),
    }
}
fn forms(h: &Harness, initial: FormMemory) -> FormService {
    let expansion = h.expansion();
    h.wire_outcomes(&expansion);
    FormService::new(
        initial,
        h.writer.register(Memory::default(), Revision(0)).unwrap(),
        h.library.watch(),
        expansion,
        h.engine.clone(),
        h.apps.clone(),
    )
    .unwrap()
}
#[test]
fn tokens_complete_once_replacement_and_close_release_only_their_guard() {
    let snippet = SnippetRecord::new("Form", vec![], "{{field name=who}}");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    assert!(forms.requested(request(1, id)));
    assert_eq!(
        forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .unwrap()
            .initial_values["who"],
        "Guest"
    );
    assert!(h.engine.state.lock().unwrap().suppressed);
    assert!(forms
        .submit(
            form_token(0),
            HashMap::new(),
            crate::expansion::testing::submitted_at(100)
        )
        .unwrap()
        .is_none());
    assert!(forms.cancel(form_token(0)).unwrap().is_none());
    forms.panel_focused(form_token(1), focus(true, 1));
    forms.panel_focused(form_token(0), focus(false, 2));
    assert_eq!(
        h.engine.state.lock().unwrap().focus,
        [(form_token(1), focus(true, 1))]
    );
    let result = finish(
        forms
            .submit(
                form_token(1),
                HashMap::from([("who".into(), "Ada".into())]),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(result.admission, Ok(()));
    assert!(forms.pending().get().is_none());
    assert!(!h.engine.state.lock().unwrap().suppressed);
    assert!(forms
        .submit(
            form_token(1),
            HashMap::new(),
            crate::expansion::testing::submitted_at(100)
        )
        .unwrap()
        .is_none());
    assert!(
        !forms.requested(request(1, id)),
        "completed requests cannot be replayed"
    );
    assert!(forms.requested(request(2, id)));
    assert_eq!(
        forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .unwrap()
            .initial_values["who"],
        "Ada"
    );
    assert!(forms.requested(request(3, id)));
    assert!(h.engine.state.lock().unwrap().suppressed);
    assert_eq!(
        h.engine.state.lock().unwrap().cancelled,
        [form_token(1), form_token(2)]
    );
    finish(forms.close_windows().unwrap().unwrap()).unwrap();
    assert!(forms.pending().get().is_none());
    assert!(!h.engine.state.lock().unwrap().suppressed);
    assert!(h.apps.activations.lock().unwrap().is_empty());
    assert_eq!(
        h.engine.state.lock().unwrap().cancelled,
        [form_token(1), form_token(2), form_token(3)]
    );
    assert_eq!(h.engine.state.lock().unwrap().submissions.len(), 1);
}

#[test]
fn memory_uses_valid_fallbacks_caps_smallest_uuid_and_prunes_deleted_snippets() {
    use crate::expansion::testing::until;
    let snippets: Vec<_> = (1..=201)
        .map(|id| {
            let mut s = SnippetRecord::new("Form", vec![], "");
            s.id = Uuid::from_u128(id);
            s
        })
        .collect();
    let h = Harness::new(KudzuLibrary {
        snippets,
        ..Default::default()
    });
    let initial = FormMemory {
        values: (1..=200)
            .map(|id| {
                (
                    Uuid::from_u128(id),
                    HashMap::from([("who".into(), "Remembered".into())]),
                )
            })
            .collect(),
    };
    let forms = forms(&h, initial);
    let mut req = request(1, Uuid::from_u128(2));
    req.fields.extend([
        FormFieldSpec {
            name: "default".into(),
            kind: FormFieldKind::Choice,
            options: vec!["a".into(), "b".into()],
            default_value: Some("b".into()),
        },
        FormFieldSpec {
            name: "first".into(),
            kind: FormFieldKind::Choice,
            options: vec!["a".into(), "b".into()],
            default_value: Some("invalid".into()),
        },
        FormFieldSpec {
            name: "empty".into(),
            kind: FormFieldKind::Text,
            options: vec![],
            default_value: None,
        },
    ]);
    forms.requested(req);
    let pending = forms.pending().get();
    assert_eq!(
        pending.as_ref().as_ref().unwrap().initial_values,
        HashMap::from([
            ("who".into(), "Remembered".into()),
            ("default".into(), "b".into()),
            ("first".into(), "a".into()),
            ("empty".into(), "".into())
        ])
    );
    forms.requested(request(2, Uuid::from_u128(201)));
    finish(
        forms
            .submit(
                form_token(2),
                HashMap::from([("who".into(), "Newest".into())]),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    forms.requested(request(3, Uuid::from_u128(1)));
    assert_eq!(
        forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .unwrap()
            .initial_values["who"],
        "Guest"
    );
    forms.requested(request(4, Uuid::from_u128(2)));
    assert_eq!(
        forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .unwrap()
            .initial_values["who"],
        "Remembered"
    );
    let (deleted, revision) = h.library.delete_snippets(&[Uuid::from_u128(2)]).unwrap();
    until(&forms.pruned(), |state| state.library >= revision);
    h.library.restore(deleted).unwrap();
    forms.requested(request(5, Uuid::from_u128(2)));
    assert_eq!(
        forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .unwrap()
            .initial_values["who"],
        "Guest"
    );
}

#[test]
fn interrupted_submission_reports_an_outcome_after_admission_while_suppressed() {
    use crate::expansion::InsertionOutcome;
    let snippet = SnippetRecord::new("Form", vec![], "{{field name=who}} {{selection}}");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    forms.requested(request(1, id));
    forms.panel_focused(form_token(1), focus(true, 1));
    h.engine.input_outside_form(form_token(1));
    forms.panel_focused(form_token(1), focus(false, 2));
    assert!(h.engine.state.lock().unwrap().suppressed);
    h.engine.input_outside_form(form_token(1));
    let preview = finish(
        forms
            .preview(form_token(1), HashMap::from([("who".into(), "Ada".into())]))
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(preview.text, "Ada selection");
    let result = finish(
        forms
            .submit(
                form_token(1),
                HashMap::new(),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(result.admission, Ok(()));
    assert!(forms.outcomes().get().is_none());
    h.engine.resolve_form(form_token(1), id);
    assert_eq!(
        forms.outcomes().get().as_ref().unwrap().outcome,
        InsertionOutcome::Interrupted
    );
    assert_eq!(h.engine.state.lock().unwrap().submissions.len(), 1);
    assert!(!h.engine.state.lock().unwrap().suppressed);
    assert!(forms
        .preview(form_token(1), HashMap::new())
        .unwrap()
        .is_none());
}

#[test]
fn memory_is_accepted_before_submission_and_a_disk_failure_does_not_block_insert() {
    let snippet = SnippetRecord::new("Form", vec![], "{{field name=who}}");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let sink = Memory::default();
    *sink.failure.lock().unwrap() = Some(PersistError::Encode);
    let forms = Arc::new(
        FormService::new(
            FormMemory::default(),
            h.writer.register(sink, Revision(0)).unwrap(),
            h.library.watch(),
            h.expansion(),
            h.engine.clone(),
            h.apps.clone(),
        )
        .unwrap(),
    );
    let weak = Arc::downgrade(&forms);
    *h.engine.before_submit.lock().unwrap() = Some(Arc::new(move || {
        let forms = weak.upgrade().unwrap();
        forms.requested(request(2, id));
        assert_eq!(
            forms
                .pending()
                .get()
                .as_ref()
                .as_ref()
                .unwrap()
                .initial_values["who"],
            "Remember me"
        );
    }));
    forms.requested(request(1, id));
    let submitted = finish(
        forms
            .submit(
                form_token(1),
                HashMap::from([("who".into(), "Remember me".into())]),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(submitted.admission, Ok(()));
    assert_eq!(submitted.memory_error, None);
    assert!(submitted.memory_revision > Revision(1));
    forms.stop_pruning();
    assert_eq!(h.writer.shutdown(), Err(PersistError::Encode));
    assert_eq!(forms.saved().get().error, Some(PersistError::Encode));
    assert!(forms.saved().get().durable < submitted.memory_revision);
    assert_eq!(h.engine.state.lock().unwrap().submissions.len(), 1);
}

#[test]
fn submitted_memory_reopens_from_the_local_json_file() {
    let snippet = SnippetRecord::new("Form", vec![], "{{field name=who}}");
    let id = snippet.id;
    let initial = KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    };
    let folder = std::env::temp_dir().join(format!("kudzu-s27-form-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("form-memory.json");
    {
        let h = Harness::new(initial.clone());
        let forms = FormService::open(
            path.clone(),
            &h.writer,
            h.library.watch(),
            h.expansion(),
            h.engine.clone(),
            h.apps.clone(),
        )
        .unwrap();
        forms.requested(request(1, id));
        finish(
            forms
                .submit(
                    form_token(1),
                    HashMap::from([("who".into(), "Saved locally".into())]),
                    crate::expansion::testing::submitted_at(100),
                )
                .unwrap()
                .unwrap(),
        );
        forms.stop_pruning();
        h.writer.shutdown().unwrap();
    }
    {
        let h = Harness::new(initial);
        let forms = FormService::open(
            path,
            &h.writer,
            h.library.watch(),
            h.expansion(),
            h.engine.clone(),
            h.apps.clone(),
        )
        .unwrap();
        forms.requested(request(1, id));
        assert_eq!(
            forms
                .pending()
                .get()
                .as_ref()
                .as_ref()
                .unwrap()
                .initial_values["who"],
            "Saved locally"
        );
        forms.stop_pruning();
        h.writer.shutdown().unwrap();
    }
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn concurrent_submit_and_cancel_consume_one_token_and_preserve_another_suppression_owner() {
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let expansion = h.expansion();
    let guard = expansion.suppress();
    let forms = Arc::new(
        FormService::new(
            FormMemory::default(),
            h.writer.register(Memory::default(), Revision(0)).unwrap(),
            h.library.watch(),
            expansion,
            h.engine.clone(),
            h.apps.clone(),
        )
        .unwrap(),
    );
    forms.requested(request(1, id));
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let results: Vec<_> = (0..8)
        .map(|i| {
            let forms = forms.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                if i % 2 == 0 {
                    forms
                        .submit(
                            form_token(1),
                            HashMap::new(),
                            crate::expansion::testing::submitted_at(100),
                        )
                        .unwrap()
                        .map(|task| {
                            finish(task);
                        })
                } else {
                    forms.cancel(form_token(1)).unwrap().map(|task| {
                        finish(task).unwrap();
                    })
                }
            })
        })
        .collect();
    assert_eq!(
        results
            .into_iter()
            .map(|thread| thread.join().unwrap().is_some() as usize)
            .sum::<usize>(),
        1
    );
    let engine = h.engine.state.lock().unwrap();
    assert_eq!(engine.submissions.len() + engine.cancelled.len(), 1);
    assert!(engine.suppressed);
    drop(engine);
    drop(guard);
    assert!(!h.engine.state.lock().unwrap().suppressed);
}

#[test]
fn dropping_the_form_service_cancels_and_clears_a_retained_watch() {
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    let pending = forms.pending();
    forms.requested(request(1, id));
    drop(forms);
    assert!(pending.get().is_none());
    assert_eq!(h.engine.state.lock().unwrap().cancelled, [form_token(1)]);
    assert!(!h.engine.state.lock().unwrap().suppressed);
}

mod review;
