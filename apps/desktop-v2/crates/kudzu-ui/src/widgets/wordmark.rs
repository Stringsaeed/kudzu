//! The Echo wordmark (`components/app/wordmark.tsx`, `design/wordmark/`), drawn in the current
//! ink. `Full` is the detailed outline set; `Small` keeps the steps legible at compact sizes.
//!
//! Rune icons are square and ignore a nested `viewBox`, so each SVG is wrapped in a group that
//! moves its viewBox origin to the square's and centres it vertically; the element is the
//! wordmark's own aspect ratio, with the square icon centred on it.

use rune::{el, icon, Color, El, Icon};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    #[default]
    Full,
    Small,
}

/// viewBox `-1.5 -1.5 735 201`: centred in a 735 square.
static FULL: Icon = Icon {
    name: "kudzu-wordmark-echo",
    view: 735.0,
    body: concat!(
        r#"<g transform="translate(1.5 268.5)">"#,
        include_str!("../../../../design/wordmark/kudzu-wordmark-echo.svg"),
        "</g>"
    ),
};

/// viewBox `-4.5 -4.5 741 207`: centred in a 741 square.
static SMALL: Icon = Icon {
    name: "kudzu-wordmark-echo-small",
    view: 741.0,
    body: concat!(
        r#"<g transform="translate(4.5 271.5)">"#,
        include_str!("../../../../design/wordmark/kudzu-wordmark-echo-small.svg"),
        "</g>"
    ),
};

impl Variant {
    /// Width over height.
    pub fn aspect(self) -> f32 {
        match self {
            Variant::Full => 735.0 / 201.0,
            Variant::Small => 741.0 / 207.0,
        }
    }
    fn icon(self) -> &'static Icon {
        match self {
            Variant::Full => &FULL,
            Variant::Small => &SMALL,
        }
    }
}

/// The wordmark `height` points tall, in `ink`; named "Kudzu" for assistive technology.
pub fn wordmark(variant: Variant, height: f32, ink: Color) -> El {
    let w = height * variant.aspect();
    el().role("image").label("Kudzu").size(w, height).child(
        icon(variant.icon(), w)
            .absolute(0.0, (height - w) / 2.0)
            .color(ink)
            .a11y_hidden(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_keeps_the_svg_aspect() {
        assert!((Variant::Full.aspect() - 3.6567).abs() < 1e-3);
        assert!((Variant::Small.aspect() - 3.5797).abs() < 1e-3);
        assert!(FULL.body.contains("viewBox=\"-1.5 -1.5 735 201\""));
        assert!(SMALL.body.contains("viewBox=\"-4.5 -4.5 741 207\""));
    }
}
