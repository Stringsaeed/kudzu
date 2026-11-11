//! FRM-4 after the S34 re-review, on the modelled panels: focus reports that arrive late or
//! never, events with equal or missing stamps, keys the session routes elsewhere, suppression
//! held through a submission, and which windows count as Kudzu's.

use kudzu_core::activity::ActivityOutcome;

use super::desk::{editor, At, Out};
use super::focus::{form_outcomes, inserted, opened};
use super::panels::{windows, Focus};
use super::rig::{ms, snippet, Rig};

// MARK: Reports that arrive late or never (re-review blocker 2)

/// Focus leaves the form without a click, a key reaches the target, and the user clicks back
/// on the form and submits, all before the panel's reports arrive. They arrive while the
/// target comes forward, after the submission: they still count, and the key that went to
/// the target interrupts the form.
#[test]
fn reports_that_arrive_after_the_submission_still_catch_typing_in_the_target() {
    let (rig, token) = opened(true);
    rig.focus_moves(Focus::Target);
    rig.press("z");
    rig.click_on(Focus::Form, None);
    rig.at(At::Activate(0), |d| d.deliver_reports());
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The same when the blur report never arrives: the engine can't tell where that key went, so
/// it deletes nothing.
#[test]
fn a_blur_report_that_never_arrives_fails_closed() {
    let (rig, token) = opened(true);
    rig.focus_moves(Focus::Target);
    rig.press("z");
    rig.click_on(Focus::Form, None);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Reports that arrive late with no typing in between: the form goes ahead.
#[test]
fn late_reports_without_typing_elsewhere_let_the_form_go_ahead() {
    let (rig, token) = opened(true);
    rig.focus_moves(Focus::Panel);
    rig.click_on(Focus::Form, None);
    rig.press("Sam");
    rig.at(At::Activate(0), |d| d.deliver_reports());
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
}

// MARK: Equal and missing timestamps (re-review major 7)

/// The panel closes late and a key reaches the target with the very same timestamp as the
/// submitting Return. A different event with that stamp is not the submission: interrupted.
#[test]
fn another_key_with_the_submit_events_stamp_interrupts() {
    let (rig, token) = opened(true);
    rig.press("Sam⏎");
    let stamp = rig.with(|s| s.last_down);
    rig.focus_moves(Focus::Target);
    rig.key_at_focus_stamped("z", true, stamp);
    rig.settle();
    rig.key_at_focus_stamped("z", false, stamp);
    rig.settle();
    rig.submit_by_return(token, &[("who", "Sam")], stamp);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The submitting Return itself, with its own stamp, is part of the form.
#[test]
fn the_submitting_key_itself_is_part_of_the_form() {
    let (rig, token) = opened(true);
    rig.press("Sam⏎");
    let stamp = rig.with(|s| s.last_down);
    rig.submit_by_return(token, &[("who", "Sam")], stamp);
    assert_eq!(rig.log(), inserted());
}

/// A key-down with no timestamp can't be placed before the submission: interrupted.
#[test]
fn a_key_without_a_timestamp_interrupts() {
    let (rig, token) = opened(true);
    rig.press("Sam⏎");
    let stamp = rig.with(|s| s.last_down);
    rig.focus_moves(Focus::Target);
    rig.key_at_focus_stamped("z", true, 0);
    rig.settle();
    rig.key_at_focus_stamped("z", false, 0);
    rig.settle();
    rig.submit_by_return(token, &[("who", "Sam")], stamp);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The session says where a key goes: one for another process interrupts the form even while
/// its panel counts as key, whatever the reports say.
#[test]
fn a_key_the_session_routes_to_another_process_interrupts() {
    let (rig, token) = opened(true);
    rig.press("Sa");
    let target = super::desk::editor().pid.unwrap();
    rig.key_at_focus_to("m", true, target);
    rig.settle();
    rig.key_at_focus_to("m", false, target);
    rig.settle();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The form window couldn't say which event submitted: typing since the last report can't be
/// placed before it, so the form deletes nothing.
#[test]
fn a_submission_without_its_event_after_typing_fails_closed() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.submit_by(token, &[("who", "Sam")], None);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// A submission event with a zero stamp is as good as none.
#[test]
fn a_submission_event_without_a_stamp_fails_closed() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    let unstamped = crate::InputEvent {
        stamp: crate::EventStamp(0),
        key_code: Some(36),
    };
    rig.submit_by(token, &[("who", "Sam")], Some(unstamped));
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

// MARK: Suppression doesn't hide a cancellation (re-review major 4)

/// A submitted form's sequence pauses; the user clicks in the target, moving its caret, while
/// suppression is still held (by the forms service or another owner). The rest of the sequence
/// doesn't go in at the new caret.
#[test]
fn a_click_during_a_submitted_forms_pause_stops_it_while_suppressed() {
    let rig = Rig::new(&[snippet(
        ";addr",
        "Dear {{field name=who}}{{delay ms=100}}, hi",
    )]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.show_form(token);
    rig.press("Sam");
    rig.at(At::Sleep(ms(50), 2), |d| d.clicks_text(0));
    rig.submit(token, &[("who", "Sam")]);
    assert!(
        !rig.posted().contains(&Out::Type(", hi".into())),
        "{:?}",
        rig.posted()
    );
    assert_eq!(rig.doc(), "|Dear Sam");
}

/// The same for a Quick Search sequence inserted while a suppression guard is held.
#[test]
fn a_click_during_a_quick_search_pause_stops_it_while_suppressed() {
    let rig = Rig::new(&[snippet(";q", "one{{delay ms=100}}two")]);
    rig.engine.set_suppressed(true);
    rig.at(At::Sleep(ms(50), 1), |d| d.clicks_text(0));
    rig.engine
        .insert_snippet(rig.snippet_id(";q"), Some(editor()));
    rig.settle();
    assert_eq!(rig.posted(), [Out::Type("one".into())]);
    assert_eq!(rig.doc(), "|one");
}

// MARK: Which windows are Kudzu's (re-review majors 8 and 9)

/// The window that can handle the click isn't named, though the one under the pointer is the
/// form panel: that window may pass the click through, so it isn't proof. Interrupted.
#[test]
fn a_click_naming_only_the_window_under_the_pointer_interrupts() {
    let (rig, token) = opened(false);
    rig.click_with_fields(windows::FORM, 0);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// A Kudzu window the host says passes clicks through isn't where the click lands.
#[test]
fn a_click_on_a_kudzu_window_that_passes_clicks_through_interrupts() {
    let (rig, token) = opened(false);
    rig.engine.set_kudzu_windows(&[
        crate::KudzuWindow {
            number: windows::FORM as u32,
            passes_clicks_through: false,
        },
        crate::KudzuWindow {
            number: windows::PANEL as u32,
            passes_clicks_through: true,
        },
    ]);
    rig.click_in(windows::PANEL);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// A window the host no longer lists (it closed) isn't Kudzu's any more.
#[test]
fn a_click_on_a_window_kudzu_no_longer_lists_interrupts() {
    let (rig, token) = opened(false);
    rig.engine.set_kudzu_windows(&[crate::KudzuWindow {
        number: windows::FORM as u32,
        passes_clicks_through: false,
    }]);
    rig.click_in(windows::PANEL);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Until the host says which windows are Kudzu's, no click is known to be in Kudzu.
#[test]
fn without_kudzus_windows_every_click_interrupts() {
    let (rig, token) = opened(false);
    rig.engine.set_kudzu_windows(&[]);
    rig.click_in(windows::FORM);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

// MARK: A click's process (third review, majors 2 and 5)

/// Kudzu's window N closed and the target's new window got number N before the host's next
/// push: the stale snapshot matches, but the session routes the click to the target's process.
#[test]
fn a_click_the_session_routes_to_another_process_interrupts_despite_the_snapshot() {
    let (rig, token) = opened(false);
    rig.click_routed_to(windows::FORM, editor().pid.unwrap());
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The session naming Kudzu's process still needs the snapshot to name the window.
#[test]
fn a_click_routed_to_kudzu_still_needs_a_listed_window() {
    let (rig, token) = opened(false);
    rig.click_routed_to(windows::OTHER, super::desk::own_pid());
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Routed to Kudzu and on a listed window: the form goes ahead.
#[test]
fn a_click_routed_to_kudzu_on_a_listed_window_does_not_interrupt() {
    let (rig, token) = opened(false);
    rig.click_routed_to(windows::FORM, super::desk::own_pid());
    rig.press("Sam");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
}

/// A double-click on Submit: the second click arrives while the target comes forward. It
/// lands on Kudzu's panel, so it can't move the target's caret, and the submission goes ahead.
#[test]
fn the_second_click_of_a_double_click_on_submit_does_not_interrupt() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.at(At::Activate(0), |d| d.click_in(windows::FORM));
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
}
