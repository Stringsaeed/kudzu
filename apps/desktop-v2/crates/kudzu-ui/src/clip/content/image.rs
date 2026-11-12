//! An image: the original on a checkerboard, falling back to its thumbnail.

use rune::{col, icon, El, SurfaceFit};

use super::loader::Want;
use super::{checkerboard, well, Body};
use crate::theme::{icons, Theme};

/// "Copied image, 640 by 480 pixels, from Safari."
pub fn alt(b: &Body) -> String {
    let mut s = String::from("Copied image");
    if let Some(i) = b.entry.image {
        s.push_str(&format!(", {} by {} pixels", i.width, i.height));
    }
    if let Some(source) = &b.entry.source {
        s.push_str(&format!(", from {}", source.name));
    }
    s
}

/// The aspect ratio of the box (never narrower than 0.9, as the Tauri preview).
pub fn box_ratio(b: &Body) -> f32 {
    b.entry
        .image
        .map_or(1.6, |i| i.width as f32 / (i.height.max(1) as f32))
        .max(0.9)
}

pub fn view(theme: &Theme, b: &Body) -> El {
    let c = &theme.c;
    let pixels = b
        .media
        .and_then(|m| m.first_ready(b.entry.id, &[Want::Image, Want::Thumbnail]));
    let mut frame = well(theme)
        .key("clip-image-body")
        .role("img")
        .label(alt(b))
        .padding(16.0)
        .center()
        .clip_rounded(14.0);
    frame = if b.large {
        frame.h(560.0)
    } else {
        frame.aspect_ratio(box_ratio(b)).min_h(160.0).max_h(420.0)
    };
    match pixels {
        Some(px) => frame.child(checkerboard(theme, 920.0, 640.0)).child(
            col()
                .key("clip-image-pixels")
                .w_full()
                .h_full()
                .pixels_fit(&px, SurfaceFit::Fit),
        ),
        None => frame.child(icon(&icons::IMAGE, 28.0).color(c.faint)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::clip;
    use kudzu_core::clipboard::{ClipKind, ClipSource, ImageSize};

    #[test]
    fn the_alt_text_names_size_and_source() {
        let mut e = clip("", 1, false);
        e.kind = ClipKind::Image;
        e.image = Some(ImageSize {
            width: 640,
            height: 480,
        });
        e.source = Some(ClipSource {
            bundle_id: "s".into(),
            name: "Safari".into(),
            path: None,
        });
        let b = Body {
            entry: &e,
            full: None,
            terms: &[],
            large: false,
            media: None,
            copied_value: None,
        };
        assert_eq!(alt(&b), "Copied image, 640 by 480 pixels, from Safari");
        assert!((box_ratio(&b) - 640.0 / 480.0).abs() < 1e-6);
        e.image = Some(ImageSize {
            width: 100,
            height: 400,
        });
        let b = Body {
            entry: &e,
            full: None,
            terms: &[],
            large: false,
            media: None,
            copied_value: None,
        };
        assert_eq!(box_ratio(&b), 0.9, "tall images do not make a tall box");
    }
}
