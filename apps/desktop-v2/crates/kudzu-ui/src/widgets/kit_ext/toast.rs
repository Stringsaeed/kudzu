//! RUNE-GAP(#573: rune-kit `Toast` takes its radius from `Kit`): delete this module and build
//! `rune_kit::Toast` directly once it does.
//!
//! Sonner's toast (`components/ui/sonner.tsx`: `--border-radius: calc(var(--radius) * 1.4)`, 14
//! points) as rune-kit's [`rune_kit::Toast`], decorated. Verdict in `PARITY_MATRIX.md`:
//! **decorated** (D6, after S06): rune-kit's toast has fixed 8 point corners; the built element
//! takes Kudzu's.

use rune::El;
use rune_kit::Toast;

use crate::theme::{radius, Theme};

/// `calc(var(--radius) * 1.4)`.
pub const RADIUS: f32 = radius::BASE * 1.4;

pub fn build(toast: Toast, theme: &Theme) -> El {
    toast.build(theme.kit()).radius(RADIUS)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_radius_is_sonners() {
        assert_eq!(super::RADIUS, 14.0);
    }
}
