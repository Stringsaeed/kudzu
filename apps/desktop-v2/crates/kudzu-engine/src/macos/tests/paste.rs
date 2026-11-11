//! EXP-10, paste and restore. A snippet paste saves every pasteboard item and flavour, writes
//! one transient, concealed item, presses ⌘V, and 400 ms later puts the user's clipboard back,
//! unless something newer was copied meanwhile.
//!
//! **Deliberately different from the copied engine (D14):** the clipboard is saved and written
//! *before* the trigger's Backspaces, so a refused paste deletes nothing, and a ⌘V refused after
//! the Backspaces types the trigger back. The copied engine deleted first and lost the trigger.

use std::sync::{Arc, Mutex};

use kudzu_core::activity::{ActivityOutcome, InsertFailure};
use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};
use kudzu_core::{InsertionMethod, SnippetRecord};

use super::desk::{plain, user_clipboard, At, BoardOp, Ev, ListenerOp, Out, PLAIN};
use super::fakes::{FakeEvents, FakeObserve};
use super::rig::{backspaces, ms, outs, pasted, whole_word, Rig};
use crate::macos::pasteboard::{Items, RESTORE_DELAY};
use crate::TapMode;

fn transient(text: &str) -> Ev {
    Ev::Board(BoardOp::Transient {
        text: text.into(),
        html: false,
    })
}

fn restored() -> Ev {
    Ev::Board(BoardOp::Wrote(Some("user text".into())))
}

#[test]
fn a_paste_restores_every_item_and_flavour_after_400_ms() {
    assert_eq!(RESTORE_DELAY, ms(400));
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let during: Arc<Mutex<Items>> = Arc::default();
    let seen = during.clone();
    // The ⌘V is the fourth press, after three Backspaces.
    rig.at(At::Press(3), move |d| *seen.lock().unwrap() = d.clipboard());
    rig.type_(";sig");

    // D14: the clipboard is saved and written before the first Backspace.
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![Ev::Out(Out::Paste), Ev::Slept(ms(400)), restored()],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "Best, Sam|");
    // While the target read it, the clipboard held only the snippet, marked for clipboard
    // managers to skip.
    let types: Vec<String> = during.lock().unwrap()[0]
        .iter()
        .map(|(kind, _)| kind.clone())
        .collect();
    assert_eq!(
        types,
        [
            PLAIN,
            "org.nspasteboard.TransientType",
            "org.nspasteboard.ConcealedType"
        ]
    );
    // Afterwards every item and flavour is back, and the restore counts as Kudzu's own change
    // (the clipboard history must not record it as a copy).
    assert_eq!(rig.clipboard(), user_clipboard());
    assert!(rig.pasteboard.is_own_change(rig.with(|s| s.count)));
    assert_eq!(rig.host.expansions(), 1);
}

#[test]
fn a_formatted_paste_carries_html() {
    let mut snippet = pasted(";sig", "Best, Sam");
    snippet.formatting = Some(Formatting {
        source: snippet.content.clone(),
        spans: vec![FormatSpan {
            start: 0,
            end: 4,
            style: FormatStyle::Bold,
        }],
    });
    let rig = Rig::new(&[snippet]);
    let html: Arc<Mutex<Option<String>>> = Arc::default();
    let seen = html.clone();
    rig.at(At::Press(3), move |d| {
        let items = d.clipboard();
        let found = items[0].iter().find(|(kind, _)| kind == "public.html");
        *seen.lock().unwrap() = found.map(|(_, data)| String::from_utf8_lossy(data).into());
    });
    rig.type_(";sig");
    let expected = [
        vec![Ev::Board(BoardOp::Transient {
            text: "Best, Sam".into(),
            html: true,
        })],
        outs(backspaces(3)),
        vec![Ev::Out(Out::Paste), Ev::Slept(ms(400)), restored()],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    let html = html.lock().unwrap().clone().expect("an HTML flavour");
    assert!(html.contains("<strong>Best</strong>, Sam"), "{html}");
    assert_eq!(rig.clipboard(), user_clipboard());
}

#[test]
fn a_newer_copy_is_not_overwritten() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Sleep(RESTORE_DELAY, 0), |d| d.user_copies("newer"));
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            Ev::Board(BoardOp::UserCopy("newer".into())),
            Ev::Slept(ms(400)),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(plain(&rig.clipboard()).as_deref(), Some("newer"));
    assert_eq!(rig.pasteboard.change_count(), rig.with(|s| s.count));
    assert!(!rig.pasteboard.is_own_change(rig.with(|s| s.count)));
}

/// D14: the clipboard can't be saved, so nothing is deleted. The key the listener withheld is
/// typed back and the trigger stays as typed. Nothing on the clipboard is touched.
#[test]
fn an_unsafe_snapshot_refuses_the_paste() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.unreadable = true)
    });
    rig.type_(";sig");
    assert_eq!(rig.log(), outs(vec![Out::Type("g".into())]));
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::PasteUnavailable
        }]
    );
}

/// D14: the write fails, so nothing is deleted; the clipboard is put back and the withheld key
/// typed back.
#[test]
fn a_failed_write_puts_the_clipboard_back() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.transient_fails = true)
    });
    rig.type_(";sig");
    assert_eq!(
        rig.log(),
        [vec![restored()], outs(vec![Out::Type("g".into())])].concat()
    );
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::PasteUnavailable
        }]
    );
}

/// The ⌘V can't be posted (permission went away mid-insertion): the clipboard comes back at
/// once, without waiting. Typing the trigger back is refused too, so it is gone.
#[test]
fn a_failed_paste_keystroke_restores_at_once() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Press(2), |d| d.set_post_event(false));
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![restored()],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(rig.doc(), "|");
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::NotAllowed
        }]
    );
}

#[test]
fn stopping_the_engine_restores_a_pending_paste() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Sleep(RESTORE_DELAY, 0), |d| d.engine().stop());
    rig.type_(";sig");
    // The restore happens during the wait, when the engine stops, and only once.
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            Ev::Listener(ListenerOp::Stopped(TapMode::Active)),
            restored(),
            Ev::Slept(ms(400)),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// Two pastes overlap (another thread pastes while the first waits to restore). The second
/// first puts the user's clipboard back, so it saves and restores that, never the first
/// snippet.
#[test]
fn a_second_paste_finishes_the_first_restore_first() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let pasteboard = rig.pasteboard.clone();
    rig.at(At::Sleep(RESTORE_DELAY, 0), move |d| {
        let (events, clock) = (FakeEvents(d.clone()), FakeObserve(d.clone()));
        pasteboard
            .paste_text(&events, &clock, "second", None)
            .unwrap();
    });
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            restored(),
            transient("second"),
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            restored(),
            Ev::Slept(ms(400)),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "Best, Samsecond|");
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// A clipboard-history pick written while a snippet paste waits: the user's clipboard is put
/// back first, then the pick replaces it, and the late restore leaves the pick alone.
#[test]
fn a_history_pick_during_a_pending_restore_is_kept() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let pasteboard = rig.pasteboard.clone();
    rig.at(At::Sleep(RESTORE_DELAY, 0), move |_| {
        let picked = vec![vec![(PLAIN.to_string(), b"picked".to_vec())]];
        pasteboard.write_items(&picked);
    });
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            restored(),
            Ev::Board(BoardOp::Wrote(Some("picked".into()))),
            Ev::Slept(ms(400)),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(plain(&rig.clipboard()).as_deref(), Some("picked"));
    assert!(rig.pasteboard.is_own_change(rig.with(|s| s.count)));
}

/// TEMPLATE_SYNTAX "Key presses and pauses": each pasted piece of a sequence is restored before
/// the next step. The first piece is prepared before the Backspaces (D14).
#[test]
fn each_pasted_step_of_a_sequence_is_restored_before_the_next() {
    let rig = Rig::new(&[pasted(";seq", "first{{key name=tab}}second")]);
    rig.type_(";seq");
    let expected = [
        vec![transient("first")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            restored(),
            Ev::Out(Out::Tab),
            Ev::Slept(ms(30)),
            transient("second"),
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            restored(),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "first\tsecond|");
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// D14: posting ⌘V fails after the Backspaces (the event couldn't be created), so the clipboard
/// comes back at once and the whole trigger is typed back.
#[test]
fn a_refused_paste_keystroke_types_the_trigger_back() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.paste_refused = true)
    });
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![restored(), Ev::Out(Out::Type(";sig".into()))],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::EventCreation
        }]
    );
    assert_eq!(rig.host.expansions(), 0);
}

/// D14 for a whole-word trigger: the listener withheld the separator, so when the clipboard
/// can't be saved the separator is what is typed back.
#[test]
fn a_refused_whole_word_paste_types_the_separator_back() {
    let snippet = SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..whole_word("sig", "Best, Sam")
    };
    let rig = Rig::with(&[snippet], |d| d.with(|s| s.unreadable = true));
    rig.type_("sig ");
    assert_eq!(rig.log(), outs(vec![Out::Type(" ".into())]));
    assert_eq!(rig.doc(), "sig |");
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// D14 for a whole-word trigger: a ⌘V refused after the Backspaces types back the trigger and
/// its separator.
#[test]
fn a_refused_whole_word_paste_keystroke_types_trigger_and_separator_back() {
    let snippet = SnippetRecord {
        insertion: Some(InsertionMethod::Paste),
        ..whole_word("sig", "Best, Sam")
    };
    let rig = Rig::with(&[snippet], |d| d.with(|s| s.paste_refused = true));
    rig.type_("sig ");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![restored(), Ev::Out(Out::Type("sig ".into()))],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "sig |");
    assert_eq!(rig.clipboard(), user_clipboard());
}
