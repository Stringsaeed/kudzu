//! Where the picker sits on the display under the pointer: centred, a fifth of the way down,
//! as the palette is. Display-local points from the display's top-left, which is what
//! `OverlayPlacement::Rect` takes. The live call (pointer display, full-screen Spaces) is S71.

pub const WIDTH: f32 = 800.0;
pub const HEIGHT: f32 = 520.0;
/// Fraction of the display height from the top.
pub const FRACTION: f32 = 0.20;

/// Top-left of the 800×520 panel on a display of the given size.
pub fn display_local_origin(display_width: f32, display_height: f32) -> (f32, f32) {
    ((display_width - WIDTH) / 2.0, display_height * FRACTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_is_centred_a_fifth_of_the_way_down() {
        let (x, y) = display_local_origin(1920.0, 1080.0);
        assert!((x - 560.0).abs() < f32::EPSILON && (y - 216.0).abs() < f32::EPSILON);
    }
}
