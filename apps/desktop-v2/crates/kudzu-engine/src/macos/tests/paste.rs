//! EXP-10, paste and restore. A snippet paste saves every pasteboard item and flavour, writes
//! one transient, concealed item, presses ⌘V, and 400 ms later puts the user's clipboard back,
//! unless something newer was copied meanwhile.

use std::sync::{Arc, Mutex};

use kudzu_core::activity::{ActivityOutcome, InsertFailure};
use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};

use super::desk::{plain, user_clipboard, At, BoardOp, Ev, ListenerOp, Out, PLAIN};
use super::fakes::{FakeEvents, FakeObserve};
use super::rig::{backspaces, ms, outs, pasted, Rig};
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

    let expected = [
        outs(backspaces(3)),
        vec![
            transient("Best, Sam"),
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            restored(),
        ],
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
        outs(backspaces(3)),
        vec![
            Ev::Board(BoardOp::Transient {
                text: "Best, Sam".into(),
                html: true,
            }),
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            restored(),
        ],
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
        outs(backspaces(3)),
        vec![
            transient("Best, Sam"),
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

/// Current behaviour, kept as it is: the trigger is already deleted when the paste is refused,
/// so the document is left without trigger or snippet. Nothing on the clipboard is touched.
#[test]
fn an_unsafe_snapshot_refuses_the_paste() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.unreadable = true)
    });
    rig.type_(";sig");
    assert_eq!(rig.log(), outs(backspaces(3)));
    assert_eq!(rig.doc(), "|");
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::PasteUnavailable
        }]
    );
}

#[test]
fn a_failed_write_puts_the_clipboard_back() {
    let rig = Rig::with(&[pasted(";sig", "Best, Sam")], |d| {
        d.with(|s| s.transient_fails = true)
    });
    rig.type_(";sig");
    assert_eq!(rig.log(), [outs(backspaces(3)), vec![restored()]].concat());
    assert_eq!(rig.clipboard(), user_clipboard());
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::PasteUnavailable
        }]
    );
}

/// The ⌘V can't be posted (permission went away mid-insertion): the clipboard comes back at
/// once, without waiting. As with a refused snapshot, the trigger is already gone.
#[test]
fn a_failed_paste_keystroke_restores_at_once() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Press(2), |d| d.set_post_event(false));
    rig.type_(";sig");
    let expected = [
        outs(backspaces(3)),
        vec![transient("Best, Sam"), restored()],
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
        outs(backspaces(3)),
        vec![
            transient("Best, Sam"),
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
        outs(backspaces(3)),
        vec![
            transient("Best, Sam"),
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
        outs(backspaces(3)),
        vec![
            transient("Best, Sam"),
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
/// the next step.
#[test]
fn each_pasted_step_of_a_sequence_is_restored_before_the_next() {
    let rig = Rig::new(&[pasted(";seq", "first{{key name=tab}}second")]);
    rig.type_(";seq");
    let expected = [
        outs(backspaces(3)),
        vec![
            transient("first"),
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
