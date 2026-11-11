//! After a submission: the form is no longer pending, but the engine is still working on it.
//! Late key-status reports still reach it, and typed triggers stay suppressed until its outcome.
use super::*;

/// S35 review (Grok), addendum 1. A key-status change that the form window reports after
/// submitting (the panel resigning key as it hides) still counts against a typed form, so it
/// must reach the engine; it used to be dropped because the form was no longer pending.
#[test]
fn a_focus_report_after_submitting_still_reaches_the_engine() {
    let snippet = SnippetRecord::new("Form", vec![], "{{field name=who}}");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let forms = forms(&h, FormMemory::default());
    let token = form_token(1);
    assert!(forms.requested(request(1, id)));
    forms.panel_focused(token, focus(true, 1));
    let submitted = finish(
        forms
            .submit(
                token,
                HashMap::new(),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(submitted.admission, Ok(()));
    forms.panel_focused(token, focus(false, 2));
    assert_eq!(
        h.engine.state.lock().unwrap().focus,
        [(token, focus(true, 1)), (token, focus(false, 2))]
    );
    // A token that was never requested, or one a newer form has replaced, is still dropped.
    forms.panel_focused(form_token(2), focus(true, 1));
    assert!(forms.requested(request(2, id)));
    forms.panel_focused(token, focus(true, 3));
    assert_eq!(h.engine.state.lock().unwrap().focus.len(), 2);
}

use crate::ports::{engine::AdmissionError, FormChannel, TriggerLost};
use kudzu_core::activity::{ActivityEvent, ActivityOutcome, ActivitySource, InsertFailure};
use std::time::{Duration, Instant};

/// Long enough for any hold to settle.
const SETTLED: Duration = Duration::from_secs(10);
/// Long enough for a wrong release to show. It can't make a correct hold fail.
const QUIET: Duration = Duration::from_millis(50);
/// Another snippet, with forms of its own.
const OTHER: Uuid = Uuid::from_u128(7);

fn form_harness() -> (Harness, Uuid) {
    let snippet = SnippetRecord::new("Form", vec![], "{{field name=who}}");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    (h, id)
}
/// Requests form `native` for `snippet` and submits it; the engine accepts it.
fn submit(forms: &FormService, native: u64, snippet: Uuid) -> Submission {
    assert!(forms.requested(request(native, snippet)));
    finish(
        forms
            .submit(
                form_token(native),
                HashMap::new(),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    )
}
fn report(source: ActivitySource, snippet: Uuid, outcome: ActivityOutcome) -> ActivityEvent {
    ActivityEvent::attempt(source, snippet, None, None, outcome)
}

/// Third review (Grok), major 3. Submitting hid the form and released its suppression at
/// once, but the engine only holds input once it starts deleting the form's trigger, after
/// activating the target and letting it settle. A trigger typed in between expanded on its
/// own. The submitted form now keeps triggers suppressed until its outcome arrives.
#[test]
fn a_trigger_typed_while_the_target_activates_does_not_expand() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    assert!(forms.requested(request(1, id)));
    assert!(
        !h.engine.type_trigger(),
        "a trigger expanded in the open form"
    );
    let submitted = finish(
        forms
            .submit(
                form_token(1),
                HashMap::new(),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(submitted.admission, Ok(()));
    // The engine has accepted the form and is bringing the target back to the front.
    assert!(
        !h.engine.type_trigger(),
        "a trigger typed during activation expanded"
    );
    // Reports that aren't this form's outcome leave it in place: a typed expansion of the
    // same snippet, another snippet's form, another snippet's lost trigger.
    h.engine
        .emit_activity(report(ActivitySource::Typed, id, ActivityOutcome::Expanded));
    h.engine.emit_activity(report(
        ActivitySource::Form,
        OTHER,
        ActivityOutcome::Expanded,
    ));
    h.engine.emit_trigger_lost(TriggerLost {
        snippet_id: OTHER,
        trigger: ";other".into(),
    });
    assert!(!h.engine.wait_unsuppressed(QUIET));
    assert!(!h.engine.type_trigger());
    h.engine.resolve_form(form_token(1), id);
    assert!(
        h.engine.wait_unsuppressed(SETTLED),
        "the form's outcome left triggers suppressed"
    );
    assert!(h.engine.type_trigger());
}

/// Any final report of a submitted form ends its hold: expanded, failed, interrupted, lost
/// (the attempt's own report, or D17's notice alone), blocked, or cancelled before its turn.
#[test]
fn every_final_report_of_the_submitted_form_releases_the_guard() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    let outcomes = [
        ActivityOutcome::Expanded,
        ActivityOutcome::InsertionFailed {
            error: InsertFailure::TargetGone,
        },
        ActivityOutcome::Interrupted,
        ActivityOutcome::TriggerLost,
        ActivityOutcome::Paused,
        ActivityOutcome::FormCancelled,
    ];
    for (n, outcome) in outcomes.into_iter().enumerate() {
        assert_eq!(submit(&forms, n as u64 + 1, id).admission, Ok(()));
        assert!(!h.engine.type_trigger(), "released before {outcome:?}");
        h.engine
            .emit_activity(report(ActivitySource::Form, id, outcome.clone()));
        assert!(
            h.engine.wait_unsuppressed(SETTLED),
            "{outcome:?} left triggers suppressed"
        );
    }
    assert_eq!(submit(&forms, 10, id).admission, Ok(()));
    assert!(!h.engine.type_trigger());
    h.engine.emit_trigger_lost(TriggerLost {
        snippet_id: id,
        trigger: ";form".into(),
    });
    assert!(h.engine.wait_unsuppressed(SETTLED));
}

/// Two submitted forms in flight: each waits for its own outcome.
#[test]
fn each_submitted_form_waits_for_its_own_outcome() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    assert_eq!(submit(&forms, 1, id).admission, Ok(()));
    assert_eq!(submit(&forms, 2, OTHER).admission, Ok(()));
    h.engine.resolve_form(form_token(2), OTHER);
    assert!(!h.engine.wait_unsuppressed(QUIET), "form 1 was released");
    h.engine.resolve_form(form_token(1), id);
    assert!(h.engine.wait_unsuppressed(SETTLED));
}

/// A refused admission means no outcome will come: the guard goes before the result does.
/// A token the engine no longer has (it restarted, or dropped the form) is refused as stale,
/// and the service cancels it like any other refusal.
#[test]
fn an_admission_error_releases_the_guard_at_once() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    h.engine.state.lock().unwrap().admission_error = Some(AdmissionError::Busy);
    assert_eq!(submit(&forms, 1, id).admission, Err(AdmissionError::Busy));
    assert!(h.engine.type_trigger());
    h.engine.state.lock().unwrap().admission_error = None;
    assert!(forms.requested(request(2, id)));
    h.engine.cancel_form(form_token(2));
    let refused = finish(
        forms
            .submit(
                form_token(2),
                HashMap::new(),
                crate::expansion::testing::submitted_at(100),
            )
            .unwrap()
            .unwrap(),
    );
    assert_eq!(refused.admission, Err(AdmissionError::StaleToken));
    assert!(h.engine.type_trigger());
    let attempts = h.engine.state.lock().unwrap().cancel_attempts.clone();
    assert_eq!(
        attempts.iter().filter(|t| **t == form_token(2)).count(),
        2,
        "the service didn't cancel the stale form"
    );
}

/// The engine never reports this form (it stopped, or the report was lost): the deadline
/// releases the guard, so triggers can't stay suppressed for good.
#[test]
fn a_form_whose_outcome_never_arrives_is_released_at_the_deadline() {
    let (h, id) = form_harness();
    let deadline = Duration::from_millis(400);
    let forms = forms(&h, FormMemory::default()).with_outcome_deadline(deadline);
    let started = Instant::now();
    assert_eq!(submit(&forms, 1, id).admission, Ok(()));
    assert!(!h.engine.type_trigger());
    assert!(h.engine.wait_unsuppressed(SETTLED));
    assert!(started.elapsed() >= deadline);
    assert!(forms.outcomes().get().is_none());
}

/// Dropping the service releases a submitted form's guard with it.
#[test]
fn dropping_the_form_service_releases_a_submitted_forms_guard() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    assert_eq!(submit(&forms, 1, id).admission, Ok(()));
    assert!(!h.engine.type_trigger());
    drop(forms);
    assert!(h.engine.type_trigger());
}

/// S34 follow-up. When the adapter stops, the engine reports it stopped and nothing in flight
/// will report after that: the hold goes then, not at the deadline.
#[test]
fn an_engine_stop_releases_a_submitted_forms_guard_at_once() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    assert_eq!(submit(&forms, 1, id).admission, Ok(()));
    assert!(!h.engine.type_trigger());
    h.engine
        .emit_activity(ActivityEvent::health(ActivityOutcome::EngineStopped));
    assert!(
        h.engine.wait_unsuppressed(Duration::from_secs(2)),
        "the stopped engine's form held triggers until the deadline"
    );
}

/// S34 follow-up. A newer adapter retires this one (its own stop report is dropped as
/// retired); the composition resyncs the services after starting it, and the hold goes then.
#[test]
fn a_newer_engine_releases_a_submitted_forms_guard_at_once() {
    let (h, id) = form_harness();
    let forms = forms(&h, FormMemory::default());
    assert_eq!(submit(&forms, 1, id).admission, Ok(()));
    h.engine.restart_instance();
    h.wire_outcomes(&forms.expansion);
    assert!(
        h.engine.wait_unsuppressed(Duration::from_secs(2)),
        "the retired engine's form held triggers until the deadline"
    );
}
