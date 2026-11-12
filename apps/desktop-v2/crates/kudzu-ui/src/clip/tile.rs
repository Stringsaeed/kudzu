//! The leading visual of a clip row: a kind glyph on a tinted square, a colour swatch for
//! colours, a key for secrets. Thumbnails and the source app's badge arrive with the media
//! loading S59 owns; until then images and files show their kind glyph.

use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{col, icon, row, El, Icon, Pixels, SurfaceFit};

use super::content::loader::{Media, Want};

use crate::theme::{icons, Theme};

/// The glyph for a kind (`KIND_ICON` in `clip-tile.tsx`).
pub fn kind_icon(e: &ClipEntry) -> &'static Icon {
    match e.kind {
        ClipKind::Text | ClipKind::Color => &icons::ALIGN_LEFT,
        ClipKind::Code => &icons::BRACES,
        ClipKind::Link => &icons::GLOBE,
        ClipKind::Image => &icons::IMAGE,
        ClipKind::File if e.files.len() > 1 => &icons::FILES,
        ClipKind::File => &icons::FILE,
        ClipKind::Other => &icons::PACKAGE,
    }
}

/// `size` is 36 in the main list and 32 when compact.
pub fn tile(
    theme: &Theme,
    e: &ClipEntry,
    concealed: bool,
    active: bool,
    size: f32,
    media: Option<&Media>,
) -> El {
    let square = glyph_tile(theme, e, concealed, active, size, media);
    badge(theme, e, size, media, square)
}

fn glyph_tile(
    theme: &Theme,
    e: &ClipEntry,
    concealed: bool,
    active: bool,
    size: f32,
    media: Option<&Media>,
) -> El {
    let c = &theme.c;
    let radius = (size * 0.26).round();
    if let (ClipKind::Color, Some(css), false) = (e.kind, e.color.as_deref(), concealed) {
        if let Some(fill) = swatch(css) {
            return row()
                .size(size, size)
                .radius(radius)
                .fill(fill)
                .border(1.0, c.foreground.alpha(0.12));
        }
    }
    if !concealed {
        if let Some(px) = thumbnail(e, media) {
            return row()
                .size(size, size)
                .radius(radius)
                .clip_rounded(radius)
                .border(1.0, c.hairline)
                .child(col().size(size, size).pixels_fit(&px, SurfaceFit::Fit));
        }
    }
    let glyph = if concealed {
        &icons::KEY_ROUND
    } else {
        kind_icon(e)
    };
    let (fill, ink) = if active {
        (c.iris, c.iris_contrast)
    } else {
        (c.foreground.alpha(0.055), c.muted_foreground)
    };
    let mut square = row()
        .size(size, size)
        .center()
        .radius(radius)
        .fill(fill)
        .child(icon(glyph, if size >= 32.0 { 16.0 } else { 14.0 }).color(ink));
    if !active {
        square = square.border(1.0, c.hairline);
    }
    square
}

fn swatch(css: &str) -> Option<rune::Color> {
    let [r, g, b, a] = kudzu_core::color::parse(css)?;
    // sRGB transfer function: `Color` holds linear light.
    let d = |v: f64| {
        let v = v as f32;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(rune::Color {
        r: d(r),
        g: d(g),
        b: d(b),
        a: a as f32,
    })
}

/// The picture a row shows in place of a glyph: an image's thumbnail, a file's image
/// thumbnail or its Finder icon.
fn thumbnail(e: &ClipEntry, media: Option<&Media>) -> Option<Pixels> {
    let media = media?;
    match e.kind {
        ClipKind::Image => media.pixels(e.id, Want::Thumbnail),
        ClipKind::File if e.image.is_some() => media.pixels(e.id, Want::Thumbnail),
        ClipKind::File => media.pixels(e.id, Want::Icon(0)),
        _ => None,
    }
}

/// The source app's icon in the corner, like a Finder badge.
fn badge(theme: &Theme, e: &ClipEntry, size: f32, media: Option<&Media>, square: El) -> El {
    let px = e
        .source
        .as_ref()
        .and_then(|s| media.and_then(|m| m.app_pixels(&s.bundle_id)));
    let Some(px) = px else { return square };
    let _ = size;
    col().child(square).child(
        row()
            .absolute(size - 10.0, size - 10.0)
            .size(14.0, 14.0)
            .radius(5.0)
            .fill(theme.c.background)
            .padding(1.0)
            .child(col().size(12.0, 12.0).pixels_fit(&px, SurfaceFit::Fit)),
    )
}
