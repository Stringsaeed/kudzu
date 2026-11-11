//! EXP-10, Kudzu's own pasteboard operations against each other. A snippet paste is one
//! transaction from its snapshot to its ⌘V, and its late restore is another; a clipboard-history
//! write or a second paste on another thread waits for the one in progress, whichever step it
//! is at. Before this, a pick at the snapshot was lost, a pick after the write was what the
//! target pasted, and a second paste saved the first's snippet as the user's clipboard.
//!
//! The other operation starts on its own thread at a scripted point and is known to be running
//! before the paste goes on. It can only finish once the paste lets go, so the outcome doesn't
//! depend on timing; it is joined at a later scripted point (or by the test) to fix the order.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use super::desk::{plain, user_clipboard, At, BoardOp, Desk, Ev, Out, PLAIN};
use super::fakes::{FakeEvents, FakeObserve};
use super::rig::{backspaces, ms, outs, pasted, Rig};
use crate::macos::pasteboard::{Items, Pasteboard, RESTORE_DELAY};

type Op = Box<dyn FnOnce(&Pasteboard, &Desk) + Send>;

/// An operation on another thread.
#[derive(Clone, Default)]
struct Other(Arc<Mutex<Option<JoinHandle<()>>>>);

impl Other {
    /// Starts `op` when `at` comes, and returns once its thread is running.
    fn start_at(&self, rig: &Rig, at: At, op: Op) {
        let (slot, pasteboard) = (self.0.clone(), rig.pasteboard.clone());
        rig.at(at, move |d| {
            let (started, running) = mpsc::channel();
            let desk = d.clone();
            let handle = thread::spawn(move || {
                started.send(()).unwrap();
                op(&pasteboard, &desk);
            });
            running.recv().unwrap();
            *slot.lock().unwrap() = Some(handle);
        });
    }

    /// Waits for the operation to finish.
    fn join(&self) {
        if let Some(handle) = self.0.lock().unwrap().take() {
            handle.join().expect("the other operation panicked");
        }
    }

    /// Joins when `at` comes.
    fn join_at(&self, rig: &Rig, at: At) {
        let other = self.clone();
        rig.at(at, move |_| other.join());
    }
}

fn picked() -> Items {
    vec![vec![(PLAIN.to_string(), b"picked".to_vec())]]
}

fn pick() -> Op {
    Box::new(|pasteboard, _| {
        pasteboard.write_items(&picked());
    })
}

fn second_paste() -> Op {
    Box::new(|pasteboard, desk| {
        let (events, clock) = (FakeEvents(desk.clone()), FakeObserve(desk.clone()));
        pasteboard
            .paste_text(&events, &clock, "second", None)
            .unwrap();
    })
}

fn transient(text: &str) -> Ev {
    Ev::Board(BoardOp::Transient {
        text: text.into(),
        html: false,
    })
}

fn wrote(text: &str) -> Ev {
    Ev::Board(BoardOp::Wrote(Some(text.into())))
}

/// A history pick started at `at`, while `;sig` pastes, joined once the paste waits to restore.
fn pick_during_paste(at: At) -> Rig {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let other = Other::default();
    other.start_at(&rig, at, pick());
    other.join_at(&rig, At::Sleep(RESTORE_DELAY, 0));
    rig.type_(";sig");
    // The pick runs after the ⌘V: it puts the user's clipboard back, then writes itself, and
    // the paste's late restore leaves it alone.
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            wrote("user text"),
            wrote("picked"),
            Ev::Slept(ms(400)),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "Best, Sam|");
    assert_eq!(plain(&rig.clipboard()).as_deref(), Some("picked"));
    assert!(rig.pasteboard.is_own_change(rig.with(|s| s.count)));
    rig
}

#[test]
fn a_history_pick_at_the_snapshot_waits_for_the_paste() {
    pick_during_paste(At::Snapshot(0));
}

#[test]
fn a_history_pick_after_the_write_waits_for_the_paste() {
    pick_during_paste(At::Transient(0));
}

/// The record is installed and the trigger is being deleted; the ⌘V hasn't gone out.
#[test]
fn a_history_pick_after_the_record_is_installed_waits_for_the_paste() {
    pick_during_paste(At::Press(0));
}

/// A second paste after the first one's write: it runs after the ⌘V, so it saves the user's
/// clipboard, not the first snippet, and the target pastes each snippet once.
#[test]
fn a_second_paste_after_the_write_waits_for_the_first() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let other = Other::default();
    other.start_at(&rig, At::Transient(0), second_paste());
    other.join_at(&rig, At::Sleep(RESTORE_DELAY, 0));
    rig.type_(";sig");
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            wrote("user text"),
            transient("second"),
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            wrote("user text"),
            Ev::Slept(ms(400)),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "Best, Samsecond|");
    assert_eq!(rig.clipboard(), user_clipboard());
}

/// A history pick as the paste starts putting the user's clipboard back: it waits for the
/// restore, so the restore can't overwrite it.
#[test]
fn a_history_pick_during_the_restore_waits_for_it() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let other = Other::default();
    other.start_at(&rig, At::Writing(0), pick());
    rig.type_(";sig");
    other.join();
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            wrote("user text"),
            wrote("picked"),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(plain(&rig.clipboard()).as_deref(), Some("picked"));
}

/// Another paste starts while the first waits, and is still waiting when the first one's late
/// restore comes. That restore belongs to the first paste only: the second paste's snippet is
/// still on the clipboard until its own restore.
#[test]
fn a_late_restore_leaves_a_newer_paste_alone() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    let other = Other::default();
    let (sleeping, asleep) = mpsc::channel::<()>();
    let asleep = Mutex::new(asleep);
    let seen: Arc<Mutex<Option<Option<String>>>> = Arc::default();
    let seen_by_second = seen.clone();
    // The second paste's wait: it lets the first paste go on, waits for it to finish, and looks.
    rig.at(At::Sleep(RESTORE_DELAY, 1), move |d| {
        sleeping.send(()).unwrap();
        let engine = d.engine();
        while !engine.imp.idle() {
            thread::yield_now();
        }
        *seen_by_second.lock().unwrap() = Some(plain(&d.clipboard()));
    });
    other.start_at(&rig, At::Sleep(RESTORE_DELAY, 0), second_paste());
    rig.at(At::Sleep(RESTORE_DELAY, 0), move |_| {
        asleep.lock().unwrap().recv().unwrap();
    });
    rig.type_(";sig");
    other.join();
    assert_eq!(
        *seen.lock().unwrap(),
        Some(Some("second".into())),
        "the first paste's restore finished the second one early"
    );
    let expected = [
        vec![transient("Best, Sam")],
        outs(backspaces(3)),
        vec![
            Ev::Out(Out::Paste),
            wrote("user text"),
            transient("second"),
            Ev::Out(Out::Paste),
            Ev::Slept(ms(400)),
            Ev::Slept(ms(400)),
            wrote("user text"),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "Best, Samsecond|");
    assert_eq!(rig.clipboard(), user_clipboard());
}
