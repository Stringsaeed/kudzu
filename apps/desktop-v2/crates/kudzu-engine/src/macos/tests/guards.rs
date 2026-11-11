//! When typed triggers must not fire (EXP-15, EXP-16): Secure Input, pause, Kudzu's own
//! windows, suppression. Nothing is swallowed or posted; a full match is noted in Activity
//! with the reason, except in Kudzu's own windows and while suppressed. And the explicit
//! Quick Search insert, which brings the target forward and inserts at its caret.

use kudzu_core::activity::ActivityOutcome;

use super::desk::{app, editor, own_pid, Ev, Out};
use super::rig::{ms, snippet, Rig};

fn sig() -> Vec<kudzu_core::SnippetRecord> {
    vec![snippet(";sig", "Best, Sam")]
}

#[test]
fn secure_input_blocks_typed_triggers_and_names_its_owner() {
    let rig = Rig::with(&sig(), |d| {
        d.set_secure_input(true, super::desk::mail().pid)
    });
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::SecureInput {
            owner: Some("Mail".into())
        }]
    );
    assert!(!rig.host.last_status().can_expand);
}

#[test]
fn secure_input_turning_on_is_noticed_by_the_poll() {
    let rig = Rig::new(&sig());
    rig.set_secure_input(true, None);
    rig.refresh();
    let status = rig.host.last_status();
    assert!(status.secure_input && !status.can_expand);
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::SecureInput { owner: None }]
    );
}

#[test]
fn pause_blocks_typed_triggers() {
    let rig = Rig::new(&sig());
    rig.engine.set_paused(true);
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.host.outcomes(), [ActivityOutcome::Paused]);
}

#[test]
fn typed_triggers_never_fire_in_kudzus_own_windows() {
    let own = app("sh.saeed.kudzu", "Kudzu", own_pid());
    let rig = Rig::with(&sig(), |d| {
        d.with(|s| s.running.push(own.clone()));
        d.set_frontmost(Some(own.clone()));
    });
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.host.outcomes(), []);
}

#[test]
fn suppression_stops_matching() {
    let rig = Rig::new(&sig());
    rig.engine.set_suppressed(true);
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.host.outcomes(), []);
    // A trigger typed while suppressed doesn't fire when suppression ends either.
    rig.engine.set_suppressed(false);
    rig.type_("x");
    assert_eq!(rig.doc(), ";sigx|");
}

#[test]
fn a_quick_search_insert_brings_the_target_forward_and_inserts_at_its_caret() {
    let rig = Rig::with(&sig(), |d| d.set_frontmost(Some(super::desk::mail())));
    rig.set_doc("Hello |world");
    let id = rig.snippet_id(";sig");
    rig.engine.insert_snippet(id, Some(editor()));
    rig.settle();
    assert_eq!(
        rig.log(),
        [
            Ev::Activated(editor().bundle_id),
            Ev::Slept(ms(50)),
            Ev::Out(Out::Type("Best, Sam".into())),
        ]
    );
    assert_eq!(rig.doc(), "Hello Best, Sam|world");
    assert_eq!(rig.host.expansions(), 1);
}

#[test]
fn a_quick_search_insert_into_a_vanished_target_does_nothing() {
    let rig = Rig::with(&sig(), |d| d.with(|s| s.activation_works = false));
    let id = rig.snippet_id(";sig");
    rig.engine.insert_snippet(id, Some(editor()));
    rig.settle();
    assert_eq!(rig.log(), [Ev::Activated(editor().bundle_id)]);
    assert_eq!(rig.doc(), "|");
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: kudzu_core::activity::InsertFailure::TargetGone
        }]
    );
}

/// The tap reads the front app from the poll's cache (third review). A trigger typed in another
/// app before the poll notices the switch is credited to the old app, and the worker, which
/// checks the live front app, inserts nothing into the new one. After a poll tick it expands.
#[test]
fn a_trigger_typed_before_the_poll_notices_an_app_switch_inserts_nothing() {
    let rig = Rig::new(&sig());
    rig.set_frontmost(Some(super::desk::mail()));
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.host.expansions(), 0);
    rig.poll_tick();
    rig.set_doc("");
    rig.type_(";sig");
    assert_eq!(rig.host.expansions(), 1);
}
