//! RUNE-GAP(#573: rune-kit `Toast` takes its radius from `Kit`): delete this module and build
//! `rune_kit::Toast` directly once it does.
//!
//! Sonner's toast (`components/ui/sonner.tsx`: `--border-radius: calc(var(--radius) * 1.4)`, 14
//! points; Sonner's `--width` 356) as rune-kit's [`rune_kit::Toast`], decorated. Verdict in
//! `PARITY_MATRIX.md`: **decorated** (D6 after S06, D21): rune-kit's toast is 320 wide with fixed
//! 8 point corners; the built element takes Kudzu's.

use rune::El;
use rune_kit::Toast;

use crate::theme::{radius, Theme};

/// `calc(var(--radius) * 1.4)`.
pub const RADIUS: f32 = radius::BASE * 1.4;
/// Sonner's `--width`.
pub const WIDTH: f32 = 356.0;

pub fn build(toast: Toast, theme: &Theme) -> El {
    toast.build(theme.kit()).radius(RADIUS).w(WIDTH)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_radius_is_sonners() {
        assert_eq!((super::RADIUS, super::WIDTH), (14.0, 356.0));
    }
}
