//! `components/ui/separator.tsx`: a 1 pt rule in `--border`.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::El;

use crate::ui::theme::Theme;

/// `orientation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// `<Separator />`: `h-px w-full`, or `w-px self-stretch` when vertical. Decorative, as by default.
pub fn separator(th: &Theme, o: Orientation) -> El {
    let e = rune::el().fill(th.t.border).a11y_hidden();
    match o {
        Orientation::Horizontal => e.h(1.0).w_full(),
        Orientation::Vertical => e.w(1.0).self_stretch(),
    }
}

/// A horizontal separator.
pub fn horizontal(th: &Theme) -> El {
    separator(th, Orientation::Horizontal)
}

/// A vertical separator.
pub fn vertical(th: &Theme) -> El {
    separator(th, Orientation::Vertical)
}
