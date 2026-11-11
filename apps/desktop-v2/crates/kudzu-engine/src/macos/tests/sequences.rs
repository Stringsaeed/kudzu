//! EXP-15, sequences. A snippet with `{{key}}` or `{{delay}}` inserts text, key presses and
//! pauses in template order. Pauses sleep in 50 ms slices and a key press settles for 30 ms.
//! Between steps the engine checks that nothing changed; on a click, an app change, pause,
//! Secure Input, lost posting access or a stale generation it stops, and leaves what it
//! already inserted alone.

use kudzu_core::activity::{ActivityOutcome, InsertFailure, Permission};

use super::desk::{mail, At, Desk, Ev, Out};
use super::rig::{ms, snippet, Rig};

fn sequence() -> Rig {
    Rig::new(&[snippet(";s", "one{{delay ms=500}}two{{key name=tab}}three")])
}

#[test]
fn a_sequence_inserts_text_keys_and_pauses_in_order() {
    let rig = sequence();
    rig.type_(";s");
    let pause = vec![Ev::Slept(ms(50)); 10];
    let expected = [
        vec![Ev::Out(Out::Backspace), Ev::Out(Out::Type("one".into()))],
        pause,
        vec![
            Ev::Out(Out::Type("two".into())),
            Ev::Out(Out::Tab),
            Ev::Slept(ms(30)),
            Ev::Out(Out::Type("three".into())),
        ],
    ]
    .concat();
    assert_eq!(rig.log(), expected);
    assert_eq!(rig.doc(), "onetwo\tthree|");
    assert_eq!(rig.host.expansions(), 1);
    // A sequence with key presses can't be undone with Backspace: it is an ordinary Backspace.
    rig.clear_log();
    rig.type_("⌫");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), "onetwo\tthre|");
}

/// The sequence is stopped by `cause`, which happens during the pause after "one".
fn stopped_by(cause: impl FnOnce(&Desk) + Send + 'static) -> Rig {
    let rig = sequence();
    rig.at(At::Sleep(ms(50), 0), cause);
    rig.type_(";s");
    assert_eq!(rig.posted(), [Out::Backspace, Out::Type("one".into())]);
    assert_eq!(rig.doc(), "one|");
    assert_eq!(rig.host.expansions(), 0);
    rig
}

#[test]
fn a_click_stops_a_sequence() {
    let rig = stopped_by(|d| d.clicks());
    assert_eq!(rig.host.outcomes(), [ActivityOutcome::Interrupted]);
    // A click is a cancellation: the pause ends at once.
    let slices = rig
        .log()
        .iter()
        .filter(|e| **e == Ev::Slept(ms(50)))
        .count();
    assert_eq!(slices, 1);
}

#[test]
fn an_app_change_stops_a_sequence() {
    let rig = stopped_by(|d| d.set_frontmost(Some(mail())));
    assert_eq!(rig.host.outcomes(), [ActivityOutcome::Interrupted]);
}

#[test]
fn pausing_stops_a_sequence() {
    let rig = stopped_by(|d| d.engine().set_paused(true));
    assert_eq!(rig.host.outcomes(), [ActivityOutcome::Paused]);
}

#[test]
fn secure_input_stops_a_sequence() {
    let rig = stopped_by(|d| {
        d.set_secure_input(true, mail().pid);
        d.refresh();
    });
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::SecureInput {
            owner: Some("Mail".into())
        }]
    );
}

#[test]
fn losing_posting_access_stops_a_sequence() {
    let rig = stopped_by(|d| {
        d.set_post_event(false);
        d.refresh();
    });
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::MissingPermission {
            permission: Permission::Accessibility
        }]
    );
}

/// Before the poll notices, the next press itself is refused, and the sequence ends there.
#[test]
fn a_refused_press_stops_a_sequence() {
    let rig = stopped_by(|d| d.set_post_event(false));
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::InsertionFailed {
            error: InsertFailure::NotAllowed
        }]
    );
}

/// The generation moves on (here macOS disabled the tap) while nothing else changed.
#[test]
fn a_stale_generation_stops_a_sequence() {
    let rig = stopped_by(|d| d.tap_times_out());
    assert!(rig.host.outcomes().contains(&ActivityOutcome::Interrupted));
}

/// A key press between two texts gives focus a chance to move (Tab to the next field), and the
/// sequence also stops when it is cut short after a key press.
#[test]
fn a_sequence_stops_after_a_key_press_too() {
    let rig = sequence();
    rig.at(At::Sleep(ms(30), 0), |d| d.clicks_text(0));
    rig.type_(";s");
    assert_eq!(
        rig.posted(),
        [
            Out::Backspace,
            Out::Type("one".into()),
            Out::Type("two".into()),
            Out::Tab
        ]
    );
    assert_eq!(rig.doc(), "|onetwo\t");
}
