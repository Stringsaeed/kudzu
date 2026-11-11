//! Where the palette sits on the display under the pointer.
//!
//! Display-local points from that display's top-left, which is what
//! `OverlayPlacement::Rect` consumes. AppKit's frame is bottom-left; converting that
//! origin is the caller's job. The live call (pointer display, fullscreen Spaces) is S71.

pub const WIDTH: f32 = 760.0;
pub const HEIGHT: f32 = 480.0;
/// Fraction of the display height from the top.
pub const FRACTION: f32 = 0.20;

/// Top-left of the 760×480 panel, centered, `FRACTION` of the way down the display.
pub fn display_local_origin(display_width: f32, display_height: f32) -> (f32, f32) {
    ((display_width - WIDTH) / 2.0, display_height * FRACTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_is_centered_a_fifth_of_the_way_down() {
        let (x, y) = display_local_origin(1920.0, 1080.0);
        assert!((x - 580.0).abs() < f32::EPSILON);
        assert!((y - 216.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_narrower_display_keeps_the_same_fraction() {
        let (x, y) = display_local_origin(800.0, 600.0);
        assert!((x - 20.0).abs() < f32::EPSILON);
        assert!((y - 120.0).abs() < f32::EPSILON);
    }
}
