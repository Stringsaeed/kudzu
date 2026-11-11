//! Every colour token of the Tauri app's `app/globals.css`, light (`:root`, line 156) and dark
//! (`.dark`, line 254), as OKLCH with alpha. Values are copied literally; a `var(--x)` in the CSS
//! is the referenced token's value here, so the tables read as the browser resolves them.
//!
//! Alphas are CSS alphas: windows keep Rune's default sRGB blending, which is how they were
//! authored. `Oklch::color` converts to Rune's linear colour.

use rune::Color;

/// One CSS `oklch(l c h / a)` literal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
    pub a: f32,
}

impl Oklch {
    pub const fn new(l: f32, c: f32, h: f32) -> Self {
        Self { l, c, h, a: 1.0 }
    }

    pub const fn alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    pub fn color(self) -> Color {
        rune::oklcha(self.l, self.c, self.h, self.a)
    }
}

const fn ok(l: f32, c: f32, h: f32) -> Oklch {
    Oklch::new(l, c, h)
}

/// Declares the token list once: the OKLCH table type, the resolved palette and the name list.
macro_rules! palette {
    ($($name:ident),* $(,)?) => {
        /// The CSS values of one theme.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Tokens { $(pub $name: Oklch,)* }

        /// One theme's colours, ready to paint.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Palette { $(pub $name: Color,)* }

        impl Tokens {
            pub fn resolve(&self) -> Palette {
                Palette { $($name: self.$name.color(),)* }
            }

            /// Every token by its Rust name (the CSS name with `_` for `-`), in declaration order.
            pub fn entries(&self) -> Vec<(&'static str, Oklch)> {
                vec![$((stringify!($name), self.$name),)*]
            }
        }
    };
}

palette! {
    background, foreground, card, card_foreground, popover, popover_foreground,
    primary, primary_foreground, secondary, secondary_foreground, muted, muted_foreground,
    accent, accent_foreground, destructive, border, input, ring,
    chart_1, chart_2, chart_3, chart_4, chart_5,
    sidebar, sidebar_foreground, sidebar_primary, sidebar_primary_foreground, sidebar_accent,
    sidebar_accent_foreground, sidebar_border, sidebar_ring,
    surface, surface_raised, hairline, faint, iris, iris_soft, iris_foreground, iris_contrast,
    chip_time, chip_clipboard, chip_cursor, chip_form, chip_error, chip_neutral,
    success, warning, favorite,
    sidebar_vibrant,
}

// Shared literals. The CSS repeats them; naming them keeps the two tables readable.
const WHITE: Oklch = ok(1.0, 0.0, 0.0);
const INK: Oklch = ok(0.147, 0.004, 49.25);
const PAPER: Oklch = ok(0.985, 0.001, 106.423);
const STONE_100: Oklch = ok(0.97, 0.001, 106.424);
const STONE_200: Oklch = ok(0.923, 0.003, 48.717);
const STONE_400: Oklch = ok(0.709, 0.01, 56.259);
const STONE_500: Oklch = ok(0.553, 0.013, 58.071);
const STONE_800: Oklch = ok(0.268, 0.007, 34.298);
const STONE_850: Oklch = ok(0.216, 0.006, 56.043);
const BLUE_200: Oklch = ok(0.809, 0.105, 251.813);
const BLUE_400: Oklch = ok(0.623, 0.214, 259.815);
const BLUE_500: Oklch = ok(0.546, 0.245, 262.881);
const BLUE_600: Oklch = ok(0.488, 0.243, 264.376);
const BLUE_700: Oklch = ok(0.424, 0.199, 265.638);
const BLUE_50: Oklch = ok(0.97, 0.014, 254.604);

/// `:root` (`globals.css:179`).
pub const LIGHT: Tokens = Tokens {
    background: WHITE,
    foreground: INK,
    card: WHITE,
    card_foreground: INK,
    popover: WHITE,
    popover_foreground: INK,
    primary: BLUE_600,
    primary_foreground: BLUE_50,
    secondary: ok(0.967, 0.001, 286.375),
    secondary_foreground: ok(0.21, 0.006, 285.885),
    muted: STONE_100,
    muted_foreground: STONE_500,
    accent: STONE_100,
    accent_foreground: STONE_850,
    destructive: ok(0.577, 0.245, 27.325),
    border: STONE_200,
    input: STONE_200,
    ring: STONE_400,
    chart_1: BLUE_200,
    chart_2: BLUE_400,
    chart_3: BLUE_500,
    chart_4: BLUE_600,
    chart_5: BLUE_700,
    sidebar: PAPER,
    sidebar_foreground: INK,
    sidebar_primary: BLUE_500,
    sidebar_primary_foreground: BLUE_50,
    sidebar_accent: STONE_100,
    sidebar_accent_foreground: STONE_850,
    sidebar_border: STONE_200,
    sidebar_ring: STONE_400,
    surface: WHITE,
    surface_raised: WHITE,
    hairline: INK.alpha(0.08),
    faint: ok(0.65, 0.01, 56.0),
    iris: BLUE_600,
    iris_soft: BLUE_600.alpha(0.1),
    iris_foreground: BLUE_600,
    iris_contrast: BLUE_50,
    chip_time: ok(0.47, 0.13, 235.0),
    chip_clipboard: ok(0.47, 0.12, 158.0),
    chip_cursor: BLUE_600,
    chip_form: ok(0.52, 0.13, 62.0),
    chip_error: ok(0.52, 0.19, 24.0),
    chip_neutral: INK,
    success: ok(0.6, 0.15, 155.0),
    warning: ok(0.68, 0.15, 70.0),
    favorite: ok(0.72, 0.16, 65.0),
    // `.vibrant` (`globals.css:247`): the sidebar over the window's material.
    sidebar_vibrant: PAPER.alpha(0.72),
};

/// `.dark` (`globals.css:254`).
pub const DARK: Tokens = Tokens {
    background: INK,
    foreground: PAPER,
    card: STONE_850,
    card_foreground: PAPER,
    popover: STONE_850,
    popover_foreground: PAPER,
    primary: BLUE_700,
    primary_foreground: BLUE_50,
    secondary: ok(0.274, 0.006, 286.033),
    secondary_foreground: ok(0.985, 0.0, 0.0),
    muted: STONE_800,
    muted_foreground: STONE_400,
    accent: STONE_800,
    accent_foreground: PAPER,
    destructive: ok(0.704, 0.191, 22.216),
    border: WHITE.alpha(0.1),
    input: WHITE.alpha(0.15),
    ring: STONE_500,
    chart_1: BLUE_200,
    chart_2: BLUE_400,
    chart_3: BLUE_500,
    chart_4: BLUE_600,
    chart_5: BLUE_700,
    sidebar: STONE_850,
    sidebar_foreground: PAPER,
    sidebar_primary: BLUE_400,
    sidebar_primary_foreground: BLUE_50,
    sidebar_accent: STONE_800,
    sidebar_accent_foreground: PAPER,
    sidebar_border: WHITE.alpha(0.1),
    sidebar_ring: STONE_500,
    surface: STONE_850,
    surface_raised: ok(0.245, 0.006, 56.0),
    hairline: WHITE.alpha(0.08),
    faint: ok(0.58, 0.012, 56.0),
    iris: BLUE_400,
    iris_soft: BLUE_400.alpha(0.18),
    iris_foreground: BLUE_200,
    iris_contrast: BLUE_50,
    chip_time: ok(0.8, 0.1, 220.0),
    chip_clipboard: ok(0.8, 0.12, 158.0),
    chip_cursor: BLUE_200,
    chip_form: ok(0.84, 0.11, 75.0),
    chip_error: ok(0.75, 0.16, 24.0),
    chip_neutral: PAPER,
    success: ok(0.76, 0.14, 158.0),
    warning: ok(0.8, 0.13, 75.0),
    favorite: ok(0.82, 0.14, 75.0),
    // `.vibrant.dark` (`globals.css:251`).
    sidebar_vibrant: STONE_850.alpha(0.55),
};

/// Theme-independent paint from `globals.css`'s base layer and utilities.
pub mod fixed {
    use super::{ok, Oklch, WHITE};

    /// Thin scroll bar thumb (`scrollbar-color`, `globals.css:369`).
    pub const SCROLLBAR: Oklch = ok(0.5, 0.0, 0.0).alpha(0.25);
    /// `sheen`: top-lit gradient over raised actions, top to bottom (`globals.css:393`).
    pub const SHEEN: [(f32, Oklch); 2] = [(0.0, WHITE.alpha(0.1)), (0.6, WHITE.alpha(0.0))];
    /// `sheen-strong` (`globals.css:396`).
    pub const SHEEN_STRONG: [(f32, Oklch); 2] = [(0.0, WHITE.alpha(0.2)), (0.7, WHITE.alpha(0.0))];
    /// `noise`: a 1 px dot on a 3 px grid (`globals.css:447`).
    pub const NOISE_DOT: Oklch = WHITE.alpha(0.025);
    pub const NOISE_GRID: f32 = 3.0;
    /// Tinted chips: tint and ring are the tone at these shares of opacity
    /// (`color-mix(in oklch, tone N%, transparent)`, `globals.css:493`).
    pub const CHIP_TINT: f32 = 0.13;
    pub const CHIP_RING: f32 = 0.24;
    pub const CHIP_RING_HOVER: f32 = 0.70;
}

#[cfg(test)]
mod tests {
    use super::super::css;
    use super::*;

    fn check(tokens: &Tokens, dark: bool) {
        let sheet = css::Sheet::parse(css::GLOBALS);
        for (name, value) in tokens.entries() {
            let css = match name {
                "sidebar_vibrant" => sheet.vibrant_sidebar(dark),
                _ => sheet.colour(&name.replace('_', "-"), dark),
            };
            assert_eq!(
                value,
                css,
                "--{name} ({})",
                if dark { "dark" } else { "light" }
            );
        }
    }

    #[test]
    fn light_tokens_equal_the_css() {
        check(&LIGHT, false);
    }

    #[test]
    fn dark_tokens_equal_the_css() {
        check(&DARK, true);
    }

    #[test]
    fn every_css_colour_token_is_here() {
        let sheet = css::Sheet::parse(css::GLOBALS);
        let ours: Vec<String> = LIGHT
            .entries()
            .iter()
            .map(|(n, _)| n.replace('_', "-"))
            .collect();
        for name in sheet.colour_names() {
            assert!(ours.contains(&name), "--{name} has no Rust token");
        }
    }

    #[test]
    fn fixed_paint_equals_the_css() {
        assert_eq!(fixed::SCROLLBAR, css::oklch("oklch(0.5 0 0 / 0.25)"));
        assert_eq!(fixed::SHEEN[0].1, css::oklch("oklch(1 0 0 / 0.1)"));
        assert_eq!(fixed::SHEEN_STRONG[0].1, css::oklch("oklch(1 0 0 / 0.2)"));
        assert_eq!(fixed::NOISE_DOT, css::oklch("oklch(1 0 0 / 0.025)"));
    }
}
