//! shadcn's `Checkbox` (`components/ui/checkbox.tsx`) as rune-kit's [`rune_kit::Checkbox`],
//! themed for its checked colours.
//!
//! Verdict in `PARITY_MATRIX.md`: **themed** (D6, after S06). rune-kit fills a checked box with
//! the kit's `surface_weak`, borders it with `std_border` and draws the mark in `std_icon`;
//! shadcn's is `bg-primary border-primary text-primary-foreground`. Those three palette roles
//! colour other kit surfaces too, so the checkbox is built with its own copy of the kit where
//! they are the primary colours. Not reachable: the checked box's `raised` depth and
//! `sheen-strong` (the box is an inner element), recorded in the matrix.

use std::sync::LazyLock;

use rune::El;
use rune_kit::{Checkbox, Kit};

use crate::theme::Theme;

fn kit(theme: &Theme) -> &'static Kit {
    static LIGHT: LazyLock<Kit> = LazyLock::new(|| checked_kit(Theme::light()));
    static DARK: LazyLock<Kit> = LazyLock::new(|| checked_kit(Theme::dark()));
    if theme.dark {
        &DARK
    } else {
        &LIGHT
    }
}

fn checked_kit(theme: &Theme) -> Kit {
    let mut k = theme.kit().clone();
    k.c.surface_weak = theme.c.primary;
    k.c.std_border = theme.c.primary;
    k.c.std_icon = theme.c.primary_foreground;
    k
}

pub fn build(checkbox: Checkbox, theme: &Theme) -> El {
    checkbox.build(kit(theme))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_roles_are_primary() {
        for t in [Theme::light(), Theme::dark()] {
            let k = kit(t);
            assert_eq!(
                (k.c.surface_weak, k.c.std_border, k.c.std_icon),
                (t.c.primary, t.c.primary, t.c.primary_foreground)
            );
            // Nothing else changes.
            assert_eq!(k.c.surface_raised, t.kit().c.surface_raised);
        }
    }
}
