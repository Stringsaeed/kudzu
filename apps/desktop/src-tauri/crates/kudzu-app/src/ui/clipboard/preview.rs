//! `clip-preview.tsx` and `clip-content.tsx`: the heading (kind, formatted, secret, pinned,
//! time), the main preview by kind (text, code with line numbers, links, colours with their
//! values and contrast, images, file lists with Finder icons, other data), and the details
//! (copy timeline, facts, raw formats). Shared by the main window and the picker.

use std::cell::RefCell;
use std::sync::Arc;

use kudzu_core::client::clipboard::{self as clip, kind_label};
use kudzu_core::client::js;
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{col, el, icon, row, text, Color, Cx, El, Family, Shadow, SurfaceFit, TextStyle};
use uuid::Uuid;

use super::list::{js_len, js_slice, pin_glyph};
use super::media::{self, Source};
use super::tile::{checkerboard, inset_hairline, with_fallback};
use super::{color, secret, ClipHost};
use crate::ui::icons;
use crate::ui::kit::app_icon::app_icon;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// Characters rendered before "…and N more" (keeps huge pastes from stalling layout).
const RENDER_LIMIT: usize = 60_000;
const CODE_LINE_LIMIT: usize = 1_500;
const DAY: i64 = 86_400_000;

fn tnum(s: TextStyle) -> TextStyle {
    s.features("'tnum' 1")
}

/// `rounded-md px-1.5 py-0.5 text-[11px]` chips.
fn chip(label: &str, fill: Color, ink: Color, w: f32) -> El {
    row().gap(4.0).center_y().radius(radius::MD).px(6.0).py(2.0).fill(fill).child(text(label.to_string()).font(chip_font(w)).color(ink))
}

/// `text-[11px]` inside a `text-xs` row: the inherited `leading` ratio (4/3) at 11 pt.
pub fn chip_font(w: f32) -> TextStyle {
    TextStyle::new(11.0, w).lh(14.67)
}

/// `ClipHeading`: kind, formatted, secret and pinned chips, and the copy time.
pub fn heading<S: ClipHost>(th: &Theme, e: &ClipEntry) -> El {
    let t = &th.t;
    let mut h = row().key("clip-heading").w_full().min_w(0.0).gap(8.0).center_y();
    h = h.child(chip(kind_label(e.kind), Theme::tint(t.foreground, 0.06), t.muted_foreground, weight::MEDIUM).key("kind"));
    if e.rich {
        h = h.child(chip("Formatted", Theme::tint(t.foreground, 0.04), t.faint, weight::NORMAL).key("formatted"));
    }
    if let Some(b) = secret::badge::<S>(th, e) {
        h = h.child(b.key("secret"));
    }
    if e.pinned {
        h = h.child(
            row().key("pinned")
                .gap(4.0)
                .center_y()
                .radius(radius::MD)
                .px(6.0)
                .py(2.0)
                .fill(t.iris_soft)
                .child(pin_glyph(12.0, t.iris_foreground))
                .child(text("Pinned").font(chip_font(weight::MEDIUM)).color(t.iris_foreground)),
        );
    }
    let time = clip::clip_time(&e.copied_at, super::now_ms(), false, &chrono::Local);
    h.child(el().key("spacer").grow(1.0).min_w(0.0)).child(text(time).key("time").font(tnum(ty::XS)).color(t.faint).truncate().shrink())
}

/// `<Marked>` over a long text: highlights as spans.
fn marked_text(s: &str, terms: &[String], t: &crate::ui::theme::Tokens) -> El {
    super::list::marked(s, terms, t)
}

fn full_or_excerpt(e: &ClipEntry) -> (Option<Arc<String>>, String) {
    let full = super::full_text(e);
    let text = full.as_deref().cloned().unwrap_or_else(|| e.text.clone().unwrap_or_default());
    (full, text)
}

/// `ClipContent`: the preview for its kind; secrets stay covered until hovered.
#[allow(clippy::too_many_arguments)]
pub fn content<S: ClipHost>(s: &S, cx: &mut Cx<S>, layer: &mut Vec<El>, e: &ClipEntry, terms: &[String], large: bool, width: f32, height: f32) -> El {
    let th = theme_for(cx);
    let body = match e.kind {
        ClipKind::Code => code_body(th, e, terms, large),
        ClipKind::Link => link_body::<S>(th, e, large),
        ClipKind::Color => color_body::<S>(th, e, large, cx, layer),
        ClipKind::Image => image_body(th, e, large, width, height),
        ClipKind::File => file_body::<S>(th, e, large, width, height, cx, layer),
        ClipKind::Other => other_body(th, e),
        ClipKind::Text => text_body(th, e, terms, large),
    };
    secret::concealed_box(s, th, e, body)
}

fn text_body(th: &Theme, e: &ClipEntry, terms: &[String], large: bool) -> El {
    let t = &th.t;
    let (full, text_all) = full_or_excerpt(e);
    let shown = js_slice(&text_all, RENDER_LIMIT);
    let size = if large { 15.0 } else { 13.5 };
    // `leading-relaxed` loses to `text-[13.5px]` in tailwind-merge, so the body's 20 px holds.
    let style = TextStyle::new(size, weight::NORMAL).lh(20.0);
    let mut c = col().key("text-body").w_full().child(marked_text(&shown, terms, t).font(style).color(Theme::tint(t.foreground, 0.9)).wrap().wrap_anywhere().selectable(true).w_full());
    if full.is_none() && e.text_len > js_len(&text_all) {
        // The shimmer while the rest loads.
        c = c.child(
            el().key("loading").h(12.0)
                .w(64.0)
                .radius(4.0)
                .mt(4.0)
                .linear_gradient(std::f32::consts::FRAC_PI_2, [(0.0, Color::TRANSPARENT), (0.5, Theme::tint(t.foreground, 0.1)), (1.0, Color::TRANSPARENT)])
                .looping(rune::LoopSpec::opacity(1.6, 0.0, 0.35, 1.0)),
        );
    }
    let hidden = full.as_ref().map_or(0, |f| js_len(f)) as i64 - js_len(&shown) as i64;
    if hidden > 0 {
        c = c.child(text(format!("…and {} more characters. Copy it to get everything.", js::group_thousands(hidden))).key("overflow").font(ty::XS).color(t.faint).mt(12.0));
    }
    c
}

fn code_body(th: &Theme, e: &ClipEntry, terms: &[String], large: bool) -> El {
    let t = &th.t;
    let (_, all) = full_or_excerpt(e);
    let lines: Vec<&str> = all.split('\n').collect();
    let shown = &lines[..lines.len().min(CODE_LINE_LIMIT)];
    let size = if large { 13.5 } else { 12.5 };
    let style = TextStyle::new(size, weight::NORMAL).family(Family::Mono).lh(size * 1.65);
    let numbers = (1..=shown.len()).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
    let code = shown.iter().map(|l| if l.is_empty() { " " } else { l }).collect::<Vec<_>>().join("\n");
    let table = row()
        .items_start()
        .child(text(numbers).font(tnum(style)).color(Theme::tint(t.faint, 0.7)).align(rune::Align::End).pl(12.0).pr(16.0).a11y_hidden())
        .child(marked_text(&code, terms, t).font(style).color(Theme::tint(t.foreground, 0.9)).pr(16.0).selectable(true));
    let mut c = col()
        .key("code-body")
        .w_full()
        .radius(radius::XL)
        .fill(Theme::tint(t.foreground, 0.025))
        .elevate(&t.elev_well)
        .py(12.0)
        .child(row().key("code-scroll").w_full().scroll_x().child(table));
    if lines.len() > shown.len() {
        c = c.child(text(format!("…and {} more lines.", js::group_thousands((lines.len() - shown.len()) as i64))).font(ty::XS).color(t.faint).px(12.0).pt(8.0));
    }
    c
}

fn link_body<S: ClipHost>(th: &Theme, e: &ClipEntry, large: bool) -> El {
    let t = &th.t;
    let url = js::trim(e.text.as_deref().unwrap_or("")).to_string();
    let mail = url.to_ascii_lowercase().starts_with("mailto:");
    let title = clip::describe(e).title;
    let id = e.id;
    let head = row()
        .gap(12.0)
        .center_y()
        .child(el().size(44.0, 44.0).center().radius(radius::XL).fill(Theme::tint(t.foreground, 0.05)).box_shadow(inset_hairline(th)).child(icon(&icons::GLOBE, 20.0).color(t.muted_foreground)))
        .child(
            col()
                .min_w(0.0)
                .shrink()
                .child(text(title).font(TextStyle::new(if large { 20.0 } else { 17.0 }, weight::SEMIBOLD).lh(if large { 28.0 } else { 24.0 }).tracking(-0.4)).color(t.foreground).truncate())
                .child(text(if mail { "Email address" } else { "Web link" }).font(ty::XS).color(t.muted_foreground)),
        );
    let url_box = el()
        .w_full()
        .radius(radius::XL)
        .fill(Theme::tint(t.foreground, 0.025))
        .elevate(&t.elev_well)
        .px(14.0)
        .py(12.0)
        .child(text(url).font(TextStyle::new(12.5, weight::NORMAL).family(Family::Mono).lh(20.3)).color(Theme::tint(t.foreground, 0.85)).wrap().wrap_anywhere().selectable(true));
    let open = Button::new(if mail { "Write email" } else { "Open in browser" })
        .variant(Variant::Secondary)
        .size(Size::Sm)
        .icon(&icons::EXTERNAL_LINK)
        .id(format!("open-link-{id}"))
        .on_press(move |_: &mut S| {
            let _ = crate::backend::clipboard::clipboard_open(id, None);
        })
        .build(th);
    col().key("link-body").w_full().gap(16.0).child(head).child(url_box).child(row().child(open))
}

// MARK: Colour

thread_local! {
    /// The colour value just copied (`done` for 1.2 s).
    static COPIED: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn copy_value<S: ClipHost>(th: &Theme, cx: &mut Cx<S>, layer: &mut Vec<El>, id: Uuid, label: &'static str, value: String, last: bool) -> El {
    let t = &th.t;
    let tag = format!("{id}:{label}");
    let done = COPIED.with(|c| c.borrow().as_deref() == Some(tag.as_str()));
    let v = value.clone();
    let button = Button::icon_only(if done { &icons::CHECK } else { &icons::COPY }, format!("Copy {label} value"))
        .variant(Variant::Ghost)
        .size(Size::IconXs)
        .color(if done { t.success } else { t.foreground })
        .id(format!("copy-value-{label}"))
        .on_press_tx(move |s: &mut S, tx| {
            tx.copy(v.clone());
            let tag = format!("{id}:{label}");
            COPIED.with(|c| *c.borrow_mut() = Some(tag.clone()));
            s.later(
                1200,
                Box::new(move |_, _| {
                    COPIED.with(|c| {
                        let mut c = c.borrow_mut();
                        if c.as_deref() == Some(tag.as_str()) {
                            *c = None;
                        }
                    })
                }),
            );
        })
        .build(th);
    let button = Tooltip::new(format!("copy-{label}")).text(format!("Copy {label}")).build(cx, button, layer);
    let mut r = row()
        .key(format!("value-{label}"))
        .h(36.0)
        .gap(12.0)
        .px(12.0)
        .center_y()
        .hover_group_focus()
        .child(text(label).font(ty::XS2.weight_of(weight::MEDIUM)).color(t.faint).w(48.0))
        .child(text(value).font(TextStyle::new(12.5, weight::NORMAL).family(Family::Mono).lh(18.0)).color(t.foreground).truncate().grow(1.0).min_w(0.0).selectable(true))
        .child(el().child(button).group_opacity(0.0, 1.0));
    if !last {
        r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.hairline);
    }
    r
}

fn color_body<S: ClipHost>(th: &Theme, e: &ClipEntry, large: bool, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
    let t = &th.t;
    let value = e.color.clone().or_else(|| e.text.clone()).unwrap_or_default();
    let rgba = color::parse(&value);
    let info = rgba.map(color::formats);
    let fill = rgba.map_or(Color::TRANSPARENT, color::to_color);
    let h = if large { 256.0 } else { 160.0 };
    let width = cx.layout_size("color-hero").map_or(600.0, |s| s[0]);
    let mut hero = row()
        .key("color-hero")
        .w_full()
        .h(h)
        .padding(16.0)
        .items_end()
        .justify_between()
        .radius(radius::XXL)
        .clip_rounded(radius::XXL)
        .elevate(&t.elev_card)
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(if fill.a < 1.0 { checkerboard(th, width, h) } else { el() }))
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(fill))
        .child(
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .radius(radius::XXL)
                .box_shadow(Shadow { spread: 1.0, color: Color::BLACK.alpha(0.1), inset: true, ..Shadow::default() })
                .box_shadow(Shadow { y: 1.0, color: Color::WHITE.alpha(0.2), inset: true, ..Shadow::default() })
                .inert(),
        );
    if let Some(f) = &info {
        let ink = if f.light { Color::BLACK.alpha(0.75) } else { Color::WHITE.alpha(0.9) };
        let aa = |bg: Color, txt: Color, n: &str, help: &str| {
            row()
                .radius(radius::MD)
                .px(6.0)
                .py(2.0)
                .fill(bg)
                .help(help.to_string())
                .child(text("Aa").font(tnum(ty::XS2.weight_of(weight::SEMIBOLD))).color(fill))
                .child(text(format!(" {n}")).font(tnum(ty::XS2.weight_of(weight::SEMIBOLD))).color(txt))
        };
        hero = hero
            .child(text(f.values[0].1.clone()).font(TextStyle::new(14.0, weight::MEDIUM).family(Family::Mono).lh(20.0)).color(ink))
            .child(
                row()
                    .gap(6.0)
                    .child(aa(Color::WHITE, Color::BLACK.alpha(0.8), &f.on_white, "Contrast with white text"))
                    .child(aa(Color::BLACK, Color::WHITE.alpha(0.8), &f.on_black, "Contrast with black text")),
            );
    }
    let mut body = col().key("color-body").w_full().gap(16.0).child(hero);
    if let Some(f) = info {
        let mut card = col().w_full().radius(radius::XL).fill(t.surface).elevate(&t.elev_card);
        let n = f.values.len();
        for (i, (label, v)) in f.values.into_iter().enumerate() {
            card = card.child(copy_value::<S>(th, cx, layer, e.id, label, v, i + 1 == n));
        }
        body = body.child(card);
    }
    body
}

// MARK: Media

/// An image at its natural size, shrunk to fit `bw`×`bh` (`max-h-full max-w-full object-contain`).
fn fitted(iw: f32, ih: f32, bw: f32, bh: f32) -> (f32, f32) {
    let k = (bw / iw).min(bh / ih).min(1.0);
    ((iw * k).floor().max(1.0), (ih * k).floor().max(1.0))
}

/// The framed picture: `rounded-[6px]`, a hairline ring and a soft drop shadow, the checkerboard
/// only behind it.
fn framed(th: &Theme, full: Source, thumb: Option<Source>, w: f32, h: f32, shadow: bool) -> El {
    let t = &th.t;
    let p = media::picture(full);
    let fallback = match thumb {
        Some(s) => {
            let tp = media::picture(s);
            el().w_full().h_full().pixels_fit(&tp.pixels, SurfaceFit::Fill).live_opacity(&tp.shown)
        }
        None => el(),
    };
    let mut f = el()
        .key("framed-image")
        .size(w, h)
        .radius(6.0)
        .clip_rounded(6.0)
        .box_shadow(Shadow { spread: 1.0, color: t.hairline, ..Shadow::default() })
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).child(checkerboard(th, w, h)).live_opacity(&p.transparent))
        .child(with_fallback(&p, SurfaceFit::Fill, fallback));
    if shadow {
        f = f.box_shadow(Shadow { y: 8.0, blur: 24.0, spread: -12.0, color: Color::BLACK.alpha(0.5), ..Shadow::default() });
    }
    f
}

fn image_body(th: &Theme, e: &ClipEntry, large: bool, width: f32, height: f32) -> El {
    let t = &th.t;
    let (iw, ih) = e.image.map_or((1600.0, 1000.0), |i| (i.width.max(1) as f32, i.height.max(1) as f32));
    let ratio = iw / ih;
    let box_h = if large { (height * 0.64).min(640.0) } else { (width / ratio.max(0.9)).clamp(160.0, 420.0) };
    let (w, h) = fitted(iw, ih, width - 32.0, box_h - 32.0);
    let mut label = "Copied image".to_string();
    if let Some(i) = e.image {
        label.push_str(&format!(", {} by {} pixels", i.width, i.height));
    }
    if let Some(src) = &e.source {
        label.push_str(&format!(", from {}", src.name));
    }
    el().key("image-body")
        .w_full()
        .h(box_h)
        .center()
        .padding(16.0)
        .radius(radius::XL)
        .fill(Theme::tint(t.foreground, 0.025))
        .elevate(&t.elev_well)
        .role("img")
        .label(label)
        .child(framed(th, Source::Image(e.id), Some(Source::Thumb(e.id)), w, h, true))
}

fn file_body<S: ClipHost>(th: &Theme, e: &ClipEntry, large: bool, width: f32, height: f32, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
    let t = &th.t;
    let mut c = col().key("file-body").w_full().gap(12.0);
    if let (1, Some(i)) = (e.files.len(), e.image) {
        let box_h = if large { (height * 0.56).min(560.0) } else { 224.0 };
        let (w, h) = fitted(i.width.max(1) as f32, i.height.max(1) as f32, width - 32.0, box_h - 32.0);
        c = c.child(
            el().w_full()
                .h(box_h)
                .center()
                .padding(16.0)
                .radius(radius::XL)
                .fill(Theme::tint(t.foreground, 0.025))
                .elevate(&t.elev_well)
                .role("img")
                .label(format!("Preview of {}", clip::basename(&e.files[0])))
                .child(framed(th, Source::File(e.files[0].clone()), Some(Source::Thumb(e.id)), w, h, false)),
        );
    }
    let mut list = col().key("file-list").w_full().radius(radius::XL).fill(t.surface).elevate(&t.elev_card).role("list").label("Copied files");
    let shown = e.files.len().min(200);
    for (i, path) in e.files.iter().take(200).enumerate() {
        let name = clip::basename(path);
        let id = e.id;
        let open = Button::icon_only(&icons::EXTERNAL_LINK, format!("Open {name}"))
            .variant(Variant::Ghost)
            .size(Size::IconXs)
            .id(format!("file-open-{i}"))
            .on_press(move |_: &mut S| {
                let _ = crate::backend::clipboard::clipboard_open(id, Some(i));
            })
            .build(th);
        let reveal = Button::icon_only(&icons::FOLDER_OPEN, format!("Show {name} in Finder"))
            .variant(Variant::Ghost)
            .size(Size::IconXs)
            .id(format!("file-reveal-{i}"))
            .on_press(move |_: &mut S| {
                let _ = crate::backend::clipboard::clipboard_reveal(id, Some(i));
            })
            .build(th);
        let open = Tooltip::new(format!("file-open-{i}")).text("Open").build(cx, open, layer);
        let reveal = Tooltip::new(format!("file-reveal-{i}")).text("Show in Finder").build(cx, reveal, layer);
        let pic = media::picture(Source::Icon(path.clone()));
        let mut r = row()
            .key(format!("file-{i}"))
            .w_full()
            .gap(12.0)
            .px(12.0)
            .py(10.0)
            .center_y()
            .hover_group_focus()
            .role("listitem")
            .child(el().size(32.0, 32.0).child(with_fallback(&pic, SurfaceFit::Fit, el().size(32.0, 32.0).radius(radius::LG).fill(Theme::tint(t.foreground, 0.06)))))
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .child(text(name).font(ty::BASE.weight_of(weight::MEDIUM)).color(t.foreground).truncate())
                    .child(text(clip::tildify(&clip::dirname(path))).font(TextStyle::new(11.0, weight::NORMAL).family(Family::Mono).lh(16.5)).color(t.muted_foreground).truncate().selectable(true)),
            )
            .child(row().gap(2.0).child(open).child(reveal).group_opacity(0.0, 1.0));
        if i + 1 < shown {
            r = r.border_sides([0.0, 0.0, 1.0, 0.0], t.hairline);
        }
        list = list.child(r);
    }
    c = c.child(list);
    if e.files.len() > 200 {
        c = c.child(text(format!("…and {} more.", e.files.len() - 200)).font(ty::XS).color(t.faint));
    }
    c
}

fn other_body(th: &Theme, e: &ClipEntry) -> El {
    let t = &th.t;
    let (title, body) = match &e.source {
        Some(s) => (format!("Something copied in {}", s.name), format!("Kudzu can't show this kind of content, but it kept every format. Copy it again and it pastes into {} exactly as before.", s.name)),
        None => ("App-specific data".to_string(), "Kudzu can't show this kind of content, but it kept every format. Copy it again and it pastes back exactly as before.".to_string()),
    };
    col()
        .key("other-body")
        .w_full()
        .items_start()
        .gap(12.0)
        .padding(16.0)
        .radius(radius::XL)
        .fill(Theme::tint(t.foreground, 0.025))
        .elevate(&t.elev_well)
        .child(el().size(40.0, 40.0).center().radius(radius::XL).fill(Theme::tint(t.foreground, 0.06)).child(icon(&icons::PACKAGE, 20.0).color(t.muted_foreground)))
        .child(
            col()
                .child(text(title).font(TextStyle::new(13.5, weight::MEDIUM).lh(20.0)).color(t.foreground))
                .child(text(body).font(TextStyle::new(12.0, weight::NORMAL).lh(19.5)).color(t.muted_foreground).wrap().max_w(384.0).mt(4.0)),
        )
}

// MARK: Details

thread_local! {
    /// The entry whose formats disclosure is open (it closes when another item shows).
    static FORMATS_OPEN: RefCell<Option<Uuid>> = const { RefCell::new(None) };
}

/// `CopyTimeline`: every copy of this content on a track spanning the retention window (or since
/// it was first copied when history is kept forever). Hover a tick for its exact time.
fn timeline<S: ClipHost>(th: &Theme, cx: &mut Cx<S>, layer: &mut Vec<El>, e: &ClipEntry, retention_days: u32) -> El {
    let t = &th.t;
    let now = super::now_ms();
    let times: Vec<i64> = e.timings.iter().map(|d| d.timestamp_millis()).collect();
    let first = times.first().copied().unwrap_or_else(|| e.copied_at.timestamp_millis());
    let span = if retention_days > 0 { (i64::from(retention_days) * DAY).max(now - first) } else { DAY.max(now - first) };
    let start = now - span;
    let days = ((span as f64 / DAY as f64).round() as i64).max(1);
    let width = cx.layout_size("timeline-track").map_or(600.0, |s| s[0]);
    let mut track = el().key("timeline-track").w_full().h(24.0).radius(radius::MD).fill(Theme::tint(t.foreground, 0.03)).elevate(&t.elev_well);
    if days <= 31 {
        for i in 1..days {
            let p = i as f32 / days as f32;
            track = track.child(el().key(format!("day-{i}")).absolute((p * width).round(), 6.0).w(1.0).h(12.0).fill(t.hairline));
        }
    }
    for (i, &at) in times.iter().enumerate() {
        let p = ((at - start) as f64 / span as f64).clamp(0.0, 1.0) as f32;
        let latest = i + 1 == times.len();
        let x = width * (p * 0.96 + 0.02);
        let dot = el()
            .key(format!("tick-{i}"))
            .size(8.0, 8.0)
            .radius(4.0)
            .fill(if latest { t.iris } else { Theme::tint(t.iris, 0.45) })
            .box_shadow(Shadow { spread: 2.0, color: t.background, ..Shadow::default() });
        let when = clip::exact_time(&e.timings[i], now, &chrono::Local);
        let dot = Tooltip::new(format!("tick-{}-{i}", e.id)).text(when).build(cx, dot, layer);
        track = track.child(el().key(format!("copy-{i}")).absolute(x - 4.0, 8.0).child(dot));
    }
    let left = if retention_days > 0 {
        format!("{days} day{} ago", if days == 1 { "" } else { "s" })
    } else {
        js::month_day(&js::local(start, &chrono::Local))
    };
    col()
        .key("timeline")
        .w_full()
        .gap(6.0)
        .label(format!("Copied {} time{}", e.copy_count, if e.copy_count == 1 { "" } else { "s" }))
        .child(track)
        .child(
            row()
                .w_full()
                .justify_between()
                .child(text(left).font(tnum(TextStyle::new(10.5, weight::NORMAL).lh(20.0))).color(t.faint))
                .child(text("Now").font(tnum(TextStyle::new(10.5, weight::NORMAL).lh(20.0))).color(t.faint)),
        )
}

/// `ClipDetails`: when, where from, how big, how long it stays, plus the raw formats.
pub fn details<S: ClipHost>(s: &S, cx: &mut Cx<S>, layer: &mut Vec<El>, e: &ClipEntry, compact: bool) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let retention = s.retention_days();
    let now = super::now_ms();
    let tz = &chrono::Local;
    let font = if compact { TextStyle::new(11.5, weight::NORMAL).lh(16.0) } else { ty::XS };
    let mut facts: Vec<(&str, El)> = Vec::new();
    let value = |v: String| text(v).font(tnum(font)).color(Theme::tint(t.foreground, 0.85)).truncate();
    facts.push(("Copied", value(clip::exact_time(&e.copied_at, now, tz))));
    if e.copy_count > 1 {
        facts.push(("First copied", value(clip::exact_time(&e.first_copied_at, now, tz))));
    }
    if compact && e.copy_count > 1 {
        facts.push(("Times", value(format!("{}×", e.copy_count))));
    }
    if let Some(src) = &e.source {
        facts.push((
            "From",
            row().gap(6.0).center_y().min_w(0.0).child(app_icon(th, src.path.as_deref(), &src.name, 14.0)).child(text(src.name.clone()).font(font).color(Theme::tint(t.foreground, 0.85)).truncate()),
        ));
    }
    if matches!(e.kind, ClipKind::Text | ClipKind::Code) {
        let mut v = format!("{} character{}", js::group_thousands(e.text_len as i64), if e.text_len == 1 { "" } else { "s" });
        if let Some(full) = super::full_text(e) {
            let w = clip::word_count(&full);
            v.push_str(&format!(" · {} word{}", js::group_thousands(w as i64), if w == 1 { "" } else { "s" }));
        }
        if e.line_count > 1 {
            v.push_str(&format!(" · {} lines", js::group_thousands(e.line_count as i64)));
        }
        facts.push(("Length", value(v)));
    }
    if let Some(i) = e.image {
        facts.push(("Dimensions", value(format!("{} × {} px", i.width, i.height))));
    }
    if e.kind == ClipKind::File {
        facts.push(("Items", value(e.files.len().to_string())));
    }
    if matches!(e.kind, ClipKind::Image | ClipKind::Other) {
        facts.push(("Size", value(clip::format_bytes(e.size))));
    }
    if let Some(last) = e.last_used_at.as_ref().filter(|_| e.use_count > 0) {
        facts.push(("Reused", value(format!("{}× · last {}", e.use_count, js::lower(&clip::clip_time(last, now, false, tz))))));
    }
    let expires = clip::expires_in(e, retention, now);
    facts.push(("Expires", text(expires).font(tnum(font)).color(if e.pinned { t.iris_foreground } else { Theme::tint(t.foreground, 0.85) }).truncate()));

    // `grid grid-cols-[auto_minmax(0,1fr)] gap-x-5 gap-y-1.5`: a column of labels, a column of values.
    let mut labels = col().gap(6.0);
    let mut values = col().gap(6.0).grow(1.0).min_w(0.0);
    for (label, v) in facts {
        // The From row's 14 pt icon sits on the baseline grid two points lower (`items-baseline`).
        let tall = label == "From";
        labels = labels.child(el().key(label).h(if tall { 18.0 } else { 16.0 }).child(text(label).font(font).color(t.faint)));
        values = values.child(row().key(label).h(if tall { 18.0 } else { 16.0 }).pt(if tall { 2.0 } else { 0.0 }).center_y().min_w(0.0).child(v));
    }
    let grid = row().key("facts").w_full().gap(20.0).items_start().child(labels).child(values);

    let mut out = col().key("details").w_full().gap(16.0);
    if !compact && e.copy_count > 1 {
        out = out.child(
            col()
                .key("activity")
                .w_full()
                .gap(8.0)
                .label("Copy activity")
                .child(text(format!("Copied {} times", e.copy_count)).font(TextStyle::new(11.0, weight::MEDIUM).lh(20.0)).color(t.faint))
                .child(timeline::<S>(th, cx, layer, e, retention)),
        );
    }
    out = out.child(grid);
    if !compact && !e.types.is_empty() {
        let open = FORMATS_OPEN.with(|f| *f.borrow() == Some(e.id));
        let id = e.id;
        let n = e.types.len();
        let mut summary = row()
            .key("formats-summary")
            .gap(4.0)
            .center_y()
            .radius(4.0)
            .role("button")
            .expanded(open)
            .hover_group()
            .focusable(true)
            .focus_ring(2.0, 0.0, t.ring)
            .on_press(move |_: &mut S| {
                FORMATS_OPEN.with(|f| {
                    let mut f = f.borrow_mut();
                    *f = if *f == Some(id) { None } else { Some(id) };
                })
            })
            .child(icon(&icons::CHEVRON_RIGHT, 12.0).color(t.faint).group_color(t.muted_foreground).rotate(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 }).rotate_motion(crate::ui::theme::motion::moderate()))
            .child(text(format!("{n} format{}", if n == 1 { "" } else { "s" })).font(ty::XS).color(t.faint).group_color(t.muted_foreground));
        if e.rich {
            summary = summary.child(text("Formatted").font(TextStyle::new(10.0, weight::NORMAL).lh(13.33)).color(t.faint).ml(4.0).px(4.0).radius(4.0).fill(Theme::tint(t.foreground, 0.06)));
        }
        let mut disclosure = col().key("formats").gap(8.0).child(row().child(summary));
        if open {
            let mut list = col().key("formats-list").gap(2.0).pl(16.0);
            for ty_name in &e.types {
                list = list.child(text(ty_name.clone()).font(TextStyle::new(11.0, weight::NORMAL).family(Family::Mono).lh(14.67)).color(t.muted_foreground).truncate().selectable(true));
            }
            disclosure = disclosure.child(list);
        }
        out = out.child(disclosure);
    }
    out
}
