//! Every timer expires: the footer flash, the 420 ms close after ⌘↩, the 1,800 ms close
//! without permission, the error and the undo messages.

use rune::Key;

use super::{cmd, key, with, Rig};
use crate::picker::model::{FALLBACK_NOTICE, FLASH_SECS};

#[test]
fn a_pin_message_goes_after_1_6_seconds() {
    let mut rig = Rig::with_clips(&["a"]);
    rig.press(cmd('p'));
    assert!(rig.flash().is_some());
    rig.advance(FLASH_SECS - 0.05);
    assert!(rig.flash().is_some(), "still there just before");
    rig.advance(0.1);
    assert_eq!(rig.flash(), None, "gone just after");
    assert!(!rig.has_text("Pinned. It won't expire."));
}

#[test]
fn the_delete_message_stays_four_seconds() {
    let mut rig = Rig::with_clips(&["a", "b"]);
    rig.press(with(Key::Backspace, true, false, false));
    rig.advance(3.9);
    assert!(rig.flash().is_some());
    rig.advance(0.2);
    assert_eq!(rig.flash(), None);
}

#[test]
fn command_return_closes_after_420_milliseconds_and_gives_focus_back() {
    let mut rig = Rig::with_clips(&["a"]);
    rig.press(with(Key::Enter, true, false, false));
    rig.until("Copied", |r| r.flash().as_deref() == Some("Copied"));
    let started = rig.state().model.clock_secs;
    let due = rig.state().model.picker.hide.unwrap().due;
    assert!(
        (due - started - 0.42).abs() < 0.05,
        "due {due} from {started}"
    );
    rig.advance(0.30);
    assert!(rig.open_now());
    rig.advance(0.20);
    assert!(!rig.open_now(), "closed after the 420 ms");
    rig.until("focus back", |r| {
        !r.fixtures.apps.activations.lock().unwrap().is_empty()
    });
    assert!(
        rig.state().model.picker.hide.is_none(),
        "the timer is spent"
    );
}

#[test]
fn without_permission_the_picker_closes_after_1800_milliseconds() {
    let mut rig = Rig::new();
    *rig.fixtures.pasteboard.paste_allowed.lock().unwrap() = false;
    rig.copy("a");
    rig.open();
    rig.press(key(Key::Enter));
    rig.until("the sentence", |r| {
        r.flash().as_deref() == Some(FALLBACK_NOTICE)
    });
    rig.advance(1.7);
    assert!(rig.open_now(), "still readable at 1.7 s");
    assert!(rig.flash().is_some());
    rig.advance(0.2);
    assert!(!rig.open_now(), "closed by 1.9 s");
}

#[test]
fn the_fallback_sentence_itself_would_last_2_6_seconds() {
    let mut p = crate::picker::model::Picker::new(crate::clip::model::tests::now());
    p.say(
        FALLBACK_NOTICE,
        crate::picker::model::FALLBACK_FLASH_SECS,
        0.0,
    );
    assert!(!p.expire(2.59));
    assert!(p.expire(2.6));
}

#[test]
fn an_error_message_stays_three_seconds_then_goes() {
    let mut rig = Rig::new();
    let sample = kudzu_app::fixtures::SampleState::load().unwrap();
    rig.kudzu
        .clipboard
        .replace_history(sample.clipboard)
        .unwrap();
    rig.open();
    rig.press(key(Key::Enter));
    rig.until("the error", |r| r.flash().is_some());
    rig.advance(2.9);
    assert!(rig.flash().is_some());
    rig.advance(0.2);
    assert_eq!(rig.flash(), None);
}

#[test]
fn a_new_opening_cancels_a_pending_close() {
    let mut rig = Rig::with_clips(&["a"]);
    rig.press(with(Key::Enter, true, false, false));
    rig.until("Copied", |r| r.flash().as_deref() == Some("Copied"));
    rig.rt.update(|s, _| s.dismiss());
    rig.open();
    rig.advance(1.0);
    assert!(
        rig.open_now(),
        "the old 420 ms close did not hit the new opening"
    );
    assert!(rig.state().model.picker.hide.is_none());
}
