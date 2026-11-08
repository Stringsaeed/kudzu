//! `components/app/wordmark.tsx`: the Kudzu "Echo" wordmark (stepped, fading outlines on block
//! letters), from `public/brand/kudzu-wordmark-echo{,-small}.svg`, drawn in the text colour as
//! the web masks it with `bg-current`.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::sync::OnceLock;

use rune::{icon, Color, El, Icon};

/// `variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WordmarkVariant {
    /// 735 × 201.
    #[default]
    Full,
    /// The heavier cut for small sizes, 741 × 207.
    Small,
}

const FULL: &str = include_str!("../../../../../../public/brand/kudzu-wordmark-echo.svg");
const SMALL: &str = include_str!("../../../../../../public/brand/kudzu-wordmark-echo-small.svg");

/// An SVG file as a Rune icon: its paths inside a square view box as wide as the drawing, shifted
/// so its own view box starts at the top-left.
fn as_icon(svg: &'static str, name: &'static str) -> Icon {
    let vb = svg.split("viewBox=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("0 0 1 1");
    let n: Vec<f32> = vb.split_whitespace().filter_map(|v| v.parse().ok()).collect();
    let (x, y, w) = (n.first().copied().unwrap_or(0.0), n.get(1).copied().unwrap_or(0.0), n.get(2).copied().unwrap_or(1.0));
    let start = svg.find("</title>").map(|i| i + "</title>".len()).or_else(|| svg.find('>').map(|i| i + 1)).unwrap_or(0);
    let end = svg.rfind("</svg>").unwrap_or(svg.len());
    let body = format!(r#"<g fill="currentColor" transform="translate({} {})">{}</g>"#, -x, -y, &svg[start..end]);
    Icon { name, view: w, body: Box::leak(body.into_boxed_str()) }
}

fn glyph(v: WordmarkVariant) -> &'static Icon {
    static ICONS: OnceLock<[Icon; 2]> = OnceLock::new();
    let icons = ICONS.get_or_init(|| [as_icon(FULL, "kudzu-wordmark-echo"), as_icon(SMALL, "kudzu-wordmark-echo-small")]);
    &icons[(v == WordmarkVariant::Small) as usize]
}

/// The drawing's aspect ratio, width over height.
pub fn aspect(v: WordmarkVariant) -> f32 {
    match v {
        WordmarkVariant::Full => 735.0 / 201.0,
        WordmarkVariant::Small => 741.0 / 207.0,
    }
}

/// `<Wordmark variant className="w-* text-*" />`: `width` points wide, in `color`.
pub fn wordmark(v: WordmarkVariant, width: f32, color: Color) -> El {
    let h = width / aspect(v);
    // The icon is square; the box shows its top `h` points, where the drawing sits.
    rune::el()
        .role("img")
        .label("Kudzu")
        .size(width, h)
        .clip_rounded(0.0)
        .child(icon(glyph(v), width).color(color).a11y_hidden())
}
