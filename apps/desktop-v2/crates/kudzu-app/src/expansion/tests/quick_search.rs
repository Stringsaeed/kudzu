//! A Quick Search pick keeps typed triggers suppressed until the engine reports it, like a
//! submitted form (S34 follow-up): the engine activates the target and lets it settle before
//! it holds input, and a trigger typed then mustn't expand.
use super::*;
use crate::{
    expansion::testing::{finish, Harness, Memory},
    forms::FormService,
    persist::Revision,
    ports::{engine::AdmissionError, FormRequest},
};
use kudzu_core::{
    activity::{ActivityEvent, ActivityOutcome, ActivitySource},
    system::AppInfo,
    template::{FormFieldKind, FormFieldSpec, FormMemory},
};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Long enough for any hold to settle.
const SETTLED: Duration = Duration::from_secs(10);
/// Well short of the default deadline: a hold released this soon wasn't waiting for it.
const SOON: Duration = Duration::from_secs(2);
/// Long enough for a wrong release to show. It can't make a correct hold fail.
const QUIET: Duration = Duration::from_millis(50);

fn target() -> AppInfo {
    AppInfo {
        bundle_id: "com.example.target".into(),
        name: "Target".into(),
        pid: Some(42),
        path: None,
    }
}
fn picker_harness(content: &str) -> (Harness, Uuid) {
    let snippet = SnippetRecord::new("Picked", vec![], content);
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    *h.apps.frontmost.lock().unwrap() = Some(target());
    (h, id)
}
/// Opens Quick Search and picks `snippet`; returns the admission.
fn pick(service: &ExpansionService, snippet: Uuid) -> Result<(), AdmissionError> {
    let session = finish(service.begin_quick_search().unwrap()).unwrap();
    finish(session.insert(snippet).unwrap())
}

/// The S34 follow-up's red test. Picking a snippet hid the palette and released its
/// suppression at once, while the engine was still bringing the target back.
#[test]
fn a_trigger_typed_while_the_target_activates_after_a_quick_search_pick_does_not_expand() {
    let (h, id) = picker_harness("picked text");
    let service = h.expansion();
    h.wire_outcomes(&service);
    let session = finish(service.begin_quick_search().unwrap()).unwrap();
    assert!(
        !h.engine.type_trigger(),
        "a trigger expanded in the palette"
    );
    assert_eq!(finish(session.insert(id).unwrap()), Ok(()));
    // The engine has accepted the pick and is bringing the target back to the front.
    assert!(
        !h.engine.type_trigger(),
        "a trigger typed during activation expanded"
    );
    // Reports that aren't this pick's outcome leave it in place: a typed expansion of the same
    // snippet, another snippet's pick.
    h.engine.emit_activity(ActivityEvent::attempt(
        ActivitySource::Typed,
        id,
        None,
        None,
        ActivityOutcome::Expanded,
    ));
    h.engine.emit_activity(ActivityEvent::attempt(
        ActivitySource::QuickSearch,
        Uuid::from_u128(7),
        None,
        None,
        ActivityOutcome::Expanded,
    ));
    assert!(!h.engine.wait_unsuppressed(QUIET));
    h.engine.resolve_insert(0);
    assert!(
        h.engine.wait_unsuppressed(SETTLED),
        "the pick's outcome left triggers suppressed"
    );
    assert!(h.engine.type_trigger());
}

/// A refused pick will never report: the guard goes before the admission result does.
#[test]
fn a_refused_quick_search_pick_releases_the_guard_at_once() {
    let (h, id) = picker_harness("picked text");
    let service = h.expansion();
    h.wire_outcomes(&service);
    h.engine.state.lock().unwrap().admission_error = Some(AdmissionError::Busy);
    assert_eq!(pick(&service, id), Err(AdmissionError::Busy));
    assert!(h.engine.type_trigger());
}

/// A pick whose snippet has fields opens a form instead of reporting. The form takes its own
/// guard, and the pick's goes as the forms service receives the request.
#[test]
fn a_quick_search_pick_that_opens_a_form_hands_its_guard_to_the_form() {
    let (h, id) = picker_harness("{{field name=who}}");
    let service = h.expansion();
    h.wire_outcomes(&service);
    let forms = FormService::new(
        FormMemory::default(),
        h.writer.register(Memory::default(), Revision(0)).unwrap(),
        h.library.watch(),
        service.clone(),
        h.engine.clone(),
        h.apps.clone(),
    )
    .unwrap();
    assert_eq!(pick(&service, id), Ok(()));
    let token = crate::expansion::testing::form_token(1);
    assert!(forms.requested(FormRequest {
        token,
        snippet_id: id,
        snippet_name: "Picked".into(),
        fields: vec![FormFieldSpec {
            name: "who".into(),
            kind: FormFieldKind::Text,
            options: vec![],
            default_value: None,
        }],
        target: Some(target()),
        caret: None,
        selection: None,
    }));
    assert!(!h.engine.type_trigger(), "the form isn't suppressing");
    finish(forms.cancel(token).unwrap().unwrap()).unwrap();
    assert!(
        h.engine.wait_unsuppressed(SOON),
        "the pick kept triggers suppressed after its form closed"
    );
}

/// The engine never reports the pick: the deadline releases the guard.
#[test]
fn a_quick_search_pick_whose_outcome_never_arrives_is_released_at_the_deadline() {
    let (h, id) = picker_harness("picked text");
    let deadline = Duration::from_millis(400);
    let service = Arc::new(h.expansion_service().with_outcome_deadline(deadline));
    h.wire_outcomes(&service);
    let started = Instant::now();
    assert_eq!(pick(&service, id), Ok(()));
    assert!(!h.engine.type_trigger());
    assert!(h.engine.wait_unsuppressed(SETTLED));
    assert!(started.elapsed() >= deadline);
    assert!(service.outcomes().get().is_none());
}
