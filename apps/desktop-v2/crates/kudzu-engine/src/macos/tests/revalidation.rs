//! Review blocker 3: an insertion re-checks that nothing changed right after the clipboard is
//! prepared (which can wait behind a selection copy or a slow app's promised data), before each
//! Backspace, before the ⌘V, and before typing anything back. A click (generation), another app
//! in front, a pause, or listen-only typing into the deletion range stops it there. Nothing is
//! typed into a changed context: when part of the trigger is already gone, the host hears it is
//! lost (D17).

use kudzu_core::activity::ActivityOutcome;

use super::desk::{mail, user_clipboard, At, BoardOp, Ev};
use super::rig::{backspaces, outs, pasted, snippet, Rig};
use crate::{TapMode, TriggerLost};

fn transient(text: &str) -> Ev {
    Ev::Board(BoardOp::Transient {
        text: text.into(),
        html: false,
    })
}

fn restored() -> Ev {
    Ev::Board(BoardOp::Wrote(Some("user text".into())))
}

fn lost(rig: &Rig) -> Vec<TriggerLost> {
    rig.host.lost.lock().unwrap().clone()
}

fn sig_lost(rig: &Rig) -> Vec<TriggerLost> {
    vec![TriggerLost {
        snippet_id: rig.snippet_id(";sig"),
        trigger: ";sig".into(),
    }]
}

/// The user clicks in the target while the clipboard is being saved: nothing is deleted and
/// the clipboard comes back.
#[test]
fn a_click_while_the_clipboard_is_prepared_deletes_nothing() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Snapshot(0), |d| d.clicks_text(1));
    rig.type_(";sig");
    assert_eq!(rig.log(), [transient("Best, Sam"), restored()]);
    assert_eq!(rig.doc(), ";|si");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(rig.host.outcomes(), [ActivityOutcome::Interrupted]);
}

/// Listen-only: a key typed while the clipboard is prepared reaches the document after the
/// trigger, inside the range the Backspaces would delete.
#[test]
fn listen_only_typing_while_the_clipboard_is_prepared_deletes_nothing() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.refuse = vec![TapMode::Active])
    });
    rig.at(At::Snapshot(0), |d| d.types("x"));
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";sigx|");
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// Another app comes forward while a preparation waits, and then the clipboard can't be saved:
/// the withheld key is not typed into the new app. The trigger is missing it, so the host hears.
#[test]
fn a_refused_preparation_types_nothing_into_another_app() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.unreadable = true)
    });
    rig.at(At::Snapshot(0), |d| d.set_frontmost(Some(mail())));
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";si|");
    assert_eq!(lost(&rig), sig_lost(&rig));
}

/// A click between two Backspaces: the next one would delete at the new caret, so they stop.
/// What went is gone (the caret moved), so the host hears.
#[test]
fn a_click_during_the_backspaces_stops_them() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Press(0), |d| d.clicks_text(0));
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(1)),
        vec![restored()],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "|;s");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(lost(&rig), sig_lost(&rig));
}

/// A click after the last Backspace, before the ⌘V: nothing is pasted at the new caret.
#[test]
fn a_click_before_the_paste_keystroke_stops_it() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Press(2), |d| d.clicks_text(0));
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![restored()],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "|");
    assert_eq!(lost(&rig), sig_lost(&rig));
    assert_eq!(rig.host.expansions(), 0);
}

/// A sequence whose first step pastes checks the same way before its Backspaces.
#[test]
fn a_click_while_a_sequence_prepares_deletes_nothing() {
    let rig = Rig::new(&[pasted(";sig", "first{{key name=tab}}second")]);
    rig.at(At::Snapshot(0), |d| d.clicks_text(1));
    rig.type_(";sig");
    assert_eq!(rig.log(), [transient("first"), restored()]);
    assert_eq!(rig.doc(), ";|si");
}

/// A Quick Search insert re-checks its target after preparing too: nothing is pasted into the
/// app that came forward meanwhile.
#[test]
fn a_quick_search_paste_into_an_app_that_lost_the_front_is_dropped() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Snapshot(0), |d| d.set_frontmost(Some(mail())));
    let editor = super::desk::editor();
    rig.engine
        .insert_snippet(rig.snippet_id(";sig"), Some(editor.clone()));
    rig.settle();
    assert_eq!(
        rig.log(),
        [
            Ev::Activated(editor.bundle_id),
            Ev::Slept(super::rig::ms(50)),
            transient("Best, Sam"),
            restored()
        ]
    );
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.clipboard(), user_clipboard());
}

// MARK: The intended target (re-review blocker 1)

/// Rendering a form's `{{clipboard}}` waits; meanwhile another app comes forward on its own.
/// The insertion is bound to the form's target, so nothing is deleted in the new app.
#[test]
fn an_app_that_comes_forward_while_a_form_renders_is_not_adopted() {
    let rig = Rig::new(&[snippet(";addr", "Dear {{field name=who}}, {{clipboard}}")]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.focus_panel(token, true);
    rig.type_in_panel("Sam");
    rig.at(At::ReadClipboard(0), |d| d.set_frontmost(Some(mail())));
    rig.clear_log();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(rig.host.expansions(), 0);
}

/// The same for Quick Search: the paste is not delivered to the app that came forward.
#[test]
fn an_app_that_comes_forward_while_quick_search_renders_is_not_adopted() {
    let rig = Rig::new(&[snippet(";c", "Copied: {{clipboard}}")]);
    rig.at(At::ReadClipboard(0), |d| d.set_frontmost(Some(mail())));
    rig.engine
        .insert_snippet(rig.snippet_id(";c"), Some(super::desk::editor()));
    rig.settle();
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), "|");
    assert_eq!(rig.host.expansions(), 0);
}
