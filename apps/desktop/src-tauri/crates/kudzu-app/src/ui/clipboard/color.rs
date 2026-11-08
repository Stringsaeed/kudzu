//! The colour preview's maths (`cssToRgb` and `formats` in clip-content.tsx). The web parsed a
//! colour by painting it into a canvas pixel and reading it back, so values arrive as 8-bit
//! sRGB, clipped to the gamut; [`parse`] does the same for every CSS form Kudzu records
//! (`kudzu_core::clipboard::parse_color`): hex, `rgb()`, `hsl()`, `hwb()`, `lab()`, `lch()`,
//! `oklab()` and `oklch()`, comma or space separated, with an optional alpha.

use rune::Color;

/// 0–1 sRGB channels and alpha, quantized as a canvas stores them.
pub type Rgba = [f64; 4];

/// The rows and contrast figures the preview shows.
#[derive(Debug, Clone, PartialEq)]
pub struct Formats {
    pub values: [(&'static str, String); 4],
    pub on_white: String,
    pub on_black: String,
    pub light: bool,
}

fn srgb_to_linear(c: f64) -> f64 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

/// A number with an optional unit: `%` scales by `pct` (the value 100% stands for), angles turn
/// into degrees.
fn number(s: &str, pct: f64) -> Option<f64> {
    let s = s.trim();
    if s == "none" {
        return Some(0.0);
    }
    if let Some(v) = s.strip_suffix('%') {
        return v.parse::<f64>().ok().map(|v| v / 100.0 * pct);
    }
    for (unit, k) in [("deg", 1.0), ("grad", 0.9), ("rad", 180.0 / std::f64::consts::PI), ("turn", 360.0)] {
        if let Some(v) = s.strip_suffix(unit) {
            return v.parse::<f64>().ok().map(|v| v * k);
        }
    }
    s.parse::<f64>().ok()
}

fn hex(h: &str) -> Option<Rgba> {
    let digit = |i: usize, n: usize| u8::from_str_radix(h.get(i..i + n)?, 16).ok();
    let (r, g, b, a) = match h.len() {
        3 | 4 => {
            let d = |i| digit(i, 1).map(|v| f64::from(v * 17));
            (d(0)?, d(1)?, d(2)?, if h.len() == 4 { d(3)? } else { 255.0 })
        }
        6 | 8 => {
            let d = |i| digit(i, 2).map(f64::from);
            (d(0)?, d(2)?, d(4)?, if h.len() == 8 { d(6)? } else { 255.0 })
        }
        _ => return None,
    };
    Some([r / 255.0, g / 255.0, b / 255.0, a / 255.0])
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [f64; 3] {
    let f = |n: f64| {
        let k = (n + h / 30.0).rem_euclid(12.0);
        let a = s * l.min(1.0 - l);
        l - a * (-1.0f64).max((k - 3.0).min(9.0 - k).min(1.0))
    };
    [f(0.0), f(8.0), f(4.0)]
}

fn oklab_to_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3,
        -1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3,
        -0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3,
    ]
    .map(linear_to_srgb)
}

/// CIE Lab (D50) to sRGB, as CSS Color 4 converts it.
fn lab_to_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let (k, e) = (24389.0 / 27.0, 216.0 / 24389.0);
    let f1 = (l + 16.0) / 116.0;
    let f0 = a / 500.0 + f1;
    let f2 = f1 - b / 200.0;
    let x = if f0.powi(3) > e { f0.powi(3) } else { (116.0 * f0 - 16.0) / k };
    let y = if l > k * e { ((l + 16.0) / 116.0).powi(3) } else { l / k };
    let z = if f2.powi(3) > e { f2.powi(3) } else { (116.0 * f2 - 16.0) / k };
    let (x, y, z) = (x * 0.3457 / 0.3585, y, z * (1.0 - 0.3457 - 0.3585) / 0.3585);
    // D50 → D65 (Bradford), then XYZ → linear sRGB.
    let xd = 0.955473421488075 * x - 0.02309845494876471 * y + 0.06325924320057072 * z;
    let yd = -0.0283697093338637 * x + 1.0099953980813041 * y + 0.021041441191917323 * z;
    let zd = 0.012314014864481998 * x - 0.020507649298898964 * y + 1.330365926242124 * z;
    [
        3.2409699419045226 * xd - 1.537383177570094 * yd - 0.4986107602930034 * zd,
        -0.9692436362808796 * xd + 1.8759675015077202 * yd + 0.04155505740717559 * zd,
        0.05563007969699366 * xd - 0.20397695888897652 * yd + 1.0569715142428786 * zd,
    ]
    .map(linear_to_srgb)
}

/// A CSS colour as 8-bit sRGB (0–1) and alpha, as a canvas reads it back; `None` when it isn't one.
pub fn parse(color: &str) -> Option<Rgba> {
    let c = color.trim().to_ascii_lowercase();
    let raw = if let Some(h) = c.strip_prefix('#') {
        hex(h)?
    } else {
        let open = c.find('(')?;
        let name = &c[..open];
        let inner = c[open + 1..].strip_suffix(')')?;
        let (main, alpha) = match inner.split_once('/') {
            Some((m, a)) => (m, Some(a)),
            None => (inner, None),
        };
        let mut args: Vec<&str> = main.split(|ch: char| ch == ',' || ch.is_whitespace()).filter(|s| !s.is_empty()).collect();
        let mut alpha = alpha.map(str::trim);
        if alpha.is_none() && args.len() == 4 {
            alpha = args.pop();
        }
        if args.len() != 3 {
            return None;
        }
        let a = alpha.map_or(Some(1.0), |a| number(a, 1.0))?.clamp(0.0, 1.0);
        let rgb = match name {
            "rgb" | "rgba" => {
                let v = |i: usize| number(args[i], 255.0).map(|v| v / 255.0);
                [v(0)?, v(1)?, v(2)?]
            }
            "hsl" | "hsla" => hsl_to_rgb(number(args[0], 1.0)?, number(args[1], 1.0)?, number(args[2], 1.0)?),
            "hwb" => {
                let (w, b) = (number(args[1], 1.0)?, number(args[2], 1.0)?);
                if w + b >= 1.0 {
                    let g = w / (w + b);
                    [g, g, g]
                } else {
                    hsl_to_rgb(number(args[0], 1.0)?, 1.0, 0.5).map(|c| c * (1.0 - w - b) + w)
                }
            }
            "lab" => lab_to_srgb(number(args[0], 100.0)?, number(args[1], 125.0)?, number(args[2], 125.0)?),
            "lch" => {
                let (l, ch, h) = (number(args[0], 100.0)?, number(args[1], 150.0)?, number(args[2], 1.0)?.to_radians());
                lab_to_srgb(l, ch * h.cos(), ch * h.sin())
            }
            "oklab" => oklab_to_srgb(number(args[0], 1.0)?, number(args[1], 0.4)?, number(args[2], 0.4)?),
            "oklch" => {
                let (l, ch, h) = (number(args[0], 1.0)?, number(args[1], 0.4)?, number(args[2], 1.0)?.to_radians());
                oklab_to_srgb(l, ch * h.cos(), ch * h.sin())
            }
            _ => return None,
        };
        [rgb[0], rgb[1], rgb[2], a]
    };
    // The canvas round trip: clip to the gamut, then 8 bits per channel.
    Some(raw.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() / 255.0))
}

/// The colour for drawing (Rune colours are linear sRGB).
pub fn to_color([r, g, b, a]: Rgba) -> Color {
    Color { r: srgb_to_linear(r) as f32, g: srgb_to_linear(g) as f32, b: srgb_to_linear(b) as f32, a: a as f32 }
}

/// `toFixed` as JavaScript writes it (half away from zero on the decimal value).
fn fixed(v: f64, digits: usize) -> String {
    kudzu_core::client::js::to_fixed(v, digits)
}

/// HEX, RGB, HSL and OKLCH strings, the WCAG contrast against white and black text, and whether
/// the swatch is light (dark ink on it).
pub fn formats([r, g, b, a]: Rgba) -> Formats {
    let to255 = |v: f64| (v * 255.0).round() as i64;
    let mut hex = format!("#{:02x}{:02x}{:02x}", to255(r), to255(g), to255(b));
    if a < 1.0 {
        hex.push_str(&format!("{:02x}", to255(a)));
    }
    let hex = hex.to_uppercase();
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
    let mut h = 0.0;
    if d != 0.0 {
        // JavaScript's % keeps the dividend's sign.
        h = if max == r { ((g - b) / d) % 6.0 } else if max == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    }
    let h = ((h * 60.0 + 360.0).round() as i64).rem_euclid(360);
    let (lr, lg, lb) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
    let ll = (0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb).cbrt();
    let mm = (0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb).cbrt();
    let ss = (0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb).cbrt();
    let ok = [
        0.2104542553 * ll + 0.793617785 * mm - 0.0040720468 * ss,
        1.9779984951 * ll - 2.428592205 * mm + 0.4505937099 * ss,
        0.0259040371 * ll + 0.7827717662 * mm - 0.808675766 * ss,
    ];
    let chroma = ok[1].hypot(ok[2]);
    let hue = (ok[2].atan2(ok[1]).to_degrees() + 360.0) % 360.0;
    let alpha = if a < 1.0 { format!(" / {}%", (a * 100.0).round() as i64) } else { String::new() };
    let lum = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
    let hue_text = if chroma < 0.0005 { "0".to_string() } else { fixed(hue, 1) };
    Formats {
        values: [
            ("HEX", hex),
            ("RGB", format!("rgb({} {} {}{alpha})", to255(r), to255(g), to255(b))),
            ("HSL", format!("hsl({h} {}% {}%{alpha})", (s * 100.0).round() as i64, (l * 100.0).round() as i64)),
            ("OKLCH", format!("oklch({} {} {hue_text}{alpha})", fixed(ok[0], 3), fixed(chroma, 3))),
        ],
        on_white: fixed(1.05 / (lum + 0.05), 2),
        on_black: fixed((lum + 0.05) / 0.05, 2),
        light: lum > 0.4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_a_canvas() {
        assert_eq!(parse("#5b7cfa").map(formats).unwrap().values[0].1, "#5B7CFA");
        let f = formats(parse("rgb(34, 197, 94)").unwrap());
        assert_eq!(f.values[0].1, "#22C55E");
        assert_eq!(f.values[1].1, "rgb(34 197 94)");
        assert_eq!(f.values[2].1, "hsl(142 71% 45%)");
        assert!(f.light);
        let half = formats(parse("#ff000080").unwrap());
        assert_eq!(half.values[1].1, "rgb(255 0 0 / 50%)");
        assert!(parse("oklch(0.72 0.16 65)").is_some());
        assert!(parse("hsl(200deg 50% 50% / 0.5)").is_some());
        assert!(parse("nope").is_none());
        let white = formats(parse("#fff").unwrap());
        assert_eq!(white.on_black, "21.00");
        assert_eq!(white.values[3].1, "oklch(1.000 0.000 0)");
    }
}
