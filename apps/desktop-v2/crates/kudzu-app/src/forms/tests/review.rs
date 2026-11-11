use super::*;
use crate::{
    expansion::testing::until, fixtures::ids::FixedIds, library::LibraryService,
    persist::LoadSource, schedule::Scheduler,
};

#[test]
fn recovered_library_preserves_form_memory_until_a_later_observed_deletion() {
    let snippet = SnippetRecord::new("Recovered", vec![], "{{field name=who}}");
    let id = snippet.id;
    let folder = std::env::temp_dir().join(format!("kudzu-s27-recovered-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("library.json");
    std::fs::write(&path, b"corrupt fixture").unwrap();
    let mut h = Harness::new(KudzuLibrary::default());
    let (library, source) = LibraryService::open(
        path,
        "device".into(),
        h.clock.clone(),
        Arc::new(FixedIds::new(1000)),
        &h.writer,
        Arc::new(Scheduler::new(h.clock.clone()).unwrap()),
    )
    .unwrap();
    assert!(matches!(source, LoadSource::Recovered(_)));
    h.library = Arc::new(library);
    assert!(h.library.watch().get().snippets.is_empty());
    let memory = FormMemory {
        values: HashMap::from([(id, HashMap::from([("who".into(), "Remembered".into())]))]),
    };
    let forms = forms(&h, memory);
    until(&forms.pruned(), |p| {
        p.library == h.library.watch().revision()
    });
    h.library
        .transact(|lib| {
            lib.snippets.push(snippet.clone());
            Ok(())
        })
        .unwrap();
    until(&forms.pruned(), |p| {
        p.library == h.library.watch().revision()
    });
    forms.requested(request(1, id));
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
    let (deleted, revision) = h.library.delete_snippets(&[id]).unwrap();
    until(&forms.pruned(), |p| p.library == revision);
    h.library.restore(deleted).unwrap();
    forms.requested(request(2, id));
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
    forms.stop_pruning();
    h.writer.shutdown().unwrap();
    std::fs::remove_dir_all(folder).unwrap();
}

#[test]
fn closing_windows_cancels_without_activation_but_explicit_cancel_restores_target() {
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    forms.requested(request(1, id));
    finish(forms.close_windows().unwrap().unwrap()).unwrap();
    assert!(h.apps.activations.lock().unwrap().is_empty());
    assert!(!h.engine.state.lock().unwrap().suppressed);
    forms.requested(request(2, id));
    finish(forms.cancel(form_token(2)).unwrap().unwrap()).unwrap();
    assert_eq!(h.apps.activations.lock().unwrap().len(), 1);
}

#[test]
fn rejected_requests_cancel_the_engine_form_and_publish_a_content_free_signal() {
    use crate::expansion::InsertionOutcome;
    let snippet = SnippetRecord::new("Form", vec![], "private body");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    assert!(forms.requested(request(1, id)));
    assert!(!forms.requested(request(1, id)));
    assert_eq!(h.engine.state.lock().unwrap().cancelled, [form_token(1)]);
    assert!(forms.pending().get().is_none());
    assert!(!h.engine.state.lock().unwrap().suppressed);
    let notice = forms.outcomes().get().as_ref().unwrap();
    assert_eq!(
        notice.outcome,
        InsertionOutcome::FormRejected {
            token: form_token(1)
        }
    );
    assert!(!format!("{notice:?}").contains("private"));
    assert!(h.apps.activations.lock().unwrap().is_empty());
}

#[test]
fn restarted_engine_token_one_is_distinct_and_old_tokens_cannot_cancel_it() {
    use crate::ports::{engine::AdmissionError, ExpansionEngine, FormChannel};
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let expansion = h.expansion();
    h.wire_outcomes(&expansion);
    let forms = FormService::new(
        FormMemory::default(),
        h.writer.register(Memory::default(), Revision(0)).unwrap(),
        h.library.watch(),
        expansion.clone(),
        h.engine.clone(),
        h.apps.clone(),
    )
    .unwrap();
    let old = h.engine.form_token(1);
    let mut old_request = request(1, id);
    old_request.token = old;
    assert!(forms.requested(old_request.clone()));
    h.engine.stop();
    h.engine.restart_instance();
    h.wire_outcomes(&expansion);
    assert!(h.engine.state.lock().unwrap().suppressed);
    let new = h.engine.form_token(1);
    assert_ne!(old, new);
    assert_eq!(old.instance().native_token(old), Some(1));
    assert_eq!(new.instance().native_token(new), Some(1));
    assert_eq!(new.instance().native_token(old), None);
    let mut new_request = request(1, id);
    new_request.token = new;
    assert!(forms.requested(new_request));
    assert!(!forms.requested(old_request));
    assert_eq!(forms.pending().get().as_ref().as_ref().unwrap().token, new);
    assert!(forms
        .submit(
            old,
            HashMap::new(),
            crate::ports::EventStamp::from_cg_event_timestamp(100)
        )
        .unwrap()
        .is_none());
    assert_eq!(
        h.engine.submit_form(
            old,
            HashMap::new(),
            crate::ports::EventStamp::from_cg_event_timestamp(100)
        ),
        Err(AdmissionError::StaleToken)
    );
    assert!(!h.engine.state.lock().unwrap().cancelled.contains(&new));
    assert_eq!(
        finish(
            forms
                .submit(
                    new,
                    HashMap::new(),
                    crate::ports::EventStamp::from_cg_event_timestamp(100)
                )
                .unwrap()
                .unwrap()
        )
        .admission,
        Ok(())
    );
}

#[test]
fn submission_and_focus_forward_the_original_cg_event_boundaries() {
    use crate::ports::EventStamp;
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    let token = form_token(1);
    forms.requested(request(1, id));
    let after = EventStamp::from_cg_event_timestamp(9_007_199_254_740_993);
    let at = EventStamp::from_cg_event_timestamp(u64::MAX - 17);
    forms.panel_focused(token, true, Some(after));
    forms.panel_focused(token, false, None);
    assert_eq!(
        finish(forms.submit(token, HashMap::new(), at).unwrap().unwrap()).admission,
        Ok(())
    );
    let engine = h.engine.state.lock().unwrap();
    assert_eq!(
        engine.focus,
        [(token, true, Some(after)), (token, false, None)]
    );
    assert_eq!(engine.submissions[0].2, at);
    assert_eq!(at.cg_event_timestamp(), u64::MAX - 17);
}

#[test]
fn submit_start_failure_uses_cancel_target_restoration_and_reports_both_results() {
    use crate::{
        error::IoFailure,
        expansion::InsertionOutcome,
        tasks::{Task, TaskError},
    };
    struct RefuseWorker;
    impl SubmissionWorker for RefuseWorker {
        fn spawn(
            &self,
            _: Box<dyn FnOnce() -> Submission + Send>,
        ) -> Result<Task<Submission>, TaskError> {
            Err(TaskError::Start(IoFailure {
                kind: std::io::ErrorKind::WouldBlock,
            }))
        }
    }
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default()).with_submission_worker(Arc::new(RefuseWorker));
    forms.requested(request(1, id));
    let failure = match forms.submit(
        form_token(1),
        HashMap::from([("who".into(), "Kept".into())]),
        crate::ports::EventStamp::from_cg_event_timestamp(123),
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("submission unexpectedly started"),
    };
    assert_eq!(
        failure.start,
        TaskError::Start(IoFailure {
            kind: std::io::ErrorKind::WouldBlock
        })
    );
    finish(failure.restoration.unwrap()).unwrap();
    assert_eq!(
        h.apps.activations.lock().unwrap()[0].bundle_id,
        "com.example.target"
    );
    assert!(forms.pending().get().is_none());
    assert!(!h.engine.state.lock().unwrap().suppressed);
    assert!(h.engine.state.lock().unwrap().submissions.is_empty());
    assert_eq!(
        forms.outcomes().get().as_ref().unwrap().outcome,
        InsertionOutcome::SubmitStartFailed {
            token: form_token(1)
        }
    );
    forms.requested(request(2, id));
    assert_eq!(
        forms
            .pending()
            .get()
            .as_ref()
            .as_ref()
            .unwrap()
            .initial_values["who"],
        "Kept"
    );
}

#[test]
fn an_admission_rejection_cancels_the_consumed_engine_form_without_claiming_completion() {
    use crate::ports::{engine::AdmissionError, EventStamp};
    let snippet = SnippetRecord::new("Form", vec![], "");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    forms.requested(request(1, id));
    h.engine.state.lock().unwrap().admission_error = Some(AdmissionError::Busy);
    let submission = finish(
        forms
            .submit(
                form_token(1),
                HashMap::new(),
                EventStamp::from_cg_event_timestamp(11),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(submission.admission, Err(AdmissionError::Busy));
    let cancelled = h.engine.state.lock().unwrap().cancelled.clone();
    assert_eq!(cancelled, [form_token(1)]);
    assert!(forms.pending().get().is_none());
    assert!(forms.outcomes().get().is_none());
}
