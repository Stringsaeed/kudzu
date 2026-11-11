//! D14 across expansion kinds: a refused paste never loses text. The clipboard is saved and
//! written before the trigger's first Backspace, so a refused snapshot or write deletes nothing
//! (the key the listener withheld is typed back). A Backspace or ⌘V refused afterwards puts the
//! clipboard back and types back what was deleted. When typing back is refused too (posting
//! permission went away), the clipboard is still the user's and the host is told the trigger
//! is lost, so the app can say so.

use kudzu_core::activity::{ActivityOutcome, ActivitySource, InsertFailure};
use kudzu_core::{InsertionMethod, SnippetRecord};

use super::desk::{editor, user_clipboard, At, BoardOp, Desk, Ev, Out};
use super::rig::{backspaces, ms, outs, pasted, snippet, whole_word, Rig};
use crate::TriggerLost;

fn transient(text: &str) -> Ev {
    Ev::Board(BoardOp::Transient {
        text: text.into(),
        html: false,
    })
}

fn restored() -> Ev {
    Ev::Board(BoardOp::Wrote(Some("user text".into())))
}

fn typed(text: &str) -> Ev {
    Ev::Out(Out::Type(text.into()))
}

fn failed(error: InsertFailure) -> Vec<ActivityOutcome> {
    vec![ActivityOutcome::InsertionFailed { error }]
}

fn sequence() -> SnippetRecord {
    pasted(";seq", "first{{key name=tab}}second")
}

/// The `;addr` form, pasted, typed and on its key panel; `setup` adjusts the desk first.
fn form(setup: impl FnOnce(&Desk)) -> (Rig, u64) {
    let snippet = SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..snippet(";addr", "Dear {{field name=who}}, hi")
    };
    let rig = Rig::new(&[snippet]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.focus_panel(token, true);
    rig.type_in_panel("Sam");
    setup(&rig.desk);
    rig.clear_log();
    (rig, token)
}

fn form_outcomes(rig: &Rig) -> Vec<ActivityOutcome> {
    let activity = rig.host.activity.lock().unwrap();
    let forms = activity
        .iter()
        .filter(|e| e.source == Some(ActivitySource::Form));
    forms.map(|e| e.outcome.clone()).collect()
}

// MARK: Refused before anything is deleted

#[test]
fn a_whole_word_paste_whose_write_fails_deletes_nothing() {
    let snippet = SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..whole_word("sig", "Best, Sam")
    };
    let rig = Rig::with(&[snippet], |d| d.with(|s| s.transient_fails = true));
    rig.type_("sig ");
    assert_eq!(rig.log(), [restored(), typed(" ")]);
    assert_eq!(rig.doc(), "sig |");
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::PasteUnavailable));
}

#[test]
fn a_sequence_whose_first_snapshot_is_refused_deletes_nothing() {
    let rig = Rig::with(&[sequence()], |d| d.with(|s| s.unreadable = true));
    rig.type_(";seq");
    assert_eq!(rig.log(), [typed("q")]);
    assert_eq!(rig.doc(), ";seq|");
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::PasteUnavailable));
}

#[test]
fn a_sequence_whose_first_write_fails_deletes_nothing() {
    let rig = Rig::with(&[sequence()], |d| d.with(|s| s.transient_fails = true));
    rig.type_(";seq");
    assert_eq!(rig.log(), [restored(), typed("q")]);
    assert_eq!(rig.doc(), ";seq|");
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// The sequence presses Tab before it pastes, so the trigger must be deleted before anything
/// is written. The clipboard is checked first: it can't be saved, so nothing happens.
#[test]
fn a_sequence_that_pastes_after_a_key_checks_the_clipboard_first() {
    let rig = Rig::with(&[pasted(";k", "{{key name=tab}}second")], |d| {
        d.with(|s| s.unreadable = true)
    });
    rig.type_(";k");
    assert_eq!(rig.log(), [typed("k")]);
    assert_eq!(rig.doc(), ";k|");
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::PasteUnavailable));
}

#[test]
fn a_submitted_form_whose_snapshot_is_refused_deletes_nothing() {
    let (rig, token) = form(|d| d.with(|s| s.unreadable = true));
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(
        rig.log(),
        [Ev::Activated(editor().bundle_id), Ev::Slept(ms(50))]
    );
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(form_outcomes(&rig), failed(InsertFailure::PasteUnavailable));
}

#[test]
fn a_submitted_form_whose_write_fails_deletes_nothing() {
    let (rig, token) = form(|d| d.with(|s| s.transient_fails = true));
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(
        rig.log(),
        [
            Ev::Activated(editor().bundle_id),
            Ev::Slept(ms(50)),
            restored()
        ]
    );
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(rig.clipboard(), user_clipboard());
}

// MARK: Refused after the Backspaces started

#[test]
fn a_sequence_whose_first_paste_keystroke_is_refused_types_the_trigger_back() {
    let rig = Rig::with(&[sequence()], |d| d.with(|s| s.paste_refused = true));
    rig.type_(";seq");
    let expected = [
        vec![transient("first")],
        outs(backspaces(3)),
        vec![restored(), typed(";seq")],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), ";seq|");
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::EventCreation));
}

#[test]
fn a_submitted_form_whose_paste_keystroke_is_refused_types_the_trigger_back() {
    let (rig, token) = form(|d| d.with(|s| s.paste_refused = true));
    rig.submit(token, &[("who", "Sam")]);
    let expected = [
        vec![
            Ev::Activated(editor().bundle_id),
            Ev::Slept(ms(50)),
            transient("Dear Sam, hi"),
        ],
        outs(backspaces(5)),
        vec![restored(), typed(";addr")],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(form_outcomes(&rig), failed(InsertFailure::EventCreation));
}

/// The second Backspace can't be created: the one that went out is typed back, with the
/// withheld key.
#[test]
fn a_backspace_refused_partway_types_back_what_was_deleted() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.refuse_press = Some(1))
    });
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(1)),
        vec![restored(), typed("ig")],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::EventCreation));
    assert!(rig.host.lost.lock().unwrap().is_empty());
}

// MARK: Typing back is refused too

fn lost(rig: &Rig) -> Vec<TriggerLost> {
    rig.host.lost.lock().unwrap().clone()
}

fn sig_lost(rig: &Rig) -> Vec<TriggerLost> {
    vec![TriggerLost {
        snippet_id: rig.snippet_id(";sig"),
        trigger: ";sig".into(),
    }]
}

/// Posting permission goes away after the first Backspace: the second is refused, and so is
/// typing back. The clipboard is the user's and the host hears what was lost.
#[test]
fn a_backspace_refused_for_lost_permission_reports_the_lost_trigger() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Press(0), |d| d.set_post_event(false));
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(1)),
        vec![restored()],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), ";s|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(lost(&rig), sig_lost(&rig));
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::NotAllowed));
}

/// The same after every Backspace went out and the ⌘V is refused.
#[test]
fn a_paste_keystroke_refused_for_lost_permission_reports_the_lost_trigger() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Press(2), |d| d.set_post_event(false));
    rig.type_(";sig");
    assert_eq!(rig.doc(), "|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(lost(&rig), sig_lost(&rig));
}

/// A refusal that deleted nothing and typed back the withheld key loses nothing.
#[test]
fn a_refusal_that_typed_back_the_withheld_key_loses_nothing() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.unreadable = true)
    });
    rig.type_(";sig");
    assert_eq!(rig.doc(), ";sig|");
    assert!(lost(&rig).is_empty());
}

// MARK: A leading delay (review finding 4)

/// `{{delay}}` first: the pause runs before anything is deleted, so a refused paste after it
/// still has the trigger to keep.
fn delayed() -> SnippetRecord {
    pasted(";d", "{{delay ms=100}}text")
}

fn pause() -> Vec<Ev> {
    vec![Ev::Slept(ms(50)); 2]
}

#[test]
fn a_refused_write_after_a_leading_delay_deletes_nothing() {
    let rig = Rig::with(&[delayed()], |d| d.with(|s| s.transient_fails = true));
    rig.type_(";d");
    assert_eq!(rig.log(), [pause(), vec![restored(), typed("d")]].concat());
    assert_eq!(rig.doc(), ";d|");
    assert_eq!(rig.host.outcomes(), failed(InsertFailure::PasteUnavailable));
}

#[test]
fn a_refused_paste_keystroke_after_a_leading_delay_types_the_trigger_back() {
    let rig = Rig::with(&[delayed()], |d| d.with(|s| s.paste_refused = true));
    rig.type_(";d");
    let expected = [
        pause(),
        vec![transient("text")],
        outs(backspaces(1)),
        vec![restored(), typed(";d")],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), ";d|");
}

#[test]
fn a_refused_whole_word_paste_after_a_leading_delay_keeps_trigger_and_separator() {
    let snippet = SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..whole_word("dly", "{{delay ms=100}}text")
    };
    let rig = Rig::with(&[snippet], |d| d.with(|s| s.paste_refused = true));
    rig.type_("dly ");
    assert_eq!(rig.doc(), "dly |");
    assert_eq!(rig.clipboard(), user_clipboard());
}

#[test]
fn a_submitted_form_with_a_leading_delay_whose_write_fails_deletes_nothing() {
    let snippet = SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..snippet(";addr", "{{delay ms=100}}Dear {{field name=who}}, hi")
    };
    let rig = Rig::new(&[snippet]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.focus_panel(token, true);
    rig.type_in_panel("Sam");
    rig.with(|s| s.transient_fails = true);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(form_outcomes(&rig), failed(InsertFailure::PasteUnavailable));
}

/// Posting permission goes away with the trigger's Backspace: the ⌘V and typing back are both
/// refused. The clipboard is the user's and the host hears what was lost (D17).
#[test]
fn a_leading_delay_then_lost_permission_reports_the_lost_trigger() {
    let rig = Rig::new(&[delayed()]);
    rig.at(At::Press(0), |d| d.set_post_event(false));
    rig.type_(";d");
    assert_eq!(rig.doc(), "|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(
        lost(&rig),
        [TriggerLost {
            snippet_id: rig.snippet_id(";d"),
            trigger: ";d".into(),
        }]
    );
}

// MARK: A half of a key press can't be created (review finding 5)

/// The second Backspace (press 1) can't be created, at its key-down or its key-up. Either way
/// nothing of that press went out: one Backspace is typed back, with the withheld key.
#[test]
fn a_backspace_with_either_half_refused_posts_nothing_of_it() {
    for down in [true, false] {
        let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
            d.with(|s| s.refuse_half = Some((1, down)))
        });
        rig.type_(";sig");
        let expected = [
            vec![transient("Best, Sam")],
            outs(backspaces(1)),
            vec![restored(), typed("ig")],
        ]
        .concat();
        assert_eq!(rig.log(), expected, "key-down refused: {down}");
        assert_eq!(rig.doc(), ";sig|", "key-down refused: {down}");
    }
}

/// The ⌘V (press 3) can't be created at either half: nothing is pasted, the clipboard comes
/// back and the trigger is typed back.
#[test]
fn a_paste_keystroke_with_either_half_refused_pastes_nothing() {
    for down in [true, false] {
        let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
            d.with(|s| s.refuse_half = Some((3, down)))
        });
        rig.type_(";sig");
        let expected = [
            vec![transient("Best, Sam")],
            outs(backspaces(3)),
            vec![restored(), typed(";sig")],
        ]
        .concat();
        assert_eq!(rig.log(), expected, "key-down refused: {down}");
        assert_eq!(rig.doc(), ";sig|", "key-down refused: {down}");
        assert_eq!(rig.clipboard(), user_clipboard());
    }
}
