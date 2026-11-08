//! Kudzu's design system for Rune: the tokens from `app/globals.css` (generated into
//! [`tokens`]), the type scale Tailwind gave the web UI, and the utilities globals.css defines
//! (`sheen`, `press`, the `shadow-*` elevations). Screens read colours from [`Theme::t`] by their
//! CSS names (`t.muted_foreground` is `text-muted-foreground`), so a port reads like its JSX.

#[allow(dead_code)] // generated: every token globals.css defines
pub mod tokens;

use std::sync::OnceLock;

use rune::{Color, El, Shadow, TextStyle};

pub use tokens::{Tokens, MOTION_FAST, MOTION_MODERATE, MOTION_SLOW, RADIUS};

/// `font-sans`: San Francisco, which Rune loads from the system with Core Text's optical sizes
/// and tracking (the web UI asked for `-apple-system`).
pub const SANS: &str = "System Font";
/// `font-mono`: Paper Mono, bundled (OFL), as in the web build.
pub const MONO: &str = "Paper Mono";
pub static FONTS: &[&[u8]] = &[include_bytes!("../../../assets/fonts/PaperMono-Variable.ttf")];

/// Tailwind radii from `--radius` (10 pt): `rounded-sm` … `rounded-4xl`.
#[allow(dead_code)] // the whole Tailwind scale, used or not
pub mod radius {
    use super::RADIUS;
    pub const SM: f32 = RADIUS * 0.6;
    pub const MD: f32 = RADIUS * 0.8;
    pub const LG: f32 = RADIUS;
    pub const XL: f32 = RADIUS * 1.4;
    pub const XXL: f32 = RADIUS * 1.8;
    pub const XXXL: f32 = RADIUS * 2.2;
    pub const XXXXL: f32 = RADIUS * 2.6;
    pub const FULL: f32 = 9999.0;
}

/// Font weights by Tailwind name.
pub mod weight {
    pub const NORMAL: f32 = 400.0;
    pub const MEDIUM: f32 = 500.0;
    pub const SEMIBOLD: f32 = 600.0;
    pub const BOLD: f32 = 700.0;
}

/// Tailwind's type scale as the web UI used it (size / line height, in points).
#[allow(dead_code)] // the whole Tailwind scale, used or not
pub mod text {
    use rune::{Family, TextStyle};

    use super::weight;

    const fn sans(size: f32, lh: f32) -> TextStyle {
        TextStyle::new(size, weight::NORMAL).family(Family::Sans).lh(lh)
    }
    /// `text-[10px]`
    pub const XXS: TextStyle = sans(10.0, 14.0);
    /// `text-[11px]`
    pub const XS2: TextStyle = sans(11.0, 16.0);
    /// `text-xs`
    pub const XS: TextStyle = sans(12.0, 16.0);
    /// `text-[12.5px]`
    pub const XS3: TextStyle = sans(12.5, 18.0);
    /// `text-[13px] leading-5`: the body default.
    pub const BASE: TextStyle = sans(13.0, 20.0);
    /// `text-sm`
    pub const SM: TextStyle = sans(14.0, 20.0);
    /// `text-[15px]`
    pub const MD: TextStyle = sans(15.0, 22.0);
    /// `text-base`
    pub const LG: TextStyle = sans(16.0, 24.0);
    /// `text-lg`
    pub const XL: TextStyle = sans(18.0, 28.0);
    /// `text-xl`
    pub const XXL: TextStyle = sans(20.0, 28.0);
    /// `text-2xl`
    pub const XXXL: TextStyle = sans(24.0, 32.0);
    /// `text-3xl`
    pub const XXXXL: TextStyle = sans(30.0, 36.0);
    /// `font-mono text-[13px] leading-[1.7]`: the template editor.
    pub const MONO: TextStyle = TextStyle::new(13.0, weight::NORMAL).family(Family::Mono).lh(22.1);
    /// `font-mono text-xs`
    pub const MONO_XS: TextStyle = TextStyle::new(12.0, weight::NORMAL).family(Family::Mono).lh(16.0);
    /// `font-mono text-[11px]`
    pub const MONO_XS2: TextStyle = TextStyle::new(11.0, weight::NORMAL).family(Family::Mono).lh(16.0);
}

/// Motion from globals.css. The web UI's two CSS springs (`--spring-soft` 1.5 % overshoot,
/// `--spring-pop` 10 %) become Rune springs of the same duration and bounce.
#[allow(dead_code)] // every motion token globals.css defines
pub mod motion {
    use rune::Spring;

    use super::{MOTION_FAST, MOTION_MODERATE, MOTION_SLOW};

    /// `--motion-fast` (80 ms): hover tints, press-down.
    pub fn fast() -> Spring {
        Spring::new(MOTION_FAST, 0.0)
    }
    /// `--motion-moderate` (160 ms) on `--spring-soft`.
    pub fn moderate() -> Spring {
        Spring::new(MOTION_MODERATE, 0.05)
    }
    /// `--motion-slow` (240 ms) on `--spring-soft`: things that travel.
    pub fn soft() -> Spring {
        Spring::new(MOTION_SLOW, 0.05)
    }
    /// `--motion-slow` on `--spring-pop`: things that land (press release, checks, pops).
    pub fn pop() -> Spring {
        Spring::new(MOTION_SLOW, 0.3)
    }
}

/// Everything a view needs to draw Kudzu in one appearance.
pub struct Theme {
    pub dark: bool,
    /// The tokens as `.dark` / `:root` define them.
    pub t: Tokens,
    /// `--sidebar` over native vibrancy (`.vibrant`): translucent, so the material shows.
    pub sidebar_vibrant: Color,
}

static THEMES: OnceLock<[Theme; 2]> = OnceLock::new();

/// The theme for an appearance. Built once per process.
pub fn theme(dark: bool) -> &'static Theme {
    let themes = THEMES.get_or_init(|| [Theme::build(false), Theme::build(true)]);
    &themes[dark as usize]
}

impl Theme {
    fn build(dark: bool) -> Theme {
        let t = tokens::build(dark, false);
        let sidebar_vibrant = tokens::build(dark, true).sidebar;
        Theme { dark, t, sidebar_vibrant }
    }

    /// `color-mix(in oklch, a p%, b)`: `a` at `p` (0–1) with `b`, as CSS mixes it: premultiplied
    /// lightness and chroma, hue along the shorter arc, and an achromatic side taking the
    /// other's hue.
    pub fn mix(a: Color, b: Color, p: f32) -> Color {
        let lch = |c: Color| {
            let [l, x, y, alpha] = c.to_oklab();
            let chroma = (x * x + y * y).sqrt();
            let hue = if chroma < 1e-4 { f32::NAN } else { y.atan2(x).to_degrees().rem_euclid(360.0) };
            (l, chroma, hue, alpha)
        };
        let (la, ca, ha, aa) = lch(a);
        let (lb, cb, hb, ab) = lch(b);
        let (ha, hb) = match (ha.is_nan(), hb.is_nan()) {
            (true, true) => (0.0, 0.0),
            (true, false) => (hb, hb),
            (false, true) => (ha, ha),
            _ => (ha, hb),
        };
        let q = 1.0 - p;
        let alpha = aa * p + ab * q;
        if alpha <= 0.0 {
            return Color::TRANSPARENT;
        }
        // Premultiplied interpolation, as CSS Color 4 specifies.
        let l = (la * aa * p + lb * ab * q) / alpha;
        let c = (ca * aa * p + cb * ab * q) / alpha;
        let mut d = hb - ha;
        if d > 180.0 {
            d -= 360.0;
        } else if d < -180.0 {
            d += 360.0;
        }
        let h = (ha + d * q).to_radians();
        Color::from_oklab([l, c * h.cos(), c * h.sin(), alpha])
    }

    /// `color-mix(in oklch, c p%, transparent)`.
    pub fn tint(c: Color, p: f32) -> Color {
        c.alpha(c.a * p)
    }
}

/// Weight on a const style (TextStyle has no builder for it).
pub trait WeightOf {
    fn weight_of(self, w: f32) -> TextStyle;
}

impl WeightOf for TextStyle {
    fn weight_of(self, w: f32) -> TextStyle {
        TextStyle { weight: w, ..self }
    }
}

/// globals.css utilities on any element.
pub trait Utilities: Sized {
    /// `shadow-*`: every layer of an `--elev-*` token.
    fn elevate(self, layers: &[Shadow]) -> Self;
    /// `sheen` (strong = `sheen-strong`): the top-lit gradient over raised actions.
    fn sheen(self, strong: bool) -> Self;
    /// `press`: sink to `scale` while held, spring back with `--spring-pop` on release.
    /// (Named apart from `El::press`, Rune's pressed tint.)
    fn press_sink(self, scale: f32) -> Self;
}

impl Utilities for El {
    fn elevate(self, layers: &[Shadow]) -> El {
        layers.iter().fold(self, |e, s| e.box_shadow(*s))
    }

    fn sheen(self, strong: bool) -> El {
        let (a, end) = if strong { (0.2, 0.7) } else { (0.1, 0.6) };
        let white = Color::WHITE;
        self.linear_gradient(std::f32::consts::PI, [(0.0, white.alpha(a)), (end, white.alpha(0.0)), (1.0, white.alpha(0.0))])
    }

    fn press_sink(self, scale: f32) -> El {
        self.press_scale(scale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_mix_matches_css_endpoints_and_keeps_hue_from_the_chromatic_side() {
        let blue = rune::oklch(0.5, 0.2, 260.0);
        let white = Color::WHITE;
        let same = |x: Color, y: Color| (x.r - y.r).abs() < 1e-3 && (x.g - y.g).abs() < 1e-3 && (x.b - y.b).abs() < 1e-3;
        assert!(same(Theme::mix(blue, white, 1.0), blue));
        assert!(same(Theme::mix(blue, white, 0.0), white));
        let half = Theme::mix(blue, white, 0.5).to_oklab();
        let hue = half[2].atan2(half[1]).to_degrees().rem_euclid(360.0);
        assert!((hue - 260.0).abs() < 0.5, "hue {hue}");
        assert_eq!(Theme::mix(Color::TRANSPARENT, Color::TRANSPARENT, 0.5), Color::TRANSPARENT);
    }
}
