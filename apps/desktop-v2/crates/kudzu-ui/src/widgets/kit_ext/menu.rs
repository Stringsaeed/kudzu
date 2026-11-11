//! RUNE-GAP(#573: rune-kit `Menu` and `Select` take the floating surface's radius from `Kit`):
//! delete this module and build `rune_kit::Menu` directly once they do.
//!
//! shadcn's `DropdownMenu` and `ContextMenu` content (`rounded-lg`, 10 points) as rune-kit's
//! [`rune_kit::Menu`], decorated. Verdict in `PARITY_MATRIX.md`: **decorated**. rune-kit draws
//! the menu surface with a fixed 6 point radius; the built surface is pushed onto the overlay
//! layer, so the radius is set on what `Menu::build` pushed. The scrim and the context menu's
//! anchor point are pushed too: both are transparent, so their corners never show.

use rune::El;
use rune_kit::Menu;

use crate::theme::{radius, Theme};

/// `rounded-lg`.
pub const RADIUS: f32 = radius::LG;

/// `menu.build(kit, layer)` with Kudzu's surface radius.
pub fn build(menu: Menu, theme: &Theme, layer: &mut Vec<El>) -> El {
    let from = layer.len();
    let trigger = menu.build(theme.kit(), layer);
    round(layer, from, RADIUS);
    trigger
}

/// Sets `r` on every overlay element pushed from `from` on.
pub(super) fn round(layer: &mut [El], from: usize, r: f32) {
    for e in layer.iter_mut().skip(from) {
        e.set_radius(r);
    }
}
