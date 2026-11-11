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
