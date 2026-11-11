use super::*;
use crate::ports::ExpansionEngine;
use crate::{expansion::testing::finish, ports::engine::AdmissionError};
use kudzu_core::{
    activity::{ActivityEvent, ActivityOutcome, ActivitySource, InsertFailure},
    system::AppInfo,
};

#[test]
fn admission_is_separate_from_later_content_free_engine_outcomes() {
    let snippet = SnippetRecord::new("Fixture", vec![";private".into()], "private output");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let service = h.expansion();
    h.wire_outcomes(&service);
    let target = AppInfo {
        bundle_id: "example.app".into(),
        name: "Target".into(),
        pid: Some(42),
        path: None,
    };
    let admitted = finish(service.insert(id, Some(target), None).unwrap());
    assert_eq!(admitted, Ok(()));
    assert!(service.outcomes().get().is_none());
    h.engine.emit_activity(ActivityEvent::attempt(
        ActivitySource::QuickSearch,
        id,
        Some("private input".into()),
        None,
        ActivityOutcome::InsertionFailed {
            error: InsertFailure::PasteUnavailable,
        },
    ));
    let failed = service.outcomes().get().as_ref().unwrap();
    assert_eq!(
        failed.outcome,
        InsertionOutcome::Failed(InsertFailure::PasteUnavailable)
    );
    assert!(!format!("{failed:?}").contains("private"));
    h.engine.emit_activity(ActivityEvent::attempt(
        ActivitySource::QuickSearch,
        id,
        None,
        None,
        ActivityOutcome::Interrupted,
    ));
    let interrupted = service.outcomes().get().as_ref().unwrap();
    assert_eq!(interrupted.outcome, InsertionOutcome::Interrupted);
    assert!(interrupted.sequence > failed.sequence);
    assert!(h.library.watch().get().usage.is_empty());
    h.engine.emit_activity(ActivityEvent::attempt(
        ActivitySource::QuickSearch,
        id,
        None,
        None,
        ActivityOutcome::Expanded,
    ));
    assert_eq!(
        service.outcomes().get().as_ref().unwrap().outcome,
        InsertionOutcome::Expanded
    );
    let revision = service.outcomes().revision();
    h.engine.state.lock().unwrap().admission_error = Some(AdmissionError::Busy);
    assert_eq!(
        finish(service.insert(id, None, None).unwrap()),
        Err(AdmissionError::Busy)
    );
    assert_eq!(service.outcomes().revision(), revision);
    h.engine.stop();
    assert_eq!(
        finish(service.insert(id, None, None).unwrap()),
        Err(AdmissionError::NoEngine)
    );
}
