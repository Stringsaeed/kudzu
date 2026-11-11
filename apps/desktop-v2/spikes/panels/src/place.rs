//! Placement matches today's floating windows: a band down the screen under the
//! pointer, and a form beside the caret that flips above when it would pass the
//! bottom. Coordinates are top-down global points of the full display frame.

pub const PALETTE_W: f64 = 760.0;
pub const PALETTE_H: f64 = 480.0;
pub const PICKER_W: f64 = 800.0;
pub const PICKER_H: f64 = 520.0;
pub const FORM_W: f64 = 440.0;
pub const FORM_H_INITIAL: f64 = 360.0;
pub const FORM_H_MIN: f64 = 160.0;
/// Plan §1.3. The current presenter clamps a resize at 680.
pub const FORM_H_MAX: f64 = 640.0;
pub const PALETTE_FRACTION: f64 = 0.20;
pub const PICKER_FRACTION: f64 = 0.18;
pub const FORM_EMPTY_FRACTION: f64 = 0.28;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Top-left of a panel centered horizontally, `fraction` of the way down `screen`.
pub fn band_origin(screen: Frame, panel_w: f64, fraction: f64) -> (f64, f64) {
    (
        screen.x + (screen.w - panel_w) / 2.0,
        screen.y + screen.h * fraction,
    )
}

pub fn clamp_form_height(height: f64) -> f64 {
    height.clamp(FORM_H_MIN, FORM_H_MAX)
}

/// Top-left of the form. `None` centers it at 28% of the screen height.
pub fn form_origin(screen: Frame, caret: Option<Caret>, form_h: f64) -> (f64, f64) {
    let Some(caret) = caret else {
        return (
            screen.x + (screen.w - FORM_W) / 2.0,
            screen.y + screen.h * FORM_EMPTY_FRACTION,
        );
    };
    let mut x = caret.x - 24.0;
    let mut y = caret.y + caret.h + 6.0;
    if y + form_h > screen.y + screen.h {
        y = (caret.y - form_h - 6.0).max(screen.y);
    }
    let min_x = screen.x + 8.0;
    let max_x = (screen.x + screen.w - FORM_W - 8.0).max(min_x);
    x = x.clamp(min_x, max_x);
    (x, y)
}

/// Display-local points from the full frame's top-left, which is what a launcher
/// `Rect` placement consumes.
pub fn to_local(screen: Frame, (x, y): (f64, f64)) -> (f64, f64) {
    (x - screen.x, y - screen.y)
}

pub fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen() -> Frame {
        Frame {
            x: 0.0,
            y: 0.0,
            w: 1440.0,
            h: 900.0,
        }
    }

    #[test]
    fn palette_sits_at_twenty_percent_and_picker_at_eighteen() {
        let screen = screen();
        let (px, py) = band_origin(screen, PALETTE_W, PALETTE_FRACTION);
        let (cx, cy) = band_origin(screen, PICKER_W, PICKER_FRACTION);
        assert_eq!(py, 180.0);
        assert_eq!(cy, 162.0);
        assert_eq!(px, (1440.0 - PALETTE_W) / 2.0);
        assert_eq!(cx, (1440.0 - PICKER_W) / 2.0);
        assert_eq!(to_local(screen, (px, py)), (px, py));
    }

    #[test]
    fn a_secondary_display_keeps_the_band_in_that_frame() {
        let screen = Frame {
            x: 1440.0,
            y: -200.0,
            w: 1920.0,
            h: 1080.0,
        };
        let (x, y) = band_origin(screen, PALETTE_W, PALETTE_FRACTION);
        assert_eq!(y, -200.0 + 216.0);
        assert_eq!(
            to_local(screen, (x, y)),
            ((1920.0 - PALETTE_W) / 2.0, 216.0)
        );
    }

    #[test]
    fn form_opens_under_the_caret_and_flips_above_the_bottom() {
        let screen = screen();
        let below = form_origin(
            screen,
            Some(Caret {
                x: 400.0,
                y: 200.0,
                w: 2.0,
                h: 18.0,
            }),
            200.0,
        );
        assert_eq!(below, (376.0, 224.0));
        let above = form_origin(
            screen,
            Some(Caret {
                x: 400.0,
                y: 860.0,
                w: 2.0,
                h: 18.0,
            }),
            200.0,
        );
        assert_eq!(above, (376.0, 654.0));
        assert!(above.1 + 200.0 <= screen.y + screen.h);
    }

    #[test]
    fn form_clamps_x_and_uses_the_empty_caret_band() {
        let screen = screen();
        let left = form_origin(
            screen,
            Some(Caret {
                x: 0.0,
                y: 100.0,
                w: 1.0,
                h: 16.0,
            }),
            180.0,
        );
        assert_eq!(left.0, 8.0);
        let right = form_origin(
            screen,
            Some(Caret {
                x: 2000.0,
                y: 100.0,
                w: 1.0,
                h: 16.0,
            }),
            180.0,
        );
        assert_eq!(right.0, 1440.0 - FORM_W - 8.0);
        let empty = form_origin(screen, None, 360.0);
        assert_eq!(
            empty,
            ((1440.0 - FORM_W) / 2.0, 900.0 * FORM_EMPTY_FRACTION)
        );
    }

    #[test]
    fn form_height_clamps_to_the_plan_range() {
        assert_eq!(clamp_form_height(40.0), FORM_H_MIN);
        assert_eq!(clamp_form_height(900.0), FORM_H_MAX);
        assert_eq!(clamp_form_height(360.0), 360.0);
    }
}
