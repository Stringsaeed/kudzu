//! RUNE-GAP(#574): delete these guide scheduling fixtures with the native guide.
use super::*;

fn companion() -> Rect {
    Rect {
        x: 392.,
        y: 354.,
        width: 420.,
        height: 660.,
    }
}
fn settings() -> Rect {
    Rect {
        x: 830.,
        y: 162.,
        width: 740.,
        height: 852.,
    }
}
fn observation() -> Observation {
    Observation {
        visible: true,
        front_app: FrontApp::ThisApp,
        dragging: false,
        granted: false,
        reduce_motion: false,
        settings: Some(settings()),
        companion: companion(),
    }
}

fn begin(guide: &mut Lifecycle, now: u64) -> Generation {
    guide.begin(now, Some(companion()))
}

#[test]
fn delay_then_first_try_and_twelve_retries_have_exact_deadlines() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 1000);
    let mut input = observation();
    input.settings = None;
    assert_eq!(guide.poll(generation, 1449, input), Step::WaitUntil(1450));
    for attempt in 0..12 {
        let now = 1450 + attempt * 200;
        assert_eq!(
            guide.poll(generation, now, input),
            Step::WaitUntil(now + 200)
        );
    }
    assert_eq!(guide.poll(generation, 3850, input), Step::Stop);
}

#[test]
fn a_stale_generation_cannot_start_or_stop_a_new_guide() {
    let mut guide = Lifecycle::default();
    let old = begin(&mut guide, 0);
    let new = begin(&mut guide, 10);
    assert_ne!(old, new);
    assert_eq!(guide.poll(old, 450, observation()), Step::Stale);
    assert_eq!(guide.poll(new, 460, observation()), Step::Ready(settings()));
    assert!(!guide.started(old, 460, settings(), companion()));
    assert!(guide.started(new, 460, settings(), companion()));
    assert_eq!(guide.poll(new, 1660, observation()), Step::Draw(0.5));
    assert_eq!(guide.poll(new, 5260, observation()), Step::Draw(0.0));
    assert_eq!(guide.poll(new, 5261, observation()), Step::Stop);
}

#[test]
fn every_cancellation_invalidates_pending_ticks() {
    for reason in 0..8 {
        let mut guide = Lifecycle::default();
        let generation = begin(&mut guide, 0);
        assert_eq!(
            guide.poll(generation, 450, observation()),
            Step::Ready(settings())
        );
        assert!(guide.started(generation, 450, settings(), companion()));
        let mut input = observation();
        match reason {
            0 => input.dragging = true,
            1 => input.granted = true,
            2 => input.settings.as_mut().unwrap().x += 1.,
            3 => input.companion.y += 1.,
            4 => input.front_app = FrontApp::Other,
            5 => input.visible = false,
            6 => input.reduce_motion = true,
            _ => input.settings = None,
        }
        assert_eq!(
            guide.poll(generation, 500, input),
            Step::Stop,
            "reason {reason}"
        );
        assert_eq!(guide.poll(generation, 501, observation()), Step::Stale);
    }
}

#[test]
fn overlapping_windows_do_not_start_a_ghost() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    guide.poll(generation, 450, observation());
    assert!(!guide.started(generation, 450, settings(), settings()));
    assert_eq!(guide.poll(generation, 500, observation()), Step::Stale);
}

#[test]
fn reduce_motion_still_reaches_ready_for_companion_placement() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    let mut input = observation();
    input.reduce_motion = true;
    assert_eq!(guide.poll(generation, 449, input), Step::WaitUntil(450));
    assert_eq!(guide.poll(generation, 450, input), Step::Ready(settings()));
    assert!(guide.started(generation, 450, settings(), companion()));
    assert_eq!(guide.poll(generation, 451, input), Step::Stop);
}

#[test]
fn waiting_still_reaches_ready_when_only_the_grant_flag_is_set() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    let mut input = observation();
    input.granted = true;
    assert_eq!(guide.poll(generation, 450, input), Step::Ready(settings()));
}

#[test]
fn switching_apps_during_the_wait_places_nothing() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    let mut away = observation();
    away.settings = None;
    away.front_app = FrontApp::Other;
    assert_eq!(guide.poll(generation, 100, away), Step::Stop);
    assert_eq!(guide.poll(generation, 450, observation()), Step::Stale);

    // System Settings came forward, then the user returned to this app.
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    let mut settings_front = observation();
    settings_front.settings = None;
    settings_front.front_app = FrontApp::SystemSettings;
    assert_eq!(
        guide.poll(generation, 100, settings_front),
        Step::WaitUntil(450)
    );
    let mut back = settings_front;
    back.front_app = FrontApp::ThisApp;
    back.settings = Some(settings());
    assert_eq!(guide.poll(generation, 450, back), Step::Stop);
    assert_eq!(guide.poll(generation, 451, observation()), Step::Stale);
}

#[test]
fn moving_the_companion_during_the_wait_places_nothing() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    let mut moved = observation();
    moved.companion.x += 20.;
    moved.settings = Some(settings());
    assert_eq!(guide.poll(generation, 450, moved), Step::Stop);
    assert_eq!(guide.poll(generation, 451, observation()), Step::Stale);
}

#[test]
fn system_settings_coming_forward_during_the_wait_still_places() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    let mut waiting = observation();
    waiting.settings = None;
    assert_eq!(guide.poll(generation, 100, waiting), Step::WaitUntil(450));
    let mut forward = waiting;
    forward.front_app = FrontApp::SystemSettings;
    forward.settings = Some(settings());
    assert_eq!(
        guide.poll(generation, 450, forward),
        Step::Ready(settings())
    );
}

#[test]
fn hidden_or_dragging_companions_cancel_while_waiting_and_ready() {
    for ready in [false, true] {
        for dragging in [false, true] {
            let mut guide = Lifecycle::default();
            let generation = begin(&mut guide, 0);
            if ready {
                assert_eq!(
                    guide.poll(generation, 450, observation()),
                    Step::Ready(settings())
                );
            }
            let mut input = observation();
            if dragging {
                input.dragging = true;
            } else {
                input.visible = false;
            }
            assert_eq!(
                guide.poll(generation, if ready { 451 } else { 100 }, input),
                Step::Stop
            );
            assert_eq!(guide.poll(generation, 452, observation()), Step::Stale);
        }
    }
}

#[test]
fn ready_waits_without_requesting_a_wake_and_expires_after_two_hundred_ms() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    assert_eq!(
        guide.poll(generation, 450, observation()),
        Step::Ready(settings())
    );
    let mut input = observation();
    input.reduce_motion = true;
    input.front_app = FrontApp::Other;
    input.granted = true;
    input.companion.x += 20.;
    // No ghost-only observation can prevent placement or request another immediate poll.
    assert!(matches!(guide.poll(generation, 451, input), Step::Placing));
    assert!(matches!(guide.poll(generation, 649, input), Step::Placing));
    assert_eq!(guide.poll(generation, 650, input), Step::Stop);
    assert!(!guide.is_current(generation));
}

#[test]
fn started_rejects_late_placement_even_without_an_intervening_poll() {
    let mut guide = Lifecycle::default();
    let generation = begin(&mut guide, 0);
    guide.poll(generation, 450, observation());
    assert!(!guide.started(generation, 650, settings(), companion()));
    assert_eq!(guide.poll(generation, 651, observation()), Step::Stale);
}

#[test]
fn replacing_a_ready_generation_cancels_its_pending_placement() {
    let mut guide = Lifecycle::default();
    let old = begin(&mut guide, 0);
    guide.poll(old, 450, observation());
    let new = begin(&mut guide, 460);
    assert!(!guide.started(old, 461, settings(), companion()));
    assert_eq!(guide.poll(old, 462, observation()), Step::Stale);
    assert_eq!(guide.poll(new, 462, observation()), Step::WaitUntil(910));
}
