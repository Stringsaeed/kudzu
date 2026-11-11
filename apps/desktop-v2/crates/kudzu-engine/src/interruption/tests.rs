//! The interruption rules, event by event.

use super::*;
use crate::EventStamp;

const RETURN: u16 = 36;
const A: u16 = 0;

fn ev(stamp: u64, key_code: Option<u16>) -> Option<InputEvent> {
    Some(InputEvent {
        stamp: EventStamp(stamp),
        key_code,
    })
}

/// A report whose boundary is the key-down `after` (key code A).
fn report(key: bool, seq: u64, after: u64) -> PanelFocus {
    PanelFocus {
        key,
        seq,
        after: ev(after, Some(A)),
    }
}

fn submission(stamp: u64, key_code: Option<u16>, focus_seq: u64) -> Submission {
    Submission {
        event: ev(stamp, key_code),
        focus_seq,
    }
}

/// A form whose panel became key after event 1 (report 1).
fn key() -> Interruption {
    let mut form = Interruption::default();
    form.focus(report(true, 1, 1));
    form
}

#[test]
fn typing_and_clicking_in_the_key_panel_then_submitting_may_delete() {
    let mut form = key();
    form.click(true);
    form.key_down(2, A, false);
    form.key_down(3, RETURN, false);
    form.submit(submission(3, Some(RETURN), 1));
    assert!(form.may_delete());
}

#[test]
fn a_key_down_while_not_key_interrupts() {
    let mut form = key();
    form.focus(report(false, 2, 1));
    assert!(
        !form.interrupted(),
        "losing focus alone isn't an interruption"
    );
    form.key_down(3, A, false);
    assert!(form.interrupted());
}

#[test]
fn a_key_down_the_session_sends_elsewhere_interrupts_even_while_key() {
    let mut form = key();
    form.key_down(2, A, true);
    assert!(form.interrupted());
}

#[test]
fn a_click_outside_kudzu_interrupts_whatever_focus_does() {
    for (before, after) in [(true, true), (true, false), (false, true), (false, false)] {
        let mut form = Interruption::default();
        form.focus(report(before, 1, 1));
        form.click(false);
        form.focus(report(after, 2, 2));
        assert!(form.interrupted(), "key {before} then {after}");
    }
}

#[test]
fn a_late_loss_of_key_catches_the_typing_routed_after_it() {
    // Key-downs 2 and 3 came while the panel still counted as key; it had lost key after 2.
    let mut form = key();
    form.key_down(2, A, false);
    form.key_down(3, A, false);
    form.focus(report(false, 2, 2));
    form.submit(submission(4, None, 2));
    assert!(form.interrupted());
    // Lost after the last key-down: that typing was in the panel.
    let mut form = key();
    form.key_down(2, A, false);
    form.focus(report(false, 2, 2));
    form.focus(report(true, 3, 2));
    form.key_down(4, RETURN, false);
    form.submit(submission(4, Some(RETURN), 3));
    assert!(form.may_delete());
}

#[test]
fn a_later_gain_puts_the_typing_after_it_back_in_the_panel() {
    // Lost after 1, back after 3; key-down 2 went elsewhere, 4 into the panel.
    let mut in_between = key();
    in_between.key_down(2, A, false);
    in_between.focus(report(false, 2, 1));
    in_between.focus(report(true, 3, 3));
    in_between.submit(submission(5, None, 3));
    assert!(in_between.interrupted());
    let mut after_it = key();
    after_it.key_down(4, RETURN, false);
    after_it.focus(report(false, 2, 1));
    after_it.focus(report(true, 3, 3));
    after_it.submit(submission(4, Some(RETURN), 3));
    assert!(after_it.may_delete());
}

#[test]
fn a_missing_report_interrupts() {
    let mut form = key();
    form.focus(report(false, 3, 2));
    assert!(form.interrupted());
}

#[test]
fn a_repeated_report_changes_nothing() {
    let mut form = key();
    form.focus(report(true, 1, 1));
    form.key_down(2, RETURN, false);
    form.submit(submission(2, Some(RETURN), 1));
    assert!(form.may_delete());
}

#[test]
fn reports_counted_by_the_submission_still_count_when_they_arrive_after_it() {
    // Focus left (report 2, after 1), a key went to the target (2), focus came back (report 3,
    // after 3), Return submitted (4); reports 2 and 3 arrive after the submission.
    let mut form = key();
    form.key_down(2, A, false);
    form.key_down(4, RETURN, false);
    form.submit(submission(4, Some(RETURN), 3));
    assert!(!form.interrupted());
    assert!(!form.may_delete(), "reports 2 and 3 haven't arrived");
    form.focus(report(false, 2, 1));
    form.focus(report(true, 3, 3));
    assert!(form.interrupted());
}

#[test]
fn a_submission_waits_for_its_reports_and_then_may_delete() {
    let mut form = key();
    form.key_down(4, RETURN, false);
    form.submit(submission(4, Some(RETURN), 3));
    assert!(!form.may_delete());
    form.focus(report(false, 2, 1));
    assert!(!form.may_delete());
    form.focus(report(true, 3, 3));
    assert!(form.may_delete());
}

#[test]
fn reports_after_the_submissions_count_are_the_panel_closing() {
    let mut form = key();
    form.key_down(2, RETURN, false);
    form.submit(submission(2, Some(RETURN), 1));
    form.focus(report(false, 2, 1));
    assert!(form.may_delete());
}

#[test]
fn another_event_with_the_submitting_stamp_is_after_it() {
    let mut form = key();
    form.key_down(5, RETURN, false);
    form.key_down(5, A, false);
    form.submit(submission(5, Some(RETURN), 1));
    assert!(form.interrupted());
    // Two exact matches are ambiguous too.
    let mut form = key();
    form.key_down(5, RETURN, false);
    form.key_down(5, RETURN, false);
    form.submit(submission(5, Some(RETURN), 1));
    assert!(form.interrupted());
}

#[test]
fn another_event_with_a_reports_stamp_can_not_be_placed() {
    let mut form = key();
    form.key_down(2, RETURN, false);
    form.focus(report(false, 2, 2));
    form.submit(submission(3, None, 2));
    assert!(form.interrupted());
}

#[test]
fn a_key_down_without_a_stamp_can_not_be_placed() {
    let mut form = key();
    form.key_down(0, A, false);
    form.key_down(5, RETURN, false);
    form.submit(submission(5, Some(RETURN), 1));
    assert!(form.interrupted());
}

#[test]
fn a_submission_without_its_event_counts_typing_against_it() {
    for event in [None, ev(0, Some(RETURN))] {
        let mut typed = key();
        typed.key_down(2, A, false);
        typed.submit(Submission {
            event,
            focus_seq: 1,
        });
        assert!(typed.interrupted());
        let mut quiet = key();
        quiet.submit(Submission {
            event,
            focus_seq: 1,
        });
        assert!(quiet.may_delete(), "no typing since the last report");
    }
}

#[test]
fn a_report_without_a_boundary_can_not_place_earlier_typing() {
    let mut typed = key();
    typed.key_down(2, A, false);
    typed.focus(PanelFocus {
        key: false,
        seq: 2,
        after: None,
    });
    typed.submit(submission(3, None, 2));
    assert!(typed.interrupted());
}

#[test]
fn after_the_submission_any_key_or_a_click_outside_kudzu_interrupts() {
    let then: [fn(&mut Interruption); 2] = [|f| f.key_down(9, A, false), |f| f.click(false)];
    for then in then {
        let mut form = key();
        form.submit(submission(2, None, 1));
        then(&mut form);
        assert!(form.interrupted());
    }
    // The second click of a double-click on Submit lands on Kudzu's panel.
    let mut form = key();
    form.submit(submission(2, None, 1));
    form.click(true);
    assert!(!form.interrupted());
}

#[test]
fn an_interruption_is_final() {
    let mut form = key();
    form.click(false);
    form.focus(report(true, 2, 2));
    form.click(true);
    form.submit(submission(3, None, 2));
    assert!(form.interrupted());
    assert!(!form.may_delete());
}
