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
        relevant_front_app: true,
        dragging: false,
        granted: false,
        reduce_motion: false,
        settings: Some(settings()),
        companion: companion(),
    }
}

#[test]
fn delay_then_first_try_and_twelve_retries_have_exact_deadlines() {
    let mut guide = Lifecycle::default();
    let generation = guide.begin(1000, companion());
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
    let old = guide.begin(0, companion());
    let new = guide.begin(10, companion());
    assert_ne!(old, new);
    assert_eq!(guide.poll(old, 450, observation()), Step::Stale);
    assert_eq!(guide.poll(new, 460, observation()), Step::Ready(settings()));
    assert!(!guide.started(old, 460, settings(), companion()));
    assert!(guide.started(new, 460, settings(), companion()));
    assert_eq!(guide.poll(new, 1660, observation()), Step::Draw(0.5));
    assert_eq!(guide.poll(new, 5260, observation()), Step::Stop);
}

#[test]
fn every_cancellation_invalidates_pending_ticks() {
    for reason in 0..8 {
        let mut guide = Lifecycle::default();
        let generation = guide.begin(0, companion());
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
            4 => input.relevant_front_app = false,
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
    let generation = guide.begin(0, companion());
    guide.poll(generation, 450, observation());
    assert!(!guide.started(generation, 450, settings(), settings()));
    assert_eq!(guide.poll(generation, 500, observation()), Step::Stale);
}
