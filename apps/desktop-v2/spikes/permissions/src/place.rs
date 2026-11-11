//! Companion placement beside System Settings. Coordinates are AppKit's: origin at the
//! bottom-left of the primary display, y up. Matches `macos.m` `showBesideSettings`.

pub const COMPANION_W: f64 = 420.0;
pub const COMPANION_H: f64 = 660.0;
pub const GAP: f64 = 18.0;
pub const MARGIN: f64 = 12.0;
pub const CANVAS_PAD: f64 = 80.0;
pub const MIN_SETTINGS_W: f64 = 400.0;
pub const MIN_SETTINGS_H: f64 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn max_x(self) -> f64 {
        self.x + self.w
    }
    pub fn max_y(self) -> f64 {
        self.y + self.h
    }
    pub fn mid_x(self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn mid_y(self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn is_empty(self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }
    pub fn intersects(self, other: Self) -> bool {
        self.x < other.max_x()
            && other.x < self.max_x()
            && self.y < other.max_y()
            && other.y < self.max_y()
    }
    pub fn union(self, other: Self) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            w: self.max_x().max(other.max_x()) - x,
            h: self.max_y().max(other.max_y()) - y,
        }
    }
    pub fn inset(self, d: f64) -> Self {
        Self {
            x: self.x - d,
            y: self.y - d,
            w: self.w + 2.0 * d,
            h: self.h + 2.0 * d,
        }
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn contains_point(self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.max_x() && y >= self.y && y < self.max_y()
    }
}

/// CoreGraphics window bounds (origin top-left of the primary display, y down) to AppKit.
pub fn cg_to_appkit(bounds: Rect, desktop_top: f64) -> Rect {
    Rect {
        x: bounds.x,
        y: desktop_top - bounds.max_y(),
        w: bounds.w,
        h: bounds.h,
    }
}

/// Top-left of the companion. Prefers the left of Settings; flips to the right when the left
/// gap would sit past the visible-frame margin. Clamps to a 12 pt inset.
pub fn companion_origin(destination: Rect, available: Rect) -> (f64, f64) {
    let mut x = destination.x - COMPANION_W - GAP;
    if x < available.x + MARGIN {
        x = destination.max_x() + GAP;
    }
    // macos.m: MAX(lower, MIN(preferred, upper)). When the companion is taller than the
    // visible frame, the lower bound wins so the origin stays on-screen even if the top overflows.
    x = x
        .min(available.max_x() - COMPANION_W - MARGIN)
        .max(available.x + MARGIN);
    let y = (destination.max_y() - COMPANION_H)
        .min(available.max_y() - COMPANION_H - MARGIN)
        .max(available.y + MARGIN);
    (x, y)
}

pub fn companion_frame(destination: Rect, available: Rect) -> Rect {
    let (x, y) = companion_origin(destination, available);
    Rect {
        x,
        y,
        w: COMPANION_W,
        h: COMPANION_H,
    }
}

/// Overlay canvas: union of companion and Settings, expanded 80 pt.
pub fn canvas(companion: Rect, destination: Rect) -> Rect {
    companion.union(destination).inset(CANVAS_PAD)
}

pub fn should_animate(companion: Rect, destination: Rect, reduce_motion: bool) -> bool {
    !companion.intersects(destination) && !reduce_motion && !destination.is_empty()
}

/// AppKit point to overlay-local top-down points.
pub fn to_overlay(canvas: Rect, x: f64, y: f64) -> (f32, f32) {
    ((x - canvas.x) as f32, (canvas.max_y() - y) as f32)
}

/// AppKit rect to overlay-local top-down rect (origin top-left of the overlay).
pub fn rect_to_overlay(canvas: Rect, r: Rect) -> (f32, f32, f32, f32) {
    let (x, y) = to_overlay(canvas, r.x, r.max_y());
    (x, y, r.w as f32, r.h as f32)
}

/// Overlay placement for an AppKit canvas: points from the primary display's top-left.
pub fn canvas_to_overlay_placement(canvas: Rect, desktop_top: f64) -> (f32, f32, f32, f32) {
    (
        canvas.x as f32,
        (desktop_top - canvas.max_y()) as f32,
        canvas.w as f32,
        canvas.h as f32,
    )
}

/// Ghost icon centre along the same cubic the Tauri overlay uses. `phase` is 0..1 of one 2.4 s cycle.
pub fn ghost_point(source: (f64, f64), destination: Rect, phase: f64) -> (f64, f64) {
    let end = (
        destination.mid_x() + destination.w * 0.15,
        destination.mid_y(),
    );
    let bend = ((source.0 + end.0) / 2.0, source.1.max(end.1) + 65.0);
    let t = ((phase - 0.12) / 0.65).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let u = 1.0 - t;
    (
        u * u * u * source.0 + 3.0 * u * u * t * bend.0 + t * t * t * end.0,
        u * u * u * source.1 + 3.0 * u * u * t * bend.1 + t * t * t * end.1,
    )
}

pub fn ghost_fade(phase: f64) -> f64 {
    (phase * 10.0).min((1.0 - phase) * 10.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Rect {
        Rect {
            x: 0.0,
            y: 39.0,
            w: 1800.0,
            h: 1065.0,
        }
    }

    #[test]
    fn cg_bounds_flip_around_the_primary_display_height() {
        let cg = Rect {
            x: 100.0,
            y: 80.0,
            w: 900.0,
            h: 700.0,
        };
        let appkit = cg_to_appkit(cg, 1169.0);
        assert_eq!(
            appkit,
            Rect {
                x: 100.0,
                y: 389.0,
                w: 900.0,
                h: 700.0
            }
        );
    }

    #[test]
    fn prefers_the_left_of_settings_with_an_eighteen_point_gap() {
        let dest = Rect {
            x: 700.0,
            y: 200.0,
            w: 900.0,
            h: 700.0,
        };
        let (x, y) = companion_origin(dest, screen());
        assert_eq!(x, 700.0 - COMPANION_W - GAP);
        assert_eq!(y, dest.max_y() - COMPANION_H);
        let frame = companion_frame(dest, screen());
        assert!(!frame.intersects(dest));
        assert!(should_animate(frame, dest, false));
        assert!(!should_animate(frame, dest, true));
    }

    #[test]
    fn flips_to_the_right_when_the_left_gap_crosses_the_margin() {
        let dest = Rect {
            x: 40.0,
            y: 100.0,
            w: 800.0,
            h: 600.0,
        };
        let (x, _) = companion_origin(dest, screen());
        assert_eq!(x, dest.max_x() + GAP);
    }

    #[test]
    fn clamps_to_a_twelve_point_visible_frame_inset() {
        let dest = Rect {
            x: 0.0,
            y: 0.0,
            w: 500.0,
            h: 400.0,
        };
        let available = Rect {
            x: 0.0,
            y: 0.0,
            w: 700.0,
            h: 500.0,
        };
        let (x, y) = companion_origin(dest, available);
        assert!(x >= available.x + MARGIN);
        assert!(y >= available.y + MARGIN);
        assert!(x + COMPANION_W <= available.max_x() - MARGIN + 0.01);
        let frame = companion_frame(dest, available);
        assert!(
            frame.intersects(dest),
            "a display too narrow for both windows overlaps"
        );
        assert!(!should_animate(frame, dest, false));
    }

    #[test]
    fn canvas_is_the_union_expanded_eighty_points() {
        let a = Rect {
            x: 100.0,
            y: 200.0,
            w: 420.0,
            h: 660.0,
        };
        let b = Rect {
            x: 538.0,
            y: 180.0,
            w: 900.0,
            h: 700.0,
        };
        let c = canvas(a, b);
        assert_eq!(c.x, 20.0);
        assert_eq!(c.y, 100.0);
        assert_eq!(c.max_x(), 538.0 + 900.0 + 80.0);
        assert_eq!(c.max_y(), 180.0 + 700.0 + 80.0);
        let (ox, oy, ow, oh) = rect_to_overlay(c, b);
        assert_eq!((ow, oh), (900.0, 700.0));
        assert_eq!(ox, (b.x - c.x) as f32);
        assert_eq!(oy, (c.max_y() - b.max_y()) as f32);
        let (px, py, pw, ph) = canvas_to_overlay_placement(c, 1169.0);
        assert_eq!((pw, ph), (c.w as f32, c.h as f32));
        assert_eq!(px, c.x as f32);
        assert_eq!(py, (1169.0 - c.max_y()) as f32);
    }

    #[test]
    fn ghost_starts_at_the_source_and_ends_inside_settings() {
        let source = (210.0, 556.0);
        let dest = Rect {
            x: 538.0,
            y: 180.0,
            w: 900.0,
            h: 700.0,
        };
        let start = ghost_point(source, dest, 0.0);
        assert!((start.0 - source.0).abs() < 0.01);
        assert!((start.1 - source.1).abs() < 0.01);
        let end = ghost_point(source, dest, 1.0);
        assert!(dest.contains_point(end.0, end.1));
        assert!((ghost_fade(0.0) - 0.0).abs() < 1e-9);
        assert!((ghost_fade(0.5) - 1.0).abs() < 1e-9);
    }
}
