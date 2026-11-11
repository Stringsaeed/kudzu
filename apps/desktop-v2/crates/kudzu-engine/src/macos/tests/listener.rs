//! EXP-18, the listener. With Accessibility the engine attaches an active listener (it can
//! withhold keys); with Input Monitoring alone, or when macOS refuses the active one, a
//! listen-only one. When Accessibility arrives later it upgrades without ever running two
//! listeners at once, and it drops the listener when both permissions go.

use kudzu_core::activity::{ActivityOutcome, Permission};

use super::desk::{At, ListenerOp, Out};
use super::rig::{backspaces, ms, snippet, Rig};
use crate::TapMode;

use ListenerOp::{Created, Enabled, Refused, Stopped};
use TapMode::{Active, ListenOnly, Off};

fn sig() -> Vec<kudzu_core::SnippetRecord> {
    vec![snippet(";sig", "Best, Sam")]
}

#[test]
fn accessibility_attaches_an_active_listener() {
    let rig = Rig::unstarted(&sig(), |_| {});
    rig.engine.start();
    assert_eq!(rig.listener_log(), [Created(Active), Enabled(Active)]);
    let status = rig.host.last_status();
    assert_eq!(status.tap, Active);
    assert!(status.can_expand);
    // Polling again changes nothing.
    rig.refresh();
    assert_eq!(rig.listener_log(), [Created(Active), Enabled(Active)]);
}

#[test]
fn input_monitoring_alone_attaches_listen_only() {
    let rig = Rig::unstarted(&sig(), |d| d.grant(false, true));
    rig.engine.start();
    assert_eq!(
        rig.listener_log(),
        [Created(ListenOnly), Enabled(ListenOnly)]
    );
    let status = rig.host.last_status();
    assert_eq!(status.tap, ListenOnly);
    assert!(!status.can_expand);
    // It can see a trigger but not type: nothing is posted, the miss is noted.
    rig.clear_log();
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";sig|");
    assert_eq!(
        rig.host.outcomes(),
        [ActivityOutcome::MissingPermission {
            permission: Permission::Accessibility
        }]
    );
}

/// macOS refuses the active listener although Accessibility is granted: listen-only instead.
/// Listen-only can't withhold the last key, so it reaches the document and is deleted too.
#[test]
fn a_refused_active_listener_falls_back_to_listen_only() {
    let rig = Rig::unstarted(&sig(), |d| d.with(|s| s.refuse = vec![Active]));
    rig.engine.start();
    assert_eq!(
        rig.listener_log(),
        [Refused(Active), Created(ListenOnly), Enabled(ListenOnly)]
    );
    rig.clear_log();
    rig.type_(";sig");
    let expected = [backspaces(4), vec![Out::Type("Best, Sam".into())]].concat();
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.doc(), "Best, Sam|");
    // Each poll tries the active listener again; the listen-only one stays.
    rig.clear_log();
    rig.refresh();
    assert_eq!(rig.listener_log(), [Refused(Active)]);
}

/// Listen-only: a key typed before the Backspaces go out would land inside the range they
/// delete, so the expansion is abandoned.
#[test]
fn listen_only_abandons_an_expansion_when_typing_continues() {
    let rig = Rig::with(&sig(), |d| d.with(|s| s.refuse = vec![Active]));
    rig.at(At::Sleep(ms(30), 0), |d| d.types("x"));
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";sigx|");
    assert_eq!(rig.host.expansions(), 0);
}

#[test]
fn granting_accessibility_upgrades_without_a_second_listener() {
    let rig = Rig::unstarted(&sig(), |d| d.grant(false, true));
    rig.engine.start();
    rig.grant(true, true);
    rig.refresh();
    // The new listener is created disabled, the old one stops, then the new one starts.
    assert_eq!(
        rig.listener_log(),
        [
            Created(ListenOnly),
            Enabled(ListenOnly),
            Created(Active),
            Stopped(ListenOnly),
            Enabled(Active)
        ]
    );
    assert_eq!(rig.with(|s| s.peak_enabled), 1);
    assert_eq!(rig.with(|s| s.listeners.len()), 1);
    assert_eq!(rig.host.last_status().tap, Active);
    // One listener: a trigger expands once.
    rig.clear_log();
    rig.type_(";sig");
    let expected = [backspaces(3), vec![Out::Type("Best, Sam".into())]].concat();
    assert_eq!(rig.posted(), expected);
    assert_eq!(rig.host.expansions(), 1);
}

#[test]
fn losing_both_permissions_drops_the_listener() {
    let rig = Rig::new(&sig());
    rig.grant(false, false);
    rig.refresh();
    assert_eq!(rig.listener_log(), [Stopped(Active)]);
    assert_eq!(rig.host.last_status().tap, Off);
    assert!(rig.host.outcomes().contains(&ActivityOutcome::ListenerLost));
    // Nothing listens: typing just reaches the document.
    rig.clear_log();
    rig.type_(";sig");
    assert_eq!(rig.posted(), []);
    assert_eq!(rig.doc(), ";sig|");
    // Permission comes back: the listener is attached again.
    rig.grant(true, true);
    rig.refresh();
    assert_eq!(rig.listener_log(), [Created(Active), Enabled(Active)]);
}
