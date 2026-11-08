//! `components/app/clipboard/clip-tile.tsx`: the leading visual of a row (a thumbnail for
//! images, a swatch for colours, a kind glyph for everything else, with the source app's icon in
//! the corner like a Finder badge), plus `ClipImage` and the transparency checkerboard.

use kudzu_core::client::clipboard::is_secret;
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{el, icon, Color, El, Icon, Shadow, SurfaceFit};

use super::media::{self, Picture, Source};
use crate::ui::icons;
use crate::ui::kit::app_icon::app_icon;
use crate::ui::theme::{Theme, Utilities};

/// `KIND_ICON`.
pub fn kind_icon(kind: ClipKind) -> &'static Icon {
    match kind {
        ClipKind::Text | ClipKind::Color => &icons::TEXT_ALIGN_START,
        ClipKind::Code => &icons::BRACES,
        ClipKind::Link => &icons::GLOBE,
        ClipKind::Image => &icons::IMAGE,
        ClipKind::File => &icons::FILE,
        ClipKind::Other => &icons::PACKAGE,
    }
}

/// `shadow-[inset_0_0_0_1px_var(--hairline)]`.
pub fn inset_hairline(th: &Theme) -> Shadow {
    Shadow { spread: 1.0, color: th.t.hairline, inset: true, ..Shadow::default() }
}

/// The checkerboard behind transparent images: 7 pt squares of `foreground/7%` on every other
/// cell (`conic-gradient(…) bg-[length:14px_14px]`), clipped to `w`×`h`.
pub fn checkerboard(th: &Theme, w: f32, h: f32) -> El {
    let c = Theme::tint(th.t.foreground, 0.07);
    let (cols, rows) = ((w / 7.0).ceil() as usize, (h / 7.0).ceil() as usize);
    let mut board = el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).clip_soft(0.0).a11y_hidden().inert();
    for r in 0..rows.min(200) {
        for col in 0..cols.min(200) {
            // The conic gradient paints the top-left and bottom-right quarters of each tile.
            if (r + col) % 2 == 0 {
                board = board.child(el().absolute(col as f32 * 7.0, r as f32 * 7.0).size(7.0, 7.0).fill(c));
            }
        }
    }
    board
}

/// A picture over its fallback: the fallback shows until the picture decodes (and stays when it
/// can't), as `ClipImage`'s `onError` fallback did.
pub fn with_fallback(p: &Picture, fit: SurfaceFit, fallback: El) -> El {
    el().w_full()
        .h_full()
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(fallback).live_opacity(&p.fallback))
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).pixels_fit(&p.pixels, fit).live_opacity(&p.shown))
}

/// The thumbnail for an entry with one (images, and a single copied image file).
pub fn has_thumb(e: &ClipEntry) -> bool {
    e.kind == ClipKind::Image || (e.kind == ClipKind::File && e.image.is_some())
}

/// `ClipTile`: `size` pt square (36 comfortable, 32 compact), `active` when it's the focused row.
pub fn tile(th: &Theme, e: &ClipEntry, active: bool, size: f32, badge: bool) -> El {
    let t = &th.t;
    let r = (size * 0.26).round();
    let glyph = |i: &'static Icon| {
        let mut b = el().w_full().h_full().center().radius(r).child(icon(i, if size >= 32.0 { 16.0 } else { 14.0 }).color(if active { t.iris_contrast } else { t.muted_foreground }));
        b = if active {
            b.fill(t.iris).elevate(&t.elev_accent).sheen(true)
        } else {
            b.fill(Theme::tint(t.foreground, 0.055)).box_shadow(inset_hairline(th))
        };
        b.motion(rune::Spring::new(0.1, 0.0))
    };
    let body = match e.kind {
        ClipKind::Color if e.color.is_some() => {
            let fill = e.color.as_deref().and_then(super::color::parse).map_or(Color::TRANSPARENT, super::color::to_color);
            el().w_full()
                .h_full()
                .radius(r)
                .clip_rounded(r)
                .child(if fill.a < 1.0 { checkerboard(th, size, size) } else { el() })
                .child(
                    el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                        .radius(r)
                        .fill(fill)
                        .box_shadow(Shadow { spread: 1.0, color: Color::BLACK.alpha(0.12), inset: true, ..Shadow::default() })
                        .box_shadow(Shadow { y: 1.0, color: Color::WHITE.alpha(0.18), inset: true, ..Shadow::default() }),
                )
        }
        _ if has_thumb(e) => {
            let p = media::picture(Source::Thumb(e.id));
            el().w_full()
                .h_full()
                .radius(r)
                .clip_rounded(r)
                .box_shadow(inset_hairline(th))
                .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(checkerboard(th, size, size)).live_opacity(&p.transparent))
                .child(with_fallback(&p, SurfaceFit::Fill, glyph(&icons::IMAGE)))
        }
        ClipKind::File => {
            let many = e.files.len() > 1;
            let fallback = glyph(if many { &icons::FILES } else { &icons::FILE });
            match e.files.first() {
                Some(path) => with_fallback(&media::picture(Source::Icon(path.clone())), SurfaceFit::Fit, fallback),
                None => fallback,
            }
        }
        kind => glyph(if is_secret(e) { &icons::KEY_ROUND } else { kind_icon(kind) }),
    };
    let mut out = el().key("tile").size(size, size).radius(r).child(body.key("body")).a11y_hidden();
    if active && matches!(e.kind, ClipKind::Image | ClipKind::Color) {
        // `ring-2 ring-iris/70 ring-offset-1 ring-offset-background`.
        out = out
            .box_shadow(Shadow { spread: 3.0, color: Theme::tint(t.iris, 0.7), ..Shadow::default() })
            .box_shadow(Shadow { spread: 1.0, color: t.background, ..Shadow::default() });
    }
    if let Some(src) = e.source.as_ref().filter(|_| badge) {
        // `absolute -right-1 -bottom-1 rounded-[5px] bg-background p-px shadow-[0_0_0_1px_var(--hairline)]`.
        out = out.child(
            el().key("source").pin(None, None, Some(-4.0), Some(-4.0))
                .padding(1.0)
                .radius(5.0)
                .fill(t.background)
                .box_shadow(Shadow { spread: 1.0, color: t.hairline, ..Shadow::default() })
                .child(app_icon(th, src.path.as_deref(), &src.name, 14.0)),
        );
    }
    out
}
