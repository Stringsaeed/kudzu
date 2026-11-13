//! What a paste inserts, where it goes, and what the footer says when it cannot: the S60
//! review's B1, B2 and M3.

use std::sync::Arc;
use std::time::Duration;

use kudzu_app::clipboard::{ClipboardError, Flavor};
use kudzu_app::error::PortError;
use kudzu_app::fixtures::apps::Latch;
use kudzu_core::system::AppInfo;
use rune::Key;

use super::{key, plain, Rig};
use crate::picker::model::{ERROR_FLASH_SECS, FALLBACK_HIDE_AFTER, FALLBACK_NOTICE};

fn kudzu_itself() -> AppInfo {
    AppInfo {
        bundle_id: "sh.saeed.kudzu.next".into(),
        name: "Kudzu Next".into(),
        path: None,
        pid: Some(std::process::id() as i32),
    }
}

// MARK: B1 — one paste at a time

#[test]
fn a_reopened_picker_cannot_change_what_a_pending_paste_inserts() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    let hold = Arc::new(Latch::default());
    *rig.fixtures.apps.activation_hold.lock().unwrap() = Some(hold.clone());
    rig.press(key(Key::Enter));
    // The paste worker is inside the target's activation, which is held.
    rig.fixtures.apps.activation_entered.wait();
    assert!(!rig.open_now(), "the panel hid for the paste");

    // The user reopens the picker and picks the other clip.
    rig.open();
    rig.press(key(Key::Down));
    assert_eq!(rig.active_text(), "first");
    rig.press(key(Key::Enter));
    rig.until("the refusal", |r| r.flash().is_some());
    assert_eq!(
        rig.flash().as_deref(),
        Some(ClipboardError::PasteBusy.to_string().as_str()),
        "the second pick says why it did nothing"
    );
    assert!(rig.open_now(), "and the picker stays for another try");

    hold.set();
    rig.until("the first paste", |r| r.pasted() == 1);
    assert_eq!(
        rig.pasted_text(),
        vec![Some("second".to_owned())],
        "the first paste inserted the clip it was asked for"
    );

    // Once it finished, the same pick goes through with its own clip.
    rig.press(key(Key::Enter));
    rig.until("the second paste", |r| r.pasted() == 2);
    assert_eq!(
        rig.pasted_text(),
        vec![Some("second".to_owned()), Some("first".to_owned())]
    );
}

#[test]
fn asking_twice_in_one_opening_pastes_once() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    rig.rt.key(key(Key::Enter));
    rig.rt.key(key(Key::Enter));
    rig.until("the paste", |r| r.pasted() == 1);
    rig.settle();
    assert_eq!(rig.pasted_text(), vec![Some("second".to_owned())]);
    assert_eq!(rig.flash(), None, "the second Return was not an error");
}

#[test]
fn closing_the_picker_before_its_paste_starts_frees_the_clipboard() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    let id = rig
        .state()
        .model
        .picker
        .active()
        .map(|e| e.id)
        .expect("a clip is selected");
    // Return, then Escape before any poll takes the copy: the opening ends before its paste
    // can start.
    rig.rt.update(move |s, _| {
        crate::picker::drive::paste(s, id, false);
        s.dismiss_with(true);
    });

    // Giving focus back waits for the paste gate; it must not wait for the next opening.
    rig.until("focus given back", |r| {
        !r.fixtures.apps.activations.lock().unwrap().is_empty()
    });
    assert!(!rig.open_now());

    // A plain copy elsewhere (the main window's Copy) goes through.
    let clipboard = rig.kudzu.clipboard.clone();
    let (sent, got) = std::sync::mpsc::channel();
    std::thread::spawn(move || sent.send(clipboard.copy(id, Flavor::Plain)));
    let copied = got
        .recv_timeout(Duration::from_secs(5))
        .expect("a later copy waited for a paste that never started");
    assert!(copied.is_ok(), "the copy went through: {copied:?}");
    rig.settle();
    assert_eq!(rig.pasted(), 0, "nothing was pasted");
}

// MARK: B2 — never into Kudzu

#[test]
fn with_kudzu_in_front_the_picker_only_copies() {
    let mut rig = Rig::new();
    rig.copy("clip");
    *rig.fixtures.apps.frontmost.lock().unwrap() = Some(kudzu_itself());
    rig.open();
    assert_eq!(rig.state().model.picker.paste_label(), "Copy");
    assert!(rig.has_text("Copy"));
    rig.press(key(Key::Enter));
    rig.until("the sentence", |r| {
        r.flash().as_deref() == Some(FALLBACK_NOTICE)
    });
    assert_eq!(rig.pasted(), 0, "nothing was posted into Kudzu");
    assert!(rig.fixtures.apps.activations.lock().unwrap().is_empty());
    rig.advance(FALLBACK_HIDE_AFTER + 0.1);
    assert!(!rig.open_now());
    assert_eq!(
        rig.fixtures.pasteboard.snapshot.lock().unwrap().items[0][0].1,
        b"clip".to_vec(),
        "the clip is on the pasteboard"
    );
}

#[test]
fn with_no_app_in_front_the_picker_only_copies() {
    let mut rig = Rig::new();
    rig.copy("clip");
    *rig.fixtures.apps.frontmost.lock().unwrap() = None;
    rig.open();
    assert_eq!(rig.state().model.picker.paste_label(), "Copy");
    rig.press(key(Key::Enter));
    rig.until("the sentence", |r| {
        r.flash().as_deref() == Some(FALLBACK_NOTICE)
    });
    assert_eq!(rig.pasted(), 0);
}

// MARK: M3 — a failure after the panel hid is still seen

#[test]
fn a_target_that_quit_brings_the_panel_back_with_the_sentence_and_it_goes_again() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    // The app is gone by the time the paste activates it.
    *rig.fixtures.apps.failure.lock().unwrap() = Some(PortError::TargetGone);
    rig.press(key(Key::Enter));
    let sentence = ClipboardError::TargetGone.to_string();
    rig.until("the sentence", |r| {
        r.open_now() && r.flash().as_deref() == Some(sentence.as_str())
    });
    assert!(rig.has_text(&sentence), "it is on screen");
    rig.advance(ERROR_FLASH_SECS - 0.2);
    assert!(rig.open_now() && rig.flash().is_some());
    rig.advance(0.4);
    assert!(!rig.open_now(), "it hides again once read");
    assert_eq!(rig.flash(), None);
    assert_eq!(rig.pasted(), 0);
}

#[test]
fn losing_permission_after_the_picker_opened_shows_the_copy_sentence() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    assert_eq!(rig.state().model.picker.paste_label(), "Paste into Notes");
    *rig.fixtures.pasteboard.paste_allowed.lock().unwrap() = false;
    rig.press(key(Key::Enter));
    rig.until("the sentence", |r| {
        r.open_now() && r.flash().as_deref() == Some(FALLBACK_NOTICE)
    });
    assert!(rig.has_text(FALLBACK_NOTICE));
    assert_eq!(rig.pasted(), 0);
    rig.advance(FALLBACK_HIDE_AFTER + 0.1);
    assert!(!rig.open_now(), "it hides after the fallback delay");
}

#[test]
fn a_late_failure_does_not_touch_a_newer_opening() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    let hold = Arc::new(Latch::default());
    *rig.fixtures.apps.activation_hold.lock().unwrap() = Some(hold.clone());
    rig.press(key(Key::Enter));
    rig.fixtures.apps.activation_entered.wait();
    // A newer opening begins while the first paste is still waiting for its target.
    rig.open();
    rig.press(plain('x'));
    // The target quits; the old paste now fails.
    *rig.fixtures.apps.failure.lock().unwrap() = Some(PortError::TargetGone);
    hold.set();
    rig.until("the failure to be taken", |r| {
        r.state().pastes_pending() == 0
    });
    assert!(rig.open_now(), "the newer opening is still there");
    assert_eq!(rig.query(), "x", "its query is untouched");
    assert_eq!(
        rig.flash(),
        None,
        "and the old failure is not on its footer"
    );
}
