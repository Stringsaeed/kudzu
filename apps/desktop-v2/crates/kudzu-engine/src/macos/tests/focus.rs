//! FRM-4 with the desktop's panels modelled (`panels.rs`): where each key and click lands, and
//! the form panel's focus reports, on time or late. `forms.rs` drives the engine's focus input
//! directly; these tests drive the user and let the panel report.
//!
//! The engine only learns where focus is from the form panel. Another Kudzu panel looks to it
//! like the target app, so typing there interrupts a typed form, conservatively.

use kudzu_core::activity::{ActivityOutcome, ActivitySource};

use super::desk::{editor, Ev, Out};
use super::panels::Focus;
use super::rig::{backspaces, ms, snippet, Rig};

/// A typed form for `;addr` on its panel, which is key. The `forms` service holds a
/// suppression guard while it is open.
fn opened(late: bool) -> (Rig, u64) {
    let rig = Rig::new(&[snippet(";addr", "Dear {{field name=who}}, hi")]);
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
