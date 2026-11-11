//! EXP-14, held input. With an active listener, physical keys typed during an insertion are
//! withheld and posted again afterwards in order (key-ups too), so they land after the snippet
//! rather than inside the text being replaced. Replayed keys are matched like typing; the
//! engine's own events never are. Held keys are let go at once, unmatched, on cancellation,
//! tap loss, shutdown and the hold deadline.

use kudzu_core::activity::{ActivityOutcome, TapDisabledCause};

use super::desk::{At, Key, Out};
use super::rig::{backspaces, ms, pasted, snippet, Rig};
use crate::macos::pasteboard::RESTORE_DELAY;
use crate::TapMode;

fn replay(label: &str, down: bool) -> Out {
    Out::Replay(Key {
        label: label.into(),
        down,
    })
}

fn release(label: &str, down: bool) -> Out {
    Out::Release(Key {
        label: label.into(),
        down,
    })
}

/// Down and up of each key, replayed.
fn replayed(keys: &str) -> Vec<Out> {
    keys.chars()
        .flat_map(|c| [replay(&c.to_string(), true), replay(&c.to_string(), false)])
        .collect()
}

fn released(keys: &str) -> Vec<Out> {
    keys.chars()
        .flat_map(|c| {
            [
                release(&c.to_string(), true),
                release(&c.to_string(), false),
            ]
        })
        .collect()
}

fn sig() -> Rig {
    Rig::new(&[snippet(";sig", "Best, Sam")])
}

#[test]
fn keys_typed_during_an_insertion_replay_in_order_with_their_key_ups() {
    let rig = sig();
    // The user keeps typing while the trigger is being deleted.
    rig.at(At::Press(0), |d| d.types("x"));
    rig.at(At::Press(1), |d| d.types("y"));
    rig.type_(";sig");
    let expected = [
        backspaces(3),
        vec![Out::Type("Best, Sam".into())],
        replayed("xy"),
    ]
    .concat();
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.doc(), "Best, Samxy|");
    assert_eq!(rig.host.expansions(), 1);
}

#[test]
fn keys_typed_during_replay_queue_behind_the_rest() {
    let rig = sig();
    rig.at(At::Press(0), |d| d.types("xy"));
    rig.at(At::Held(0), |d| d.types("z"));
    rig.type_(";sig");
    let expected = [
        backspaces(3),
        vec![Out::Type("Best, Sam".into())],
        replayed("xyz"),
    ]
    .concat();
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.doc(), "Best, Samxyz|");
}

#[test]
fn a_trigger_completed_by_replay_expands() {
    let rig = Rig::new(&[snippet(";a", "Alpha"), snippet(";b", "Beta")]);
    rig.at(At::Press(0), |d| d.types(";b"));
    rig.type_(";a");
    let expected = vec![
        Out::Backspace,
        Out::Type("Alpha".into()),
        replay(";", true),
        replay(";", false),
        // The replayed "b" completes ";b": it is swallowed and the second expansion runs
        // before the rest of the queue.
        replay("b", true),
        Out::Backspace,
        Out::Type("Beta".into()),
        replay("b", false),
    ];
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.doc(), "AlphaBeta|");
    assert_eq!(rig.host.expansions(), 2);
}

#[test]
fn the_engines_own_events_are_never_matched() {
    let rig = Rig::new(&[snippet(";a", "say ;b"), snippet(";b", "Beta")]);
    rig.type_(";a");
    assert_eq!(rig.posted(), [Out::Backspace, Out::Type("say ;b".into())]);
    assert_eq!(rig.doc(), "say ;b|");
    assert_eq!(rig.host.expansions(), 1);
}

/// A sequence with a pause: the user types during it, then something cancels the insertion.
/// The held keys go to the document at once, unmatched, and the sequence stops.
fn cancelled_during_a_pause(cancel: impl FnOnce(&super::desk::Desk) + Send + 'static) -> Rig {
    let rig = Rig::new(&[snippet(";p", "one{{delay ms=500}}two")]);
    rig.at(At::Sleep(ms(50), 0), move |d| {
        d.types("x");
        cancel(d);
    });
    rig.type_(";p");
    assert_eq!(
        rig.posted(),
        [vec![Out::Backspace, Out::Type("one".into())], released("x")].concat()
    );
    assert_eq!(rig.doc(), "onex|");
    assert_eq!(rig.host.expansions(), 0);
    rig
}

#[test]
fn a_click_releases_held_input() {
    let rig = cancelled_during_a_pause(|d| d.clicks());
    assert!(rig.host.outcomes().contains(&ActivityOutcome::Interrupted));
}

#[test]
fn pausing_releases_held_input() {
    let rig = cancelled_during_a_pause(|d| d.engine().set_paused(true));
    assert!(rig.host.outcomes().contains(&ActivityOutcome::Paused));
}

#[test]
fn losing_the_tap_releases_held_input() {
    let rig = cancelled_during_a_pause(|d| d.tap_times_out());
    let outcomes = rig.host.outcomes();
    assert!(outcomes.contains(&ActivityOutcome::TapDisabled {
        cause: TapDisabledCause::Timeout,
        reenabled: true
    }));
    assert!(outcomes.contains(&ActivityOutcome::Interrupted));
}

#[test]
fn losing_the_listener_releases_held_input() {
    let rig = cancelled_during_a_pause(|d| {
        d.grant(false, false);
        d.refresh();
    });
    assert!(rig.host.outcomes().contains(&ActivityOutcome::ListenerLost));
    assert_eq!(rig.host.last_status().tap, TapMode::Off);
}

#[test]
fn shutdown_releases_held_input() {
    let rig = cancelled_during_a_pause(|d| d.engine().stop());
    assert!(rig
        .host
        .outcomes()
        .contains(&ActivityOutcome::EngineStopped));
    assert_eq!(rig.host.last_status().tap, TapMode::Off);
}

/// Physical input is never held longer than 1 s, whatever the target does. Here the paste's
/// wait runs long: the held key goes out unmatched, after the pasted text, and the insertion
/// finishes nothing more. It is not counted as an expansion (current behaviour).
#[test]
fn the_hold_deadline_releases_held_input() {
    let rig = Rig::new(&[pasted(";sig", "Best, Sam")]);
    rig.at(At::Sleep(RESTORE_DELAY, 0), |d| {
        d.types("x");
        d.advance(ms(1_001));
        d.poll_tick();
    });
    rig.type_(";sig");
    let expected = [backspaces(3), vec![Out::Paste], released("x")].concat();
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.doc(), "Best, Samx|");
    assert_eq!(rig.host.expansions(), 0);
}

#[test]
fn the_deadline_is_one_second_per_step_and_extended_by_delays() {
    // Each step may take almost a second, and a 1.5 s delay may hold input for 2.5 s.
    let rig = Rig::new(&[snippet(";p", "a{{key name=tab}}b{{delay ms=1500}}c")]);
    rig.at(At::Press(1), |d| {
        d.types("x");
        d.advance(ms(900));
        d.poll_tick();
    });
    rig.at(At::Sleep(ms(30), 0), |d| {
        d.advance(ms(900));
        d.poll_tick();
    });
    rig.at(At::Sleep(ms(50), 0), |d| {
        d.advance(ms(2_400));
        d.poll_tick();
    });
    rig.type_(";p");
    let expected = [
        vec![
            Out::Backspace,
            Out::Type("a".into()),
            Out::Tab,
            Out::Type("b".into()),
            Out::Type("c".into()),
        ],
        replayed("x"),
    ]
    .concat();
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.doc(), "a\tbcx|");
    assert_eq!(rig.host.expansions(), 1);
}
