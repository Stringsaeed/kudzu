//! Colour parsing and presentation for the clipboard colour card.
//!
//! CSS is parsed by `csscolorparser`. HEX, RGB, HSL and OKLCH strings, and the contrast
//! figures, use the same rounding as `clip-content.tsx`. Classification of a paste as a
//! colour stays in [`crate::clipboard::parse_color`].

/// 0–1 sRGB plus alpha, after an 8-bit canvas-style round-trip.
pub type Rgba = [f64; 4];

/// Copyable representations and WCAG contrast for one colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorCard {
    /// `(HEX, RGB, HSL, OKLCH)` in that order, each `(label, value)`.
    pub values: Vec<(String, String)>,
    /// Contrast of the colour against white, two decimal places.
    pub on_white: String,
    /// Contrast of the colour against black, two decimal places.
    pub on_black: String,
    /// `true` when relative luminance is above 0.4, so overlay text should be dark.
    pub light: bool,
}

/// Parse any CSS colour to 0–1 sRGB + alpha. Invalid CSS yields [`None`] (the canvas
/// trick used to fall back to black).
pub fn parse(css: &str) -> Option<Rgba> {
    let c = csscolorparser::parse(css).ok()?;
    let [r, g, b, a] = c.to_rgba8();
    Some([
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        f64::from(a) / 255.0,
    ])
}

/// HEX / RGB / HSL / OKLCH strings and contrast from 0–1 sRGB + alpha.
pub fn formats([r, g, b, a]: Rgba) -> ColorCard {
    let to255 = |v: f64| (v * 255.0).round() as u8;
    let hex_rgb = format!("#{:02X}{:02X}{:02X}", to255(r), to255(g), to255(b));
    let hex = if a < 1.0 {
        format!("{hex_rgb}{:02X}", to255(a))
    } else {
        hex_rgb
    };

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let s = if d == 0.0 {
        0.0
    } else {
        d / (1.0 - (2.0 * l - 1.0).abs())
    };
    let mut h = 0.0;
    if d != 0.0 {
        h = if max == r {
            ((g - b) / d) % 6.0
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
    }
    let h = ((h * 60.0 + 360.0).round() as i32).rem_euclid(360);

    let lr = srgb_to_linear(r);
    let lg = srgb_to_linear(g);
    let lb = srgb_to_linear(b);
    let lms_l = (0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb).cbrt();
    let lms_m = (0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb).cbrt();
    let lms_s = (0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb).cbrt();
    let ok_l = 0.2104542553 * lms_l + 0.793617785 * lms_m - 0.0040720468 * lms_s;
    let ok_a = 1.9779984951 * lms_l - 2.428592205 * lms_m + 0.4505937099 * lms_s;
    let ok_b = 0.0259040371 * lms_l + 0.7827717662 * lms_m - 0.808675766 * lms_s;
    let c = ok_a.hypot(ok_b);
    let ok_h = (ok_b.atan2(ok_a).to_degrees() + 360.0) % 360.0;
    let hue = if c < 0.0005 {
        "0".to_string()
    } else {
        to_fixed(ok_h, 1)
    };

    let alpha = if a < 1.0 {
        format!(" / {}%", (a * 100.0).round() as i32)
    } else {
        String::new()
    };
    let lum = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;

    ColorCard {
        values: vec![
            ("HEX".into(), hex),
            (
                "RGB".into(),
                format!("rgb({} {} {}{alpha})", to255(r), to255(g), to255(b)),
            ),
            (
                "HSL".into(),
                format!(
                    "hsl({h} {}% {}%{alpha})",
                    (s * 100.0).round() as i32,
                    (l * 100.0).round() as i32
                ),
            ),
            (
                "OKLCH".into(),
                format!(
                    "oklch({} {} {hue}{alpha})",
                    to_fixed(ok_l, 3),
                    to_fixed(c, 3)
                ),
            ),
        ],
        on_white: to_fixed(1.05 / (lum + 0.05), 2),
        on_black: to_fixed((lum + 0.05) / 0.05, 2),
        light: lum > 0.4,
    }
}

/// Parse a CSS colour and format it. [`None`] when the CSS is not a colour.
pub fn card(css: &str) -> Option<ColorCard> {
    parse(css).map(formats)
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_fixed(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}
