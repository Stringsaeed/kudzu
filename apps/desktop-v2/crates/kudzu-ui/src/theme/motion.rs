//! Motion tokens from `globals.css:116` and `:160`: three speeds, the two spring curves, and the
//! app's named keyframe sequences.
//!
//! Each spring curve exists twice. As an [`Easing::Piecewise`] it is the CSS `linear()` list
//! itself, for timed tweens and `Beat::easing`. As a [`Spring`] it is fitted to those samples,
//! for interruptible layout and presence motion; the fit is checked to stay within 0.02 of every
//! sample.

use rune::{Beat, Curve, Easing, Spring};

/// `--motion-fast`, `--motion-moderate`, `--motion-slow`, in seconds. The bigger the thing that
/// moves, the slower it goes.
pub const FAST: f32 = 0.080;
pub const MODERATE: f32 = 0.160;
pub const SLOW: f32 = 0.240;

/// `--spring-soft` (about 1.5% overshoot): things that travel. 17 evenly spaced samples.
pub static SOFT_STOPS: [[f32; 2]; 17] = [
    [0.0, 0.0],
    [0.0625, 0.053],
    [0.125, 0.175],
    [0.1875, 0.324],
    [0.25, 0.473],
    [0.3125, 0.609],
    [0.375, 0.723],
    [0.4375, 0.814],
    [0.5, 0.883],
    [0.5625, 0.934],
    [0.625, 0.968],
    [0.6875, 0.991],
    [0.75, 1.005],
    [0.8125, 1.012],
    [0.875, 1.015],
    [0.9375, 1.015],
    [1.0, 1.0],
];

/// `--spring-pop` (about 10% overshoot): things that land. 21 evenly spaced samples.
pub static POP_STOPS: [[f32; 2]; 21] = [
    [0.0, 0.0],
    [0.05, 0.067],
    [0.1, 0.225],
    [0.15, 0.422],
    [0.2, 0.618],
    [0.25, 0.789],
    [0.3, 0.923],
    [0.35, 1.018],
    [0.4, 1.075],
    [0.45, 1.102],
    [0.5, 1.106],
    [0.55, 1.096],
    [0.6, 1.077],
    [0.65, 1.056],
    [0.7, 1.035],
    [0.75, 1.018],
    [0.8, 1.005],
    [0.85, 0.996],
    [0.9, 0.991],
    [0.95, 0.989],
    [1.0, 1.0],
];

pub const SPRING_SOFT: Easing = Easing::Piecewise(&SOFT_STOPS);
pub const SPRING_POP: Easing = Easing::Piecewise(&POP_STOPS);

/// The fitted springs' perceptual duration as a share of the tween they stand in for, and their
/// bounce: a minimax fit to the CSS samples (worst error about 0.009).
const SOFT_RATIO: f32 = 1.0596;
const SOFT_BOUNCE: f32 = 0.1682;
const POP_RATIO: f32 = 0.7795;
const POP_BOUNCE: f32 = 0.4046;

/// The soft curve as a spring, for motion that would run `duration` seconds as a tween.
pub const fn soft_spring(duration: f32) -> Spring {
    Spring::new(duration * SOFT_RATIO, SOFT_BOUNCE)
}

/// The pop curve as a spring, for motion that would run `duration` seconds as a tween.
pub const fn pop_spring(duration: f32) -> Spring {
    Spring::new(duration * POP_RATIO, POP_BOUNCE)
}

/// The springs at `SLOW`, where the CSS uses them.
pub const SOFT: Spring = soft_spring(SLOW);
pub const POP: Spring = pop_spring(SLOW);

/// `transition: … var(--motion-*) var(--spring-*)` as a Rune curve.
pub const fn soft(duration: f32) -> Curve {
    Curve::tween(duration, SPRING_SOFT)
}

pub const fn pop(duration: f32) -> Curve {
    Curve::tween(duration, SPRING_POP)
}

/// The `press` utility: sink fast and eased out, release on the pop curve (`globals.css:404`).
pub mod press {
    use super::*;

    pub const SCALE: f32 = 0.97;
    pub const SCALE_SIDEBAR: f32 = 0.985;
    pub const SCALE_RANKED_ROW: f32 = 0.99;
    /// Footer keycaps while their key is held (`key-hint`).
    pub const SCALE_KEY_HINT: f32 = 0.9;
    pub const SINK: Curve = Curve::tween(FAST, Easing::EASE_OUT);
    pub const RELEASE: Curve = pop(SLOW);
    /// Colour, background, border, shadow, opacity and filter changes.
    pub const PAINT: Curve = Curve::tween(FAST, Easing::EASE);
}

/// `.stagger`: delays for a page's children, the sixth and later sharing the last.
pub const STAGGER: [f32; 6] = [0.0, 0.030, 0.060, 0.090, 0.120, 0.150];

/// A pose in a keyframe sequence. Translations in points, rotation in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub opacity: f32,
    pub scale: [f32; 2],
    pub translate: [f32; 2],
    pub rotate: f32,
    /// How far the colour is toward `iris_foreground` (`tick`).
    pub tint: f32,
    /// Background position as a share of the element's width (`shimmer`).
    pub shift: f32,
}

impl Pose {
    pub const REST: Pose = Pose {
        opacity: 1.0,
        scale: [1.0, 1.0],
        translate: [0.0, 0.0],
        rotate: 0.0,
        tint: 0.0,
        shift: 0.0,
    };

    pub const fn opacity(self, opacity: f32) -> Self {
        Self { opacity, ..self }
    }
    pub const fn scale(self, s: f32) -> Self {
        Self {
            scale: [s, s],
            ..self
        }
    }
    pub const fn scale_x(self, x: f32) -> Self {
        Self {
            scale: [x, self.scale[1]],
            ..self
        }
    }
    pub const fn x(self, x: f32) -> Self {
        Self {
            translate: [x, self.translate[1]],
            ..self
        }
    }
    pub const fn y(self, y: f32) -> Self {
        Self {
            translate: [self.translate[0], y],
            ..self
        }
    }
    pub const fn rotate(self, degrees: f32) -> Self {
        Self {
            rotate: degrees,
            ..self
        }
    }
    pub const fn tint(self, tint: f32) -> Self {
        Self { tint, ..self }
    }
    pub const fn shift(self, shift: f32) -> Self {
        Self { shift, ..self }
    }
}

const REST: Pose = Pose::REST;

/// Where a sequence's easing applies: between each pair of keyframes (CSS `@keyframes`), or once
/// over the whole run (a Web Animations `easing` option, as the editor's shake uses).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EasingScope {
    PerSegment,
    Whole,
}

/// A named keyframe animation: poses at offsets in `0..=1` of `duration` seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframes {
    pub name: &'static str,
    pub duration: f32,
    pub easing: Easing,
    pub scope: EasingScope,
    pub poses: &'static [(f32, Pose)],
    /// Runs forever (`infinite`), else once.
    pub looping: bool,
}

const fn once(
    name: &'static str,
    duration: f32,
    easing: Easing,
    poses: &'static [(f32, Pose)],
) -> Keyframes {
    Keyframes {
        name,
        duration,
        easing,
        scope: EasingScope::PerSegment,
        poses,
        looping: false,
    }
}

/// `rise`: opacity 0 and 4 pt down → in place (`--animate-rise`).
pub const RISE: Keyframes = once(
    "rise",
    SLOW,
    SPRING_SOFT,
    &[(0.0, REST.opacity(0.0).y(4.0)), (1.0, REST)],
);
/// `pop`: opacity 0 at 94% → full size.
pub const POP_IN: Keyframes = once(
    "pop",
    SLOW,
    SPRING_POP,
    &[(0.0, REST.opacity(0.0).scale(0.94)), (1.0, REST)],
);
/// `check-in`: opacity 0 at 40% → full size.
pub const CHECK: Keyframes = once(
    "check",
    SLOW,
    SPRING_POP,
    &[(0.0, REST.opacity(0.0).scale(0.4)), (1.0, REST)],
);
/// `tick`: 130% in the accent colour → rest.
pub const TICK: Keyframes = once(
    "tick",
    SLOW,
    SPRING_SOFT,
    &[(0.0, REST.scale(1.3).tint(1.0)), (1.0, REST)],
);
/// `star-burst`: 420 ms ease-out, 50%/−24° → 125%/6° at 55% → rest.
pub const STAR: Keyframes = once(
    "star",
    0.420,
    Easing::EASE_OUT,
    &[
        (0.0, REST.scale(0.5).rotate(-24.0)),
        (0.55, REST.scale(1.25).rotate(6.0)),
        (1.0, REST),
    ],
);
/// `bar-grow`: 480 ms soft, width scale 0 → 1, from the leading edge.
pub const BAR: Keyframes = once(
    "bar",
    0.480,
    SPRING_SOFT,
    &[(0.0, REST.scale_x(0.0)), (1.0, REST)],
);
/// `shimmer`: 1.6 s linear, forever, background −200% → 200%.
pub const SHIMMER: Keyframes = Keyframes {
    looping: true,
    ..once(
        "shimmer",
        1.6,
        Easing::Linear,
        &[(0.0, REST.shift(-2.0)), (1.0, REST.shift(2.0))],
    )
};
/// The editor's error shake: 0, −3, +3, −2, +1, 0 pt over `SLOW`, eased out as a whole
/// (`components/app/editor/motion.tsx`).
pub const SHAKE: Keyframes = Keyframes {
    scope: EasingScope::Whole,
    ..once(
        "shake",
        SLOW,
        Easing::EASE_OUT,
        &[
            (0.0, REST),
            (0.2, REST.x(-3.0)),
            (0.4, REST.x(3.0)),
            (0.6, REST.x(-2.0)),
            (0.8, REST.x(1.0)),
            (1.0, REST),
        ],
    )
};
/// `arrive`: a new row drops in from 6 pt above.
pub const ARRIVE: Keyframes = once(
    "arrive",
    SLOW,
    SPRING_SOFT,
    &[(0.0, REST.opacity(0.0).y(-6.0)), (1.0, REST)],
);
/// `vanish`: `check-in` backwards, for small marks that turn off.
pub const VANISH: Keyframes = once(
    "vanish",
    MODERATE,
    Easing::EASE_IN,
    &[(0.0, REST), (1.0, REST.opacity(0.0).scale(0.4))],
);
/// `settle`: content refreshed in place dips to 45% and comes back.
pub const SETTLE: Keyframes = once(
    "settle",
    MODERATE,
    Easing::EASE_OUT,
    &[(0.0, REST.opacity(0.45)), (1.0, REST)],
);
/// `fade-in`: a quick swap of content in place.
pub const FADE: Keyframes = once(
    "fade",
    FAST,
    Easing::EASE_OUT,
    &[(0.0, REST.opacity(0.0)), (1.0, REST)],
);
/// `sink`: `rise` played backwards in `MODERATE`, eased out, for exits.
pub const SINK: Keyframes = once(
    "sink",
    MODERATE,
    Easing::EASE_OUT,
    &[(0.0, REST), (1.0, REST.opacity(0.0).y(4.0))],
);

/// Every named sequence (the plan's eleven, plus the clipboard's fade and sink).
pub const SEQUENCES: [Keyframes; 13] = [
    RISE, POP_IN, CHECK, TICK, STAR, BAR, SHIMMER, SHAKE, ARRIVE, VANISH, SETTLE, FADE, SINK,
];

/// The sequences that only change opacity and scale, as Rune beats (`El::sequence`).
pub mod beats {
    use super::*;

    pub static POP_IN: [Beat; 2] = [
        Beat::new(0.0).opacity(0.0).scale(0.94),
        Beat::new(SLOW as f64).easing(SPRING_POP),
    ];
    pub static CHECK: [Beat; 2] = [
        Beat::new(0.0).opacity(0.0).scale(0.4),
        Beat::new(SLOW as f64).easing(SPRING_POP),
    ];
    pub static VANISH: [Beat; 2] = [
        Beat::new(0.0),
        Beat::new(MODERATE as f64)
            .opacity(0.0)
            .scale(0.4)
            .easing(Easing::EASE_IN),
    ];
    pub static SETTLE: [Beat; 2] = [
        Beat::new(0.0).opacity(0.45),
        Beat::new(MODERATE as f64).easing(Easing::EASE_OUT),
    ];
    pub static FADE: [Beat; 2] = [
        Beat::new(0.0).opacity(0.0),
        Beat::new(FAST as f64).easing(Easing::EASE_OUT),
    ];
}

#[cfg(test)]
mod tests {
    use super::super::css;
    use super::*;

    fn check_stops(stops: &[[f32; 2]], token: &str) {
        let samples = css::Sheet::parse(css::GLOBALS).linear(token);
        assert_eq!(stops.len(), samples.len(), "--{token}: sample count");
        let last = (samples.len() - 1) as f32;
        for (i, (stop, y)) in stops.iter().zip(&samples).enumerate() {
            assert!(
                (stop[0] - i as f32 / last).abs() < 1e-6,
                "--{token} stop {i}: x {}",
                stop[0]
            );
            assert_eq!(stop[1], *y, "--{token} stop {i}: y");
        }
    }

    #[test]
    fn piecewise_easings_are_the_css_linear_lists() {
        check_stops(&SOFT_STOPS, "spring-soft");
        check_stops(&POP_STOPS, "spring-pop");
        assert_eq!(SPRING_SOFT, Easing::Piecewise(&SOFT_STOPS));
        assert!((SPRING_POP.eval(0.5) - 1.106).abs() < 1e-6);
    }

    fn worst_error(spring: Spring, duration: f32, stops: &[[f32; 2]]) -> f32 {
        stops
            .iter()
            .map(|[x, y]| (spring.sample(0.0, 0.0, 1.0, x * duration).0 - y).abs())
            .fold(0.0, f32::max)
    }

    #[test]
    fn fitted_springs_stay_within_0_02_of_the_samples() {
        for duration in [FAST, MODERATE, SLOW, 0.480] {
            let soft = worst_error(soft_spring(duration), duration, &SOFT_STOPS);
            let pop = worst_error(pop_spring(duration), duration, &POP_STOPS);
            assert!(soft <= 0.02, "soft at {duration}s: {soft}");
            assert!(pop <= 0.02, "pop at {duration}s: {pop}");
        }
        assert_eq!((SOFT, POP), (soft_spring(SLOW), pop_spring(SLOW)));
    }

    #[test]
    fn speeds_equal_the_css() {
        let sheet = css::Sheet::parse(css::GLOBALS);
        assert_eq!(sheet.millis("motion-fast"), FAST * 1000.0);
        assert_eq!(sheet.millis("motion-moderate"), MODERATE * 1000.0);
        assert_eq!(sheet.millis("motion-slow"), SLOW * 1000.0);
    }

    #[test]
    fn sequences_start_at_zero_and_end_at_one() {
        for k in SEQUENCES {
            let first = k.poses.first().unwrap().0;
            let last = k.poses.last().unwrap().0;
            assert_eq!((first, last), (0.0, 1.0), "{}", k.name);
            assert!(k.poses.windows(2).all(|p| p[0].0 < p[1].0), "{}", k.name);
        }
    }

    #[test]
    fn beats_match_their_sequences() {
        for (beats, k) in [
            (&beats::POP_IN, POP_IN),
            (&beats::CHECK, CHECK),
            (&beats::VANISH, VANISH),
            (&beats::SETTLE, SETTLE),
            (&beats::FADE, FADE),
        ] {
            let seq = rune::Sequence::new(beats);
            assert!(
                (seq.duration() as f32 - k.duration).abs() < 1e-6,
                "{}",
                k.name
            );
            for (beat, (_, pose)) in beats.iter().zip(k.poses) {
                assert_eq!(
                    (beat.opacity, beat.scale),
                    (pose.opacity, pose.scale[0]),
                    "{}",
                    k.name
                );
            }
            assert_eq!(beats[1].easing, k.easing, "{}", k.name);
        }
    }
}
