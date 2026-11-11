//! The 12 elevation tokens of `globals.css` (`--elev-*`, light at line 222, dark at line 298).
//! Each CSS `box-shadow` layer is one [`Layer`], in CSS order, so it becomes one
//! `rune::Shadow { x, y, blur, spread, color, inset }` (Rune's `blur` is the CSS blur radius).
//!
//! The data tables are long by nature; the module is otherwise small.

use rune::Shadow;

use super::palette::Oklch;

/// One CSS shadow layer: `[inset] x y blur spread colour`, lengths in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    pub x: f32,
    pub y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Oklch,
    pub inset: bool,
}

impl Layer {
    pub fn shadow(&self) -> Shadow {
        Shadow {
            x: self.x,
            y: self.y,
            blur: self.blur,
            spread: self.spread,
            color: self.color.color(),
            inset: self.inset,
        }
    }
}

/// An outer layer.
const fn out(y: f32, blur: f32, spread: f32, color: Oklch) -> Layer {
    Layer {
        x: 0.0,
        y,
        blur,
        spread,
        color,
        inset: false,
    }
}

/// An inset layer.
const fn ins(y: f32, blur: f32, spread: f32, color: Oklch) -> Layer {
    Layer {
        x: 0.0,
        y,
        blur,
        spread,
        color,
        inset: true,
    }
}

macro_rules! depth {
    ($($name:ident),* $(,)?) => {
        /// The CSS layers of one theme's elevation tokens.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Tokens { $(pub $name: &'static [Layer],)* }

        /// One theme's elevation tokens as Rune shadows, outermost CSS layer first.
        #[derive(Clone, Debug, PartialEq)]
        pub struct Depth { $(pub $name: Vec<Shadow>,)* }

        impl Tokens {
            pub fn resolve(&self) -> Depth {
                Depth { $($name: self.$name.iter().map(Layer::shadow).collect(),)* }
            }

            /// Every token by its Rust name (the CSS name after `elev-`, `_` for `-`).
            pub fn entries(&self) -> [(&'static str, &'static [Layer]); 12] {
                [$((stringify!($name), self.$name),)*]
            }
        }

        impl Depth {
            /// The same tokens by name, in declaration order (for the gallery).
            pub fn entries(&self) -> [(&'static str, &[Shadow]); 12] {
                [$((stringify!($name), self.$name.as_slice()),)*]
            }
        }
    };
}

depth!(
    card,
    raised,
    raised_hover,
    pressed,
    well,
    knob,
    key,
    accent,
    selected,
    soft,
    float,
    panel
);

const fn ok(l: f32, c: f32, h: f32) -> Oklch {
    Oklch::new(l, c, h)
}

const WHITE: Oklch = ok(1.0, 0.0, 0.0);
const BLACK: Oklch = ok(0.0, 0.0, 0.0);
/// Light theme ink at three chromas: the CSS uses all three for its shadows.
const INK: Oklch = ok(0.147, 0.004, 49.25);
const INK_10: Oklch = ok(0.147, 0.01, 50.0);
const INK_20: Oklch = ok(0.147, 0.02, 50.0);
const BLUE_400: Oklch = ok(0.623, 0.214, 259.815);
const BLUE_600: Oklch = ok(0.488, 0.243, 264.376);
const BLUE_700: Oklch = ok(0.424, 0.199, 265.638);
/// `var(--hairline)` in each theme.
const HAIRLINE_LIGHT: Oklch = INK.alpha(0.08);
const HAIRLINE_DARK: Oklch = WHITE.alpha(0.08);

/// `:root` (`globals.css:222`). Light surfaces need far less shadow than dark ones.
pub const LIGHT: Tokens = Tokens {
    card: &[
        out(0.0, 0.0, 1.0, INK.alpha(0.07)),
        out(1.0, 2.0, 0.0, INK_10.alpha(0.05)),
        out(6.0, 16.0, -10.0, INK_20.alpha(0.12)),
    ],
    raised: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.7)),
        out(0.0, 0.0, 1.0, INK.alpha(0.1)),
        out(1.0, 2.0, 0.0, INK_10.alpha(0.08)),
    ],
    raised_hover: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.7)),
        out(0.0, 0.0, 1.0, INK.alpha(0.13)),
        out(2.0, 5.0, -1.0, INK_10.alpha(0.12)),
    ],
    pressed: &[
        ins(1.0, 2.0, 0.0, INK_10.alpha(0.12)),
        out(0.0, 0.0, 1.0, INK.alpha(0.1)),
    ],
    well: &[
        ins(1.0, 2.0, 0.0, INK_10.alpha(0.06)),
        ins(0.0, 0.0, 1.0, INK.alpha(0.09)),
    ],
    knob: &[
        out(0.0, 0.0, 0.5, INK_10.alpha(0.12)),
        out(1.0, 3.0, 0.0, INK_10.alpha(0.2)),
    ],
    key: &[
        ins(-1.0, 0.0, 0.0, INK.alpha(0.16)),
        out(0.0, 0.0, 0.5, INK.alpha(0.08)),
    ],
    accent: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.22)),
        out(0.0, 0.0, 1.0, BLUE_700.alpha(0.6)),
        out(1.0, 2.0, 0.0, BLUE_700.alpha(0.25)),
        out(4.0, 12.0, -4.0, BLUE_600.alpha(0.45)),
    ],
    selected: &[
        out(0.0, 0.0, 1.0, INK.alpha(0.05)),
        out(1.0, 2.0, 0.0, INK_10.alpha(0.06)),
    ],
    soft: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.7)),
        out(0.0, 0.0, 1.0, INK.alpha(0.06)),
        out(1.0, 2.0, 0.0, INK_10.alpha(0.07)),
        out(2.0, 8.0, -3.0, INK_10.alpha(0.1)),
    ],
    float: &[
        out(0.0, 0.0, 1.0, HAIRLINE_LIGHT),
        out(12.0, 32.0, -10.0, INK_20.alpha(0.18)),
        out(2.0, 6.0, 0.0, INK_10.alpha(0.06)),
    ],
    panel: &[
        out(0.0, 0.0, 1.0, HAIRLINE_LIGHT),
        out(24.0, 60.0, -14.0, INK_20.alpha(0.28)),
        out(4.0, 14.0, 0.0, INK_10.alpha(0.08)),
    ],
};

/// `.dark` (`globals.css:298`). Light comes from above: a faint top edge and a dark contact shadow.
pub const DARK: Tokens = Tokens {
    card: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.045)),
        out(0.0, 0.0, 1.0, WHITE.alpha(0.06)),
        out(1.0, 2.0, 0.0, BLACK.alpha(0.25)),
        out(8.0, 24.0, -12.0, BLACK.alpha(0.45)),
    ],
    raised: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.08)),
        out(0.0, 0.0, 1.0, WHITE.alpha(0.07)),
        out(1.0, 2.0, 0.0, BLACK.alpha(0.35)),
        out(2.0, 6.0, -2.0, BLACK.alpha(0.3)),
    ],
    raised_hover: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.1)),
        out(0.0, 0.0, 1.0, WHITE.alpha(0.1)),
        out(1.0, 2.0, 0.0, BLACK.alpha(0.4)),
        out(4.0, 10.0, -3.0, BLACK.alpha(0.4)),
    ],
    pressed: &[
        ins(1.0, 3.0, 0.0, BLACK.alpha(0.35)),
        out(0.0, 0.0, 1.0, WHITE.alpha(0.06)),
    ],
    well: &[
        ins(1.0, 2.0, 0.0, BLACK.alpha(0.3)),
        ins(0.0, 0.0, 1.0, WHITE.alpha(0.07)),
        out(1.0, 0.0, 0.0, WHITE.alpha(0.03)),
    ],
    knob: &[
        out(0.0, 0.0, 0.5, BLACK.alpha(0.3)),
        out(1.0, 3.0, 0.0, BLACK.alpha(0.45)),
    ],
    key: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.06)),
        ins(-1.0, 0.0, 0.0, BLACK.alpha(0.45)),
        out(0.0, 0.0, 0.5, BLACK.alpha(0.3)),
    ],
    accent: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.35)),
        out(0.0, 0.0, 1.0, BLUE_400.alpha(0.5)),
        out(1.0, 2.0, 0.0, BLACK.alpha(0.4)),
        out(4.0, 14.0, -4.0, BLUE_400.alpha(0.45)),
    ],
    selected: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.05)),
        out(1.0, 2.0, 0.0, BLACK.alpha(0.2)),
    ],
    soft: &[
        ins(1.0, 0.0, 0.0, WHITE.alpha(0.07)),
        out(0.0, 0.0, 1.0, WHITE.alpha(0.05)),
        out(1.0, 2.0, 0.0, BLACK.alpha(0.3)),
        out(4.0, 12.0, -4.0, BLACK.alpha(0.3)),
    ],
    float: &[
        out(0.0, 0.0, 1.0, HAIRLINE_DARK),
        out(12.0, 32.0, -8.0, BLACK.alpha(0.45)),
        out(2.0, 6.0, 0.0, BLACK.alpha(0.2)),
    ],
    panel: &[
        out(0.0, 0.0, 1.0, HAIRLINE_DARK),
        out(24.0, 64.0, -12.0, BLACK.alpha(0.55)),
        out(4.0, 16.0, 0.0, BLACK.alpha(0.25)),
    ],
};

#[cfg(test)]
mod tests {
    use super::super::css;
    use super::*;

    fn check(tokens: &Tokens, dark: bool) {
        let sheet = css::Sheet::parse(css::GLOBALS);
        for (name, layers) in tokens.entries() {
            let css = sheet.shadows(&format!("elev-{}", name.replace('_', "-")), dark);
            assert_eq!(layers.len(), css.len(), "--elev-{name}: layer count");
            for (i, (ours, theirs)) in layers.iter().zip(&css).enumerate() {
                assert_eq!(
                    ours,
                    theirs,
                    "--elev-{name} layer {i} ({})",
                    if dark { "dark" } else { "light" }
                );
            }
        }
    }

    #[test]
    fn light_depth_equals_the_css() {
        check(&LIGHT, false);
    }

    #[test]
    fn dark_depth_equals_the_css() {
        check(&DARK, true);
    }

    #[test]
    fn every_elevation_token_is_here() {
        let sheet = css::Sheet::parse(css::GLOBALS);
        let ours: Vec<String> = LIGHT
            .entries()
            .iter()
            .map(|(n, _)| format!("elev-{}", n.replace('_', "-")))
            .collect();
        let theirs = sheet.names_with_prefix("elev-");
        assert_eq!(theirs.len(), 12);
        for name in theirs {
            assert!(ours.contains(&name), "--{name} has no Rust token");
        }
    }

    #[test]
    fn layers_become_rune_shadows_in_order() {
        let depth = DARK.resolve();
        assert_eq!(depth.well.len(), 3);
        let top = depth.well[0];
        assert!(top.inset && top.y == 1.0 && top.blur == 2.0 && top.spread == 0.0);
        let ring = depth.well[1];
        assert!(ring.inset && ring.spread == 1.0 && ring.blur == 0.0);
        assert!(!depth.well[2].inset);
    }
}
