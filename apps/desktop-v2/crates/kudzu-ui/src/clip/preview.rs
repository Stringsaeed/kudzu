//! The preview of one clip: its heading (kind, formatted, secret and pinned badges, time), the
//! body for its kind, and the details (copy timeline, metadata rows, raw formats). Used by the
//! main window's preview column and the large preview; nothing here names a window state.

use chrono::{DateTime, FixedOffset};
use kudzu_core::clipboard::facts::{clip_time, kind_label};
use rune::{col, icon, row, text, El, Tx};

use super::content::{self, loader::Media, Body};
use super::facts::{details, formats_label};
use super::secret;
use super::timeline::{dot_position, timeline};
use crate::theme::{icons, Theme};

/// Everything one preview shows, borrowed from the window's state for one build.
pub struct Preview<'a> {
    pub body: Body<'a>,
    pub show_secrets: bool,
    pub retention_days: u32,
    pub now: DateTime<FixedOffset>,
    /// The raw-formats list is open.
    pub formats_open: bool,
    /// The picker's short form: no timeline, no formats.
    pub compact: bool,
}

/// How the preview reaches its window.
pub struct Hooks<S: 'static> {
    pub content: content::Hooks<S>,
    /// "Not a secret?" on the heading.
    pub not_a_secret: fn(&mut S, &mut Tx),
    /// The "N formats" summary.
    pub toggle_formats: fn(&mut S, &mut Tx),
}

impl<S: 'static> Clone for Hooks<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S: 'static> Copy for Hooks<S> {}

fn pill(theme: &Theme, label: &str, fill: rune::Color, ink: rune::Color) -> El {
    row()
        .h(18.0)
        .px(6.0)
        .center()
        .radius(6.0)
        .fill(fill)
        .child(text(label.to_owned()).font(theme.ty.meta_11).color(ink))
}

/// Kind, formatted, secret and pinned badges, and when it was copied.
pub fn heading<S: 'static>(theme: &Theme, p: &Preview, hooks: Hooks<S>) -> El {
    let c = &theme.c;
    let e = p.body.entry;
    let mut line = row()
        .key("clip-heading")
        .w_full()
        .h(18.0)
        .gap(8.0)
        .center_y()
        .min_w(0.0)
        .child(pill(
            theme,
            kind_label(e.kind),
            c.foreground.alpha(0.06),
            c.muted_foreground,
        ));
    if e.rich {
        line = line.child(pill(theme, "Formatted", c.foreground.alpha(0.04), c.faint));
    }
    if e.is_secret() {
        line = line.child(secret::badge(theme, hooks.not_a_secret));
    }
    if e.pinned {
        line = line.child(
            row()
                .h(18.0)
                .gap(4.0)
                .center_y()
                .px(6.0)
                .radius(6.0)
                .fill(c.iris_soft)
                .child(icon(&icons::PIN, 12.0).color(c.iris_foreground))
                .child(
                    text("Pinned")
                        .font(theme.ty.meta_11)
                        .color(c.iris_foreground),
                ),
        );
    }
    line.child(row().grow(1.0)).child(
        text(clip_time(e.copied_at, p.now, false))
            .font(theme.ty.control_xs)
            .color(c.faint)
            .truncate(),
    )
}

/// The body for the clip's kind; a secret stays covered until hovered or focused.
pub fn content<S: 'static>(theme: &Theme, p: &Preview, hooks: Hooks<S>) -> El {
    let body = content::body(theme, &p.body, hooks.content);
    if secret::is_hidden(p.body.entry, p.show_secrets) {
        secret::cover(theme, p.body.entry, body)
    } else {
        body
    }
}

fn copy_timeline(theme: &Theme, p: &Preview) -> El {
    let c = &theme.c;
    let e = p.body.entry;
    let line = timeline(e, p.retention_days, p.now.with_timezone(&chrono::Utc));
    let mut track = row()
        .key("clip-timeline")
        .w_full()
        .h(24.0)
        .radius(8.0)
        .fill(c.foreground.alpha(0.03));
    track = theme
        .depth
        .well
        .iter()
        .fold(track, |el, s| el.box_shadow(*s));
    // Positions are fractions of the track, whose width is not known here, so each layer is a
    // row of spacers grown by the gaps between marks.
    let ticks = line
        .ticks
        .iter()
        .map(|t| (*t, row().w(1.0).h(12.0).fill(c.hairline)))
        .collect();
    let last = line.copies.len().saturating_sub(1);
    let dots = line
        .copies
        .iter()
        .enumerate()
        .map(|(i, (at, when))| {
            let ink = if i == last {
                c.iris
            } else {
                c.iris.alpha(0.45)
            };
            let dot = row()
                .key(format!("clip-copy-{i}"))
                .size(8.0, 8.0)
                .radius(4.0)
                .fill(ink)
                .border(2.0, c.background)
                .help(kudzu_core::clipboard::facts::exact_time(*when, p.now));
            (dot_position(*at), dot)
        })
        .collect();
    let marks = row()
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .child(at_fractions(ticks))
        .child(at_fractions(dots).absolute(0.0, 0.0));
    col().w_full().gap(6.0).child(track.child(marks)).child(
        row()
            .w_full()
            .justify_between()
            .child(
                text(line.start_label)
                    .font(theme.ty.meta_10_5)
                    .color(c.faint),
            )
            .child(text("Now").font(theme.ty.meta_10_5).color(c.faint)),
    )
}

fn fact_rows(theme: &Theme, p: &Preview, full: Option<&str>) -> El {
    let c = &theme.c;
    let size = if p.compact {
        theme.ty.meta_11_5
    } else {
        theme.ty.control_xs
    };
    let mut grid = col()
        .key("clip-facts")
        .role("list")
        .label("Details")
        .w_full()
        .gap(6.0);
    for fact in details(p.body.entry, full, p.retention_days, p.now, p.compact) {
        let mut value = row().gap(6.0).center_y().min_w(0.0).grow(1.0);
        if fact.app {
            value = value.child(app_badge(theme, p, 14.0));
        }
        let ink = if fact.label == "Expires" && p.body.entry.pinned {
            c.iris_foreground
        } else {
            c.foreground.alpha(0.85)
        };
        value = value.child(text(fact.value).font(size).color(ink).truncate());
        grid = grid.child(
            row()
                .w_full()
                .gap(20.0)
                .child(
                    text(fact.label.to_owned())
                        .font(size)
                        .color(c.faint)
                        .w(76.0),
                )
                .child(value),
        );
    }
    grid
}

/// A row as wide as its parent with each element centred at its fraction of the width.
fn at_fractions(items: Vec<(f64, El)>) -> El {
    let mut layer = row().w_full().h_full().center_y();
    let mut at = 0.0;
    for (position, el) in items {
        layer = layer
            .child(row().grow(((position - at).max(0.0001)) as f32))
            .child(el);
        at = position;
    }
    layer.child(row().grow(((1.0 - at).max(0.0001)) as f32))
}

/// The source app's icon, or a quiet square until it has loaded.
pub fn app_badge(theme: &Theme, p: &Preview, size: f32) -> El {
    let slot = row().size(size, size).radius(3.0);
    let pixels = p.body.entry.source.as_ref().and_then(|s| {
        p.body
            .media
            .and_then(|m: &Media| m.app_pixels(&s.bundle_id))
    });
    match pixels {
        Some(px) => slot.pixels_fit(&px, rune::SurfaceFit::Fit),
        None => slot.fill(theme.c.foreground.alpha(0.06)),
    }
}

fn formats<S: 'static>(theme: &Theme, p: &Preview, hooks: Hooks<S>) -> El {
    let c = &theme.c;
    let e = p.body.entry;
    let mut summary = row()
        .key("clip-formats-summary")
        .role("button")
        .label(formats_label(e))
        .focusable(true)
        .gap(4.0)
        .center_y()
        .child(
            icon(
                if p.formats_open {
                    &icons::CHEVRON_DOWN
                } else {
                    &icons::CHEVRON_RIGHT
                },
                12.0,
            )
            .color(c.faint),
        )
        .child(
            text(formats_label(e))
                .font(theme.ty.control_xs)
                .color(c.faint),
        );
    if e.rich {
        summary = summary.child(pill(theme, "Formatted", c.foreground.alpha(0.06), c.faint));
    }
    let mut block = col()
        .w_full()
        .gap(8.0)
        .child(summary.on_press_tx(hooks.toggle_formats));
    if p.formats_open {
        let mut list = col().key("clip-formats").pl(16.0).gap(2.0);
        for t in &e.types {
            list = list.child(
                text(t.clone())
                    .font(rune::TextStyle::new(11.0, 400.0).family(theme.ty.mono.family))
                    .color(c.muted_foreground)
                    .truncate()
                    .selectable(true),
            );
        }
        block = block.child(list);
    }
    block
}

/// The copy timeline, the metadata rows and the raw formats.
pub fn details_section<S: 'static>(theme: &Theme, p: &Preview, hooks: Hooks<S>) -> El {
    let e = p.body.entry;
    let mut block = col().key("clip-details").w_full().gap(16.0);
    if !p.compact && e.copy_count > 1 {
        let title = format!("Copied {} times", e.copy_count);
        block = block.child(
            col()
                .role("region")
                .label("Copy activity")
                .w_full()
                .gap(8.0)
                .child(
                    text(title.clone())
                        .role("heading")
                        .label(title)
                        .font(theme.ty.meta_11)
                        .color(theme.c.faint)
                        .w_full(),
                )
                .child(copy_timeline(theme, p)),
        );
    }
    block = block.child(fact_rows(theme, p, p.body.full));
    if !p.compact && !e.types.is_empty() {
        block = block.child(formats(theme, p, hooks));
    }
    block
}

/// Heading, body and details, one under another.
pub fn column<S: 'static>(theme: &Theme, p: &Preview, hooks: Hooks<S>) -> El {
    col()
        .key("clip-preview-content")
        .w_full()
        .gap(24.0)
        .child(heading(theme, p, hooks))
        .child(content(theme, p, hooks))
        .child(details_section(theme, p, hooks))
}
