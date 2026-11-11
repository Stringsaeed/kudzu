//! FRM-4, typed forms. A trigger whose snippet has fields stays in the document (the withheld
//! key is typed back) while the form is open; submitting deletes it and inserts the result.
//! Anything that may have changed the document in between makes the submission delete
//! nothing: typing while the form is prepared, and (D9) any key-down or click that reaches the
//! target app while the form panel is not key. A token acts once; a stale one does nothing.
//!
//! **Deliberately different from the copied engine (D9):** it skipped the interruption check
//! while suppressed, which is the whole time a form is open, and never counted clicks, so
//! typing or clicking in the target app and then submitting deleted text at the wrong place.
//! The tests marked D9 pin the fix; the rest pin the copied behaviour.

use kudzu_core::activity::{ActivityOutcome, ActivitySource};

use super::desk::{editor, At, Ev, Key, Out};
use super::rig::{backspaces, ms, snippet, Rig};

fn form_snippet() -> kudzu_core::SnippetRecord {
    snippet(";addr", "Dear {{field name=who}}, hi")
}

/// The trigger is typed and the form requested, then the `forms` service holds its
/// suppression guard (as it will in the app) until submit or cancel.
fn opened() -> (Rig, u64) {
    let rig = Rig::new(&[form_snippet()]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.clear_log();
    (rig, token)
}

fn inserted() -> Vec<Ev> {
    [
        vec![Ev::Activated(editor().bundle_id), Ev::Slept(ms(50))],
        backspaces(5).into_iter().map(Ev::Out).collect(),
        vec![Ev::Out(Out::Type("Dear Sam, hi".into()))],
    ]
    .concat()
}

fn form_outcomes(rig: &Rig) -> Vec<ActivityOutcome> {
    rig.host
        .activity
        .lock()
        .unwrap()
        .iter()
        .filter(|e| e.source == Some(ActivitySource::Form))
        .map(|e| e.outcome.clone())
        .collect()
}

#[test]
fn a_typed_form_keeps_the_trigger_until_submitted() {
    let rig = Rig::new(&[form_snippet()]);
    rig.type_(";addr");
    // The swallowed "r" is typed back, then the form is asked for; nothing is deleted.
    assert_eq!(rig.posted(), [Out::Type("r".into())]);
    assert_eq!(rig.doc(), ";addr|");
    let forms = rig.host.forms.lock().unwrap().clone();
    assert_eq!(forms.len(), 1);
    assert_eq!(forms[0].target_app, Some(editor()));
    assert_eq!(
        forms[0]
            .fields
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["who"]
    );
    assert!(forms[0].caret.is_some());
}

#[test]
fn submitting_replaces_the_trigger() {
    let (rig, token) = opened();
    rig.engine.form_panel_focused(token, true);
    rig.type_in_panel("Sam⏎");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
    assert_eq!(rig.doc(), "Dear Sam, hi|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Expanded]);
}

/// D9. The user clicks into the target (the panel was still key for that click), the panel
/// loses focus, they type. The trigger is no longer where the engine thinks: nothing is
/// deleted. (The copied engine would delete five characters before the caret here.)
#[test]
fn a_key_after_the_panel_lost_focus_prevents_the_submission() {
    let (rig, token) = opened();
    rig.engine.form_panel_focused(token, true);
    rig.clicks_text(0);
    rig.engine.form_panel_focused(token, false);
    rig.type_("z");
    rig.engine.form_panel_focused(token, true);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), "z|;addr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
    assert_eq!(rig.host.expansions(), 0);
}

/// D9. A click in the target after the panel lost focus may have moved the caret.
#[test]
fn a_click_after_the_panel_lost_focus_prevents_the_submission() {
    let (rig, token) = opened();
    rig.engine.form_panel_focused(token, true);
    rig.engine.form_panel_focused(token, false);
    rig.clicks_text(2);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// D9. A form starts out not key: typing that reaches the target before the panel takes
/// focus interrupts it too.
#[test]
fn typing_before_the_panel_is_key_prevents_the_submission() {
    let (rig, token) = opened();
    rig.type_("q");
    rig.engine.form_panel_focused(token, true);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addrq|");
}

/// Losing focus is not an interruption by itself; only input that reaches the target is.
#[test]
fn losing_focus_alone_does_not_prevent_the_submission() {
    let (rig, token) = opened();
    rig.engine.form_panel_focused(token, true);
    rig.engine.form_panel_focused(token, false);
    rig.engine.form_panel_focused(token, true);
    rig.clicks();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
    assert_eq!(rig.doc(), "Dear Sam, hi|");
}

#[test]
fn a_stale_token_does_nothing_and_a_token_acts_once() {
    let (rig, token) = opened();
    rig.engine.form_panel_focused(token, true);
    rig.submit(token + 1, &[("who", "Eve")]);
    rig.engine.cancel_form(token + 1);
    rig.settle();
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addr|");
    // The real token still works, once.
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
    rig.clear_log();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), "Dear Sam, hi|");
}

/// D9. A focus report for another token says nothing about this form's panel.
#[test]
fn a_focus_report_for_another_token_is_ignored() {
    let (rig, token) = opened();
    rig.engine.form_panel_focused(token + 1, true);
    rig.type_("q");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addrq|");
}

#[test]
fn a_cancelled_form_leaves_the_trigger_intact() {
    let (rig, token) = opened();
    rig.engine.cancel_form(token);
    rig.settle();
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::FormCancelled]);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addr|");
}

/// Typing that reaches the document while the form is being prepared: the form is never shown.
#[test]
fn typing_while_the_form_is_prepared_skips_it() {
    let rig = Rig::new(&[form_snippet()]);
    rig.at(At::Sleep(ms(50), 0), |d| d.types("q"));
    rig.type_(";addr");
    assert_eq!(rig.posted(), [Out::Type("r".into())]);
    assert_eq!(rig.doc(), ";addrq|");
    assert!(rig.host.forms.lock().unwrap().is_empty());
}

/// Keys held while the trigger's key is typed back are let go, and they skip the form too.
#[test]
fn held_keys_at_the_trigger_are_released_and_skip_the_form() {
    let rig = Rig::new(&[form_snippet()]);
    rig.at(At::Press(0), |d| d.types("q"));
    let release = |down| {
        Out::Release(Key {
            label: "q".into(),
            down,
        })
    };
    rig.type_(";addr");
    assert_eq!(
        rig.posted(),
        [Out::Type("r".into()), release(true), release(false)]
    );
    assert_eq!(rig.doc(), ";addrq|");
    assert!(rig.host.forms.lock().unwrap().is_empty());
}

/// A Quick Search form has nothing to delete, so typing in the target doesn't stop it: it
/// inserts at the caret.
#[test]
fn a_palette_form_inserts_at_the_caret_whatever_was_typed() {
    let rig = Rig::new(&[form_snippet()]);
    let snippet = rig.snippet_id(";addr");
    rig.engine.insert_snippet(snippet, Some(editor()));
    rig.settle();
    let token = rig.host.form_token();
    rig.type_("q");
    rig.clear_log();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(
        rig.log(),
        [
            Ev::Activated(editor().bundle_id),
            Ev::Slept(ms(50)),
            Ev::Out(Out::Type("Dear Sam, hi".into()))
        ]
    );
    assert_eq!(rig.doc(), "qDear Sam, hi|");
}
