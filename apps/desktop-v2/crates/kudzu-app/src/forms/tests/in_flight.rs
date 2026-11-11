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
