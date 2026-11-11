//! Click-through guide: Settings outline plus a ghost icon. Rune overlay first; native NSPanel
//! only if the overlay is not on the active Space.

use std::time::Duration;

use rune::{
    el, Align, Color, ConfiguredOverlay, Cx, El, OverlayInput, OverlayOptions, OverlaySpec,
    ScreenEdge, Tx,
};

use crate::place::{self, Rect};

pub struct Guide {
    pub playing: bool,
    pub start: f64,
    pub elapsed: f64,
    pub destination: Rect,
    pub source: (f64, f64),
    pub canvas: Rect,
    pub reduce_motion: bool,
}

impl Default for Guide {
    fn default() -> Self {
        Self {
            playing: false,
            start: 0.0,
            elapsed: 0.0,
            destination: Rect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            source: (0.0, 0.0),
            canvas: Rect {
                x: 0.0,
                y: 0.0,
                w: 800.0,
                h: 800.0,
            },
            reduce_motion: false,
        }
    }
}

pub fn spec() -> ConfiguredOverlay<Guide> {
    OverlaySpec {
        name: "kudzu-spike-guide",
        size: (800.0, 800.0),
        edge: ScreenEdge::Top,
        margin: 0.0,
        align: Align::Center,
        inset: 0.0,
        interactive: false,
        view,
    }
    .options(OverlayOptions {
        prewarm: false,
        input: OverlayInput::None,
        ..OverlayOptions::default()
    })
    .macos_level(3)
    .poll(poll)
}

fn poll(guide: &mut Guide, t: f64, tx: &mut Tx) -> bool {
    if !guide.playing {
        return false;
    }
    if guide.start == 0.0 {
        guide.start = t;
    }
    guide.elapsed = t - guide.start;
    if guide.elapsed > 4.8 || guide.reduce_motion {
        guide.playing = false;
        return true;
    }
    tx.wake_after(Duration::from_secs_f64(1.0 / 30.0));
    true
}

fn view(guide: &Guide, _: &mut Cx<Guide>) -> El {
    if !guide.playing || guide.destination.is_empty() {
        return el().w_full().h_full();
    }
    let phase = (guide.elapsed % 2.4) / 2.4;
    let fade = place::ghost_fade(phase) as f32;
    let (dx, dy, dw, dh) = place::rect_to_overlay(guide.canvas, guide.destination);
    let accent = Color {
        r: 0.08,
        g: 0.37,
        b: 0.90,
        a: 0.65 * fade,
    };
    let (gx, gy) = place::ghost_point(guide.source, guide.destination, phase);
    let (ix, iy) = place::to_overlay(guide.canvas, gx, gy);
    el().w_full()
        .h_full()
        .child(
            el().key("outline")
                .pin(Some(dx - 3.0), Some(dy - 3.0), None, None)
                .size(dw + 6.0, dh + 6.0)
                .radius(14.0)
                .border(3.0, accent),
        )
        .child(
            el().key("ghost")
                .pin(Some(ix - 26.0), Some(iy - 26.0), None, None)
                .size(52.0, 52.0)
                .radius(12.0)
                .fill(Color {
                    r: 0.08,
                    g: 0.37,
                    b: 0.90,
                    a: 0.85 * fade,
                }),
        )
}

pub fn arm(
    guide: &mut Guide,
    canvas: Rect,
    destination: Rect,
    source: (f64, f64),
    reduce_motion: bool,
) {
    guide.canvas = canvas;
    guide.destination = destination;
    guide.source = source;
    guide.reduce_motion = reduce_motion;
    guide.playing = !reduce_motion
        && place::should_animate(
            Rect {
                x: 0.0,
                y: 0.0,
                w: place::COMPANION_W,
                h: place::COMPANION_H,
            },
            destination,
            reduce_motion,
        );
    guide.start = 0.0;
    guide.elapsed = 0.0;
}
