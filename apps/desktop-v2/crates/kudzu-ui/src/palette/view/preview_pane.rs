//! The preview beside the list. "Will insert" is painted in the foreground: the
//! capture's computed ink is the foreground, while the stylesheet says faint.
use rune::{col, icon, row, text, El, ScrollSpec, TextStyle};

use crate::shell::windows::PaletteState;
use crate::theme::icons;
use crate::theme::Theme;
use crate::widgets::formatted_text::formatted_text;
use crate::widgets::trigger_chip::trigger_chip;

use super::super::clamped;
use super::super::rank::{used_line, Row};

pub(super) fn pane(state: &PaletteState, theme: &Theme, now_ms: i64, offset_minutes: i32) -> El {
    let c = &theme.c;
    let presentation = &state.model.search.presentation;
    let index = clamped(state);
    let current = presentation.rows.get(index);
    let body = match current {
        Some(row) => snippet_body(state, theme, row, now_ms, offset_minutes),
        None if presentation.create => col().grow(1.0).center().child(
            text("Opens Kudzu with a new snippet".to_owned())
                .font(TextStyle::new(12.0, 400.0))
                .color(c.foreground),
        ),
        None => col().grow(1.0),
    };
    col()
        .key("palette-preview")
        .role("complementary")
        .label("Preview")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .fill(c.foreground.alpha(0.015))
        .border_sides([0.0, 0.0, 0.0, 0.5], c.hairline)
        .child(body)
}

fn snippet_body(
    state: &PaletteState,
    theme: &Theme,
    item: &Row,
    now_ms: i64,
    offset_minutes: i32,
) -> El {
    let c = &theme.c;
    let mut copy = col()
        .key("preview-copy")
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .px(20.0)
        .pt(16.0)
        .pb(12.0)
        .gap(8.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            text("Will insert".to_owned())
                .font(TextStyle::new(11.0, 500.0))
                .color(c.foreground),
        )
        .child(render(state, theme, item));
    if let Some(badge) = item.form_badge.as_deref() {
        copy = copy.child(badge_row(theme, badge));
    }
    col()
        .w_full()
        .h_full()
        .child(copy)
        .child(meta(theme, item, now_ms, offset_minutes))
}

fn render(state: &PaletteState, theme: &Theme, _item: &Row) -> El {
    let c = &theme.c;
    let search = &state.model.search;
    // Keep the last render, including while a new row is in flight.
    let shown = search
        .rendered
        .as_ref()
        .filter(|(id, _)| search.preview.shown() == Some(*id));
    let mut body = match shown {
        Some((_, result)) if result.text.is_empty() && result.actions.is_empty() => {
            text("Empty snippet".to_owned())
                .font(theme.ty.preview)
                .color(c.faint)
        }
        Some((_, result)) => formatted_text(theme, result, theme.ty.preview, c.foreground, None),
        None => row()
            .key("preview-shimmer")
            .h(16.0)
            .w_frac(0.66)
            .radius(4.0)
            .fill(c.foreground.alpha(0.08)),
    };
    if search.dim {
        body = body.opacity(0.5);
    }
    body
}

fn badge_row(theme: &Theme, badge: &str) -> El {
    let c = &theme.c;
    row()
        .key("preview-badge")
        .gap(6.0)
        .center_y()
        .px(8.0)
        .h(24.0)
        .radius(crate::theme::radius::MD)
        .fill(c.warning.alpha(0.12))
        .child(icon(&icons::TEXT_CURSOR_INPUT, 12.0).color(c.warning))
        .child(
            text(badge.to_owned())
                .font(TextStyle::new(11.0, 500.0))
                .color(c.warning),
        )
}

fn meta(theme: &Theme, item: &Row, now_ms: i64, offset_minutes: i32) -> El {
    let c = &theme.c;
    let style = theme.ty.meta_11_5;
    let mut block = col()
        .key("preview-meta")
        .shrink()
        .w_full()
        .px(20.0)
        .pt(12.0)
        .pb(12.0)
        .gap(6.0)
        .border_sides([0.5, 0.0, 0.0, 0.0], c.hairline)
        .child(meta_line(theme, "Triggers", triggers(theme, item)));
    if !item.path.is_empty() {
        block = block.child(meta_line(
            theme,
            "Group",
            row()
                .gap(4.0)
                .center_y()
                .child(icon(&icons::FOLDER, 12.0).color(c.muted_foreground))
                .child(
                    text(item.path.clone())
                        .font(style)
                        .color(c.muted_foreground),
                ),
        ));
    }
    block.child(meta_line(
        theme,
        "Used",
        text(used_line(item.count, item.last_ms, now_ms, offset_minutes))
            .font(style)
            .color(c.muted_foreground),
    ))
}

fn triggers(theme: &Theme, item: &Row) -> El {
    if item.triggers.is_empty() {
        return text("None".to_owned())
            .font(theme.ty.meta_11_5)
            .color(theme.c.muted_foreground);
    }
    let mut line = row().gap(4.0).center_y();
    for trigger in &item.triggers {
        line = line.child(trigger_chip(theme, trigger, true));
    }
    line
}

fn meta_line(theme: &Theme, label: &str, value: El) -> El {
    row()
        .gap(16.0)
        .center_y()
        .child(
            text(label.to_owned())
                .font(theme.ty.meta_11_5)
                .color(theme.c.faint)
                .w(64.0),
        )
        .child(value)
}
