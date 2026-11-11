//! Caret placement: top-left global points of the full display frame, matching today's form.

use kudzu_core::system::Rect as Caret;

pub const FORM_W: f64 = 440.0;
pub const FORM_H_MIN: f64 = 160.0;
pub const FORM_H_MAX: f64 = 640.0;
pub const FORM_H_WINDOW_MAX: f64 = 680.0;
pub const FORM_EMPTY_FRACTION: f64 = 0.28;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn clamp_form_height(height: f64) -> f64 {
    height.clamp(FORM_H_MIN, FORM_H_MAX)
}

pub fn clamp_window_height(height: f64) -> f64 {
    height.clamp(FORM_H_MIN, FORM_H_WINDOW_MAX)
}

/// Top-left of the form. `None` centres it at 28% of the screen height.
pub fn form_origin(screen: Frame, caret: Option<Caret>, form_h: f64) -> (f64, f64) {
    let Some(caret) = caret else {
        return (
            screen.x + (screen.w - FORM_W) / 2.0,
            screen.y + screen.h * FORM_EMPTY_FRACTION,
        );
    };
    let mut x = caret.x - 24.0;
    let mut y = caret.y + caret.height + 6.0;
    if y + form_h > screen.y + screen.h {
        y = (caret.y - form_h - 6.0).max(screen.y);
    }
    let min_x = screen.x + 8.0;
    let max_x = (screen.x + screen.w - FORM_W - 8.0).max(min_x);
    x = x.clamp(min_x, max_x);
    (x, y)
}

/// Display-local points from the full frame's top-left.
pub fn to_local(screen: Frame, (x, y): (f64, f64)) -> (f64, f64) {
    (x - screen.x, y - screen.y)
}

pub fn screen_holding(screens: &[Frame], caret: Option<Caret>) -> Option<Frame> {
    let Some(caret) = caret else {
        return screens.first().copied();
    };
    screens
        .iter()
        .copied()
        .find(|s| caret.x >= s.x && caret.x < s.x + s.w && caret.y >= s.y && caret.y < s.y + s.h)
        .or_else(|| screens.first().copied())
}

/// AppKit bottom-left `frame` as a top-left `Frame`, using the canvas's top edge.
pub fn top_left_frame(
    frame: kudzu_appkit::geometry::Rect,
    canvas: kudzu_appkit::geometry::Rect,
) -> Frame {
    Frame {
        x: frame.x,
        y: canvas.max_y() - frame.max_y(),
        w: frame.width,
        h: frame.height,
    }
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

    fn caret(x: f64, y: f64, h: f64) -> Caret {
        Caret {
            x,
            y,
            width: 2.0,
            height: h,
        }
    }

    #[test]
    fn form_opens_under_the_caret_and_flips_above_the_bottom() {
        let screen = screen();
        let below = form_origin(screen, Some(caret(400.0, 200.0, 18.0)), 200.0);
        assert_eq!(below, (376.0, 224.0));
        let above = form_origin(screen, Some(caret(400.0, 860.0, 18.0)), 200.0);
        assert_eq!(above, (376.0, 654.0));
        assert!(above.1 + 200.0 <= screen.y + screen.h);
    }

    #[test]
    fn form_clamps_x_and_uses_the_empty_caret_band() {
        let screen = screen();
        let left = form_origin(screen, Some(caret(0.0, 100.0, 16.0)), 180.0);
        assert_eq!(left.0, 8.0);
        let right = form_origin(screen, Some(caret(2000.0, 100.0, 16.0)), 180.0);
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
        assert_eq!(clamp_window_height(900.0), FORM_H_WINDOW_MAX);
    }

    #[test]
    fn a_secondary_display_keeps_local_origin() {
        let screen = Frame {
            x: 1440.0,
            y: -200.0,
            w: 1920.0,
            h: 1080.0,
        };
        let origin = form_origin(screen, None, 360.0);
        assert_eq!(
            to_local(screen, origin),
            ((1920.0 - FORM_W) / 2.0, 1080.0 * FORM_EMPTY_FRACTION)
        );
    }

    #[test]
    fn appkit_bottom_left_flips_to_top_left() {
        let canvas = kudzu_appkit::geometry::Rect {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        let lower = kudzu_appkit::geometry::Rect {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 400.0,
        };
        let frame = top_left_frame(lower, canvas);
        assert_eq!(frame.y, 500.0);
        assert_eq!(frame.h, 400.0);
    }
}
