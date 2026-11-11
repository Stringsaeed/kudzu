//! The fill-in form card: header, fields, preview, footer.
use rune::{col, el, icon, row, text, Cx, El, ScrollSpec};

use super::fields;
use super::place::{FORM_H_MIN, FORM_W};
use crate::shell::windows::FormState;
use crate::theme::{icons, radius, Theme};
use crate::widgets::formatted_text::formatted_text;
use crate::widgets::key_hints::{self, key_hint};
use crate::widgets::shortcut::{Shortcut, Tone};

pub fn view(state: &FormState, cx: &mut Cx<FormState>) -> El {
    let theme = state.model.theme.theme(cx.system_dark());
    let Some(request) = state.model.request.as_ref() else {
        return col().key("form").w_full().h_full();
    };
    let mut layer = Vec::new();
    let card = card(state, request, theme, &mut layer);
    let mut root = key_hints::release_on_key_up(
        col()
            .key("form")
            .role("dialog")
            .label(format!("Fill in {}", request.snippet_name))
            .w_full()
            .fill(theme.c.popover)
            .child(card),
        |s: &mut FormState| &mut s.model.held,
    );
    for overlay in layer {
        root = root.child(overlay);
    }
    root
}

fn card(
    state: &FormState,
    request: &kudzu_app::forms::FormRequest,
    theme: &Theme,
    layer: &mut Vec<El>,
) -> El {
    let mut fields_col = col().key("fields").gap(12.0).px(16.0).py(14.0).w_full();
    if let Some(panel) = state.model.panel.as_ref() {
        for field in &panel.unique {
            fields_col = fields_col.child(fields::field(state, theme, field, layer));
        }
    }
    theme.depth.panel.iter().fold(
        col()
            .key("card")
            .w(FORM_W as f32)
            .min_h(FORM_H_MIN as f32)
            .fill(theme.c.popover)
            .radius(radius::FORM_WINDOW)
            .clip_rounded(radius::FORM_WINDOW)
            .on_layout_tx(|s: &mut FormState, r, tx| s.fit_height(r[3] as f64, tx))
            .child(header(request, theme))
            .child(hairline("header", theme))
            .child(fields_col)
            .child(preview(state, theme))
            .child(hairline("footer", theme))
            .child(footer(state, theme)),
        |el, shadow| el.box_shadow(*shadow),
    )
}

fn hairline(key: &str, theme: &Theme) -> El {
    el().key(format!("hairline-{key}"))
        .h(1.0)
        .w_full()
        .fill(theme.c.hairline)
        .a11y_hidden()
}

fn header(request: &kudzu_app::forms::FormRequest, theme: &Theme) -> El {
    let c = &theme.c;
    // Left-align the 14 pt glyph in the 24 pt badge so the banner's content inset is the
    // header's 16 pt padding (metrics.json), not the extra 5 pt of a centred icon.
    let glyph = row()
        .size(24.0, 24.0)
        .center_y()
        .radius(radius::MD)
        .fill(c.iris_soft)
        .child(icon(&icons::TEXT_CURSOR_INPUT, 14.0).color(c.iris_foreground));
    let title = text(if request.snippet_name.is_empty() {
        "Fill in snippet".into()
    } else {
        request.snippet_name.clone()
    })
    .font(theme.ty.body_strong)
    .color(c.foreground)
    .truncate()
    .grow(1.0);
    let mut bar = row()
        .key("header")
        .role("banner")
        .label(request.snippet_name.clone())
        .w_full()
        .gap(10.0)
        .px(16.0)
        .py(12.0)
        .center_y()
        .window_drag()
        .child(glyph)
        .child(title);
    if let Some(target) = request.target.as_ref() {
        bar = bar.child(
            text(format!("into {}", target.name))
                .font(theme.ty.meta_11)
                .color(c.faint)
                .truncate(),
        );
    }
    bar
}

fn preview(state: &FormState, theme: &Theme) -> El {
    let c = &theme.c;
    let body = match state
        .model
        .panel
        .as_ref()
        .and_then(|panel| panel.preview_result.as_ref())
    {
        Some(result) => formatted_text(
            theme,
            result,
            theme.ty.description,
            c.foreground.alpha(0.85),
            None,
        ),
        None => el()
            .key("shimmer")
            .h(14.0)
            .w(180.0)
            .radius(radius::LG)
            .fill(c.foreground.alpha(0.08)),
    };
    let well = crate::widgets::kit_ext::input::rest_fill(theme);
    theme.depth.well.iter().fold(
        col()
            .key("preview")
            .mx(16.0)
            .mb(14.0)
            .max_h(160.0)
            .px(12.0)
            .py(10.0)
            .gap(4.0)
            .radius(radius::LG)
            .fill(well)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(text("Preview").font(theme.ty.meta_10_5).color(c.faint))
            .child(body),
        |el, shadow| el.box_shadow(*shadow),
    )
}

fn footer(state: &FormState, theme: &Theme) -> El {
    let held = &state.model.held;
    row()
        .key("footer")
        .role("contentinfo")
        .label("Next field")
        .w_full()
        .gap(16.0)
        .px(16.0)
        .py(10.0)
        .center_y()
        .child(key_hint(
            theme,
            "tab",
            "tab",
            Some("Next field"),
            held.is("tab"),
        ))
        .child(key_hint(
            theme,
            "escape",
            "escape",
            Some("Cancel"),
            held.is("escape"),
        ))
        .child(el().grow(1.0))
        .child(insert_button(theme))
}

/// Tauri's Insert is `h-7 px-3 gap-2 bg-iris text-xs` with `shadow-accent sheen-strong`.
/// S07's default button paints `primary`, which is a darker blue in dark mode.
fn insert_button(theme: &Theme) -> El {
    let c = &theme.c;
    let [(_, top), (_, bottom)] = crate::theme::palette::fixed::SHEEN_STRONG;
    let face = row()
        .gap(8.0)
        .center_y()
        .child(
            text("Insert".to_owned())
                .font(theme.ty.control_xs)
                .color(c.iris_contrast),
        )
        .child(
            Shortcut::combo("enter")
                .tone(Tone::OnAccent)
                .ink(c.iris_contrast)
                .build(theme),
        );
    theme.depth.accent.iter().fold(
        row()
            .key("insert")
            .role("button")
            .label("Insert")
            .h(28.0)
            .px(12.0)
            .center_y()
            .radius(radius::LG)
            .fill(c.iris)
            .gradient(top.color(), bottom.color())
            .on_press(|s: &mut FormState| s.submit())
            .default_button()
            .child(face),
        |el, shadow| el.box_shadow(*shadow),
    )
}
