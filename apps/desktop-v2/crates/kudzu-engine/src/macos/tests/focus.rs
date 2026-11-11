//! FRM-4 with the desktop's panels modelled (`panels.rs`): where each key and click lands, and
//! the form panel's focus reports, on time or late. `forms.rs` drives the engine's focus input
//! directly; these tests drive the user and let the panel report.
//!
//! A click is judged by the window it lands on, which the listener sees with the event: only
//! Kudzu's own windows leave a typed form alone. Where the keyboard is, the engine only learns
//! from the form panel; another Kudzu panel looks to it like the target app, so typing there
//! interrupts a typed form, conservatively.

use kudzu_core::activity::{ActivityOutcome, ActivitySource};

use super::desk::{editor, windows, At, Desk, Ev, Out};
use super::panels::Focus;
use super::rig::{backspaces, ms, snippet, Rig};

/// A typed form for `;addr` on its panel, which is key. The `forms` service holds a
/// suppression guard while it is open.
fn opened(late: bool) -> (Rig, u64) {
    opened_with(late, |_| {})
}

fn opened_with(late: bool, setup: impl FnOnce(&Desk)) -> (Rig, u64) {
    let rig = Rig::with(&[snippet(";addr", "Dear {{field name=who}}, hi")], setup);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.with(|s| s.panels.late = late);
    rig.show_form(token);
    rig.deliver_reports();
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
    let activity = rig.host.activity.lock().unwrap();
    let forms = activity
        .iter()
        .filter(|e| e.source == Some(ActivitySource::Form));
    forms.map(|e| e.outcome.clone()).collect()
}

/// The panel is key: typing goes into the form, and the trigger is replaced.
#[test]
fn typing_in_the_form_submits() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
    assert_eq!(rig.doc(), "Dear Sam, hi|");
}

/// Typing in another Kudzu panel doesn't touch the document, but the engine can't tell that
/// panel from the target app: the form is interrupted and its submission deletes nothing.
#[test]
fn typing_in_another_kudzu_panel_then_returning_interrupts_the_form() {
    let (rig, token) = opened(false);
    rig.focus_moves(Focus::Panel);
    rig.press("abc");
    rig.click_on(Focus::Form, None);
    rig.press("Sam");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addr|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

// MARK: Clicks

/// The user clicks into the target while the form is key, which moves the caret, then clicks
/// back on the form. The panel's reports arrive late, so the panel still counted as key; the
/// click in the target interrupts the form all the same.
#[test]
fn a_click_in_the_target_then_back_on_the_form_interrupts_it() {
    let (rig, token) = opened(true);
    rig.click_on(Focus::Target, Some(2));
    rig.click_on(Focus::Form, None);
    rig.deliver_reports();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The same, coming back without a click (the form window makes its panel key again).
#[test]
fn a_click_in_the_target_then_focus_back_on_the_form_interrupts_it() {
    let (rig, token) = opened(false);
    rig.click_on(Focus::Target, Some(2));
    rig.focus_moves(Focus::Form);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Clicks inside the key panel (into a field, on a menu) stay there.
#[test]
fn clicks_inside_the_form_do_not_interrupt_it() {
    let (rig, token) = opened(true);
    rig.click_on(Focus::Form, None);
    rig.press("Sa");
    rig.click_on(Focus::Form, None);
    rig.press("m");
    rig.deliver_reports();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
    assert_eq!(rig.doc(), "Dear Sam, hi|");
}

/// Focus left the form without a click (a shortcut opened Quick Search, nothing typed); the
/// click that makes the form key again is on the form, not in the target.
#[test]
fn the_click_that_makes_the_form_key_does_not_interrupt_it() {
    let (rig, token) = opened(false);
    rig.focus_moves(Focus::Panel);
    rig.click_on(Focus::Form, None);
    rig.press("Sam");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
    assert_eq!(rig.doc(), "Dear Sam, hi|");
}

/// The same with the panel's reports arriving late.
#[test]
fn a_late_report_claims_the_click_that_made_the_form_key() {
    let (rig, token) = opened(true);
    rig.focus_moves(Focus::Panel);
    rig.deliver_reports();
    rig.click_on(Focus::Form, None);
    rig.deliver_reports();
    rig.press("Sam");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
}

/// Conservative: typing before the late report says the panel is key again counts as typing
/// elsewhere.
#[test]
fn typing_before_a_late_report_claims_the_click_interrupts() {
    let (rig, token) = opened(true);
    rig.focus_moves(Focus::Panel);
    rig.deliver_reports();
    rig.click_on(Focus::Form, None);
    rig.press("S");
    rig.deliver_reports();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// A click in the target before the form is shown.
#[test]
fn a_click_before_the_form_is_shown_interrupts_it() {
    let rig = Rig::new(&[snippet(";addr", "Dear {{field name=who}}, hi")]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.click_on(Focus::Target, Some(2));
    rig.show_form(token);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Review blocker 1: the form loses key with no input, the user clicks elsewhere in the target
/// (already in front, so no focus report follows), and the form is brought back without a
/// click. The click moved the caret; the later focus gain must not forgive it.
#[test]
fn a_target_click_with_no_focus_change_then_a_programmatic_refocus_interrupts() {
    let (rig, token) = opened(false);
    rig.focus_moves(Focus::Target);
    rig.click_on(Focus::Target, Some(2));
    rig.focus_moves(Focus::Form);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// A click in another app's window doesn't land in Kudzu: the form is interrupted.
#[test]
fn a_click_in_another_app_interrupts_the_form() {
    let (rig, token) = opened(false);
    rig.click_in(windows::OTHER);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// When the event doesn't name the window under the pointer, the click is not known to be in
/// Kudzu: the form is interrupted.
#[test]
fn a_click_whose_window_is_unknown_interrupts_the_form() {
    let (rig, token) = opened(false);
    rig.click_in(0);
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Clicking into another Kudzu panel and back, typing only in the form, leaves it alone.
#[test]
fn clicks_in_another_kudzu_panel_and_back_do_not_interrupt_the_form() {
    let (rig, token) = opened(true);
    rig.click_on(Focus::Panel, None);
    rig.deliver_reports();
    rig.click_on(Focus::Form, None);
    rig.deliver_reports();
    rig.press("Sam");
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
}

// MARK: Submission

/// The submitted panel goes away and the target comes forward; meanwhile the user clicks in the
/// target. Suppression is still on, so the click doesn't cancel anything else.
#[test]
fn a_click_during_activation_interrupts_the_submission() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.at(At::Activate(0), |d| d.click_on(Focus::Target, Some(2)));
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(
        rig.log(),
        [Ev::Activated(editor().bundle_id), Ev::Slept(ms(50))]
    );
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The same without suppression.
#[test]
fn a_click_during_activation_interrupts_an_unsuppressed_submission() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.engine.set_suppressed(false);
    rig.at(At::Activate(0), |d| d.click_on(Focus::Target, Some(2)));
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Nothing holds typing until the insertion starts, so a key typed during activation reaches the
/// target after the trigger.
#[test]
fn a_key_during_activation_interrupts_the_submission() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.at(At::Activate(0), |d| {
        d.focus_moves(Focus::Target);
        d.key_at_focus("z", true);
        d.key_at_focus("z", false);
    });
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// A click while the target settles, before the trigger is deleted.
#[test]
fn a_click_while_settling_interrupts_the_submission() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.at(At::Sleep(ms(50), 1), |d| d.click_on(Focus::Target, Some(2)));
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";a|ddr");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// With the active listener, typing while the target settles is held, then replayed after the
/// insertion: it never lands inside the trigger, so the submission goes ahead.
#[test]
fn a_key_while_settling_is_held_and_replayed_after_the_insertion() {
    let (rig, token) = opened(false);
    rig.press("Sam");
    rig.at(At::Sleep(ms(50), 1), |d| {
        d.focus_moves(Focus::Target);
        d.key_at_focus("z", true);
        d.key_at_focus("z", false);
    });
    rig.submit(token, &[("who", "Sam")]);
    rig.settle();
    assert_eq!(rig.doc(), "Dear Sam, hiz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Expanded]);
}

/// Listen-only can't hold that key: it reaches the target, so the submission deletes nothing.
#[test]
fn a_key_while_settling_interrupts_a_listen_only_submission() {
    use crate::TapMode;
    let (rig, token) = opened_with(false, |d| d.with(|s| s.refuse = vec![TapMode::Active]));
    rig.press("Sam");
    rig.at(At::Sleep(ms(50), 1), |d| {
        d.focus_moves(Focus::Target);
        d.key_at_focus("z", true);
        d.key_at_focus("z", false);
    });
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Review blocker 2: Return in the form is the submit event. The panel closes and the user types
/// at once; the key reaches the target before the main thread gets to `submit_form`, and the
/// panel's blur report is late. That key came after the submit event, so it interrupts.
#[test]
fn a_key_after_the_submit_event_interrupts_even_before_the_engine_hears_of_it() {
    let (rig, token) = opened(true);
    rig.press("Sam⏎");
    let submitted_by = rig.last_event();
    rig.focus_moves(Focus::Target);
    rig.press("z");
    rig.submit_by(token, &[("who", "Sam")], Some(submitted_by));
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Review blocker 2: the submission is accepted while the worker is still busy (here with a
/// Quick Search insert whose target won't come forward). A key that reaches the target before
/// the worker starts the submission interrupts it.
#[test]
fn a_key_after_the_submission_is_accepted_interrupts_before_the_worker_starts_it() {
    let rig = Rig::new(&[
        snippet(";addr", "Dear {{field name=who}}, hi"),
        snippet(";x", "other"),
    ]);
    rig.type_(";addr");
    let token = rig.host.form_token();
    rig.engine.set_suppressed(true);
    rig.show_form(token);
    rig.press("Sam");
    // The worker blocks in the Quick Search insert's activation until released.
    let (entered, busy) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let released = std::sync::Mutex::new(released);
    rig.with(|s| s.activation_works = false);
    rig.at(At::Activate(0), move |d| {
        entered.send(()).unwrap();
        released.lock().unwrap().recv().unwrap();
        d.with(|s| s.activation_works = true);
    });
    rig.engine
        .insert_snippet(rig.snippet_id(";x"), Some(super::desk::mail()));
    busy.recv().unwrap();
    let values = super::rig::form_values(&[("who", "Sam")]);
    rig.engine.submit_form(token, values, rig.submission(token));
    // The panel closes; its blur report is late.
    rig.with(|s| s.panels.late = true);
    rig.focus_moves(Focus::Target);
    rig.desk.key_at_focus("z", true);
    rig.desk.key_at_focus("z", false);
    release.send(()).unwrap();
    rig.settle();
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// The form window moves focus to the target (no click) while its main thread is busy, and the
/// user keeps typing: those keys reach the target before the blur report. The report's stamp
/// shows they came after the change, so they interrupt.
#[test]
fn a_late_blur_catches_typing_routed_to_the_target_before_it_arrived() {
    let (rig, token) = opened(true);
    rig.press("Sa");
    rig.focus_moves(Focus::Target);
    rig.press("z");
    rig.focus_moves(Focus::Form);
    rig.deliver_reports();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), []);
    assert_eq!(rig.doc(), ";addrz|");
    assert_eq!(form_outcomes(&rig), [ActivityOutcome::Interrupted]);
}

/// Typing in the form before a late blur was in the panel: the form goes ahead.
#[test]
fn typing_in_the_form_before_a_late_blur_is_not_an_interruption() {
    let (rig, token) = opened(true);
    rig.press("Sam");
    rig.focus_moves(Focus::Panel);
    rig.focus_moves(Focus::Form);
    rig.deliver_reports();
    rig.submit(token, &[("who", "Sam")]);
    assert_eq!(rig.log(), inserted());
}

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
