//! A colour: a swatch on a checkerboard with its HEX and contrast against white and black,
//! and a card of HEX, RGB, HSL and OKLCH values that copy on press.

use kudzu_core::color;
use rune::{col, row, text, El, TextStyle};

use super::{card, checkerboard, divider, Body, Hooks};
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

fn css(theme_color: &str) -> Option<rune::Color> {
    let [r, g, b, a] = color::parse(theme_color)?;
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

fn swatch(theme: &Theme, css_value: &str, card: &color::ColorCard, large: bool) -> El {
    let height = if large { 256.0 } else { 160.0 };
    let fill = css(css_value).unwrap_or_default();
    let ink = if card.light {
        rune::Color::BLACK.alpha(0.75)
    } else {
        rune::Color::WHITE.alpha(0.9)
    };
    let chip = |bg: rune::Color, fg: rune::Color, value: &str, label: &'static str| {
        row()
            .role("text")
            .label(label)
            .gap(4.0)
            .px(6.0)
            .py(2.0)
            .radius(6.0)
            .fill(bg)
            .child(text("Aa").font(theme.ty.meta_11).color(fill))
            .child(text(value.to_owned()).font(theme.ty.meta_11).color(fg))
    };
    let swatch = col()
        .key("clip-swatch")
        .w_full()
        .h(height)
        .radius(18.0)
        .clip_rounded(18.0)
        .child(checkerboard(theme, 920.0, height))
        .child(row().absolute(0.0, 0.0).w_full().h_full().fill(fill))
        .child(
            row()
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .padding(16.0)
                .items_end()
                .justify_between()
                .child(
                    text(card.values[0].1.clone())
                        .font(TextStyle::new(14.0, 500.0).family(theme.ty.mono.family))
                        .color(ink),
                )
                .child(
                    row()
                        .gap(6.0)
                        .child(chip(
                            rune::Color::WHITE,
                            rune::Color::BLACK.alpha(0.8),
                            &card.on_white,
                            "Contrast with white text",
                        ))
                        .child(chip(
                            rune::Color::BLACK,
                            rune::Color::WHITE.alpha(0.8),
                            &card.on_black,
                            "Contrast with black text",
                        )),
                ),
        );
    theme
        .depth
        .card
        .iter()
        .fold(swatch, |el, shadow| el.box_shadow(*shadow))
}

fn value_row<S: 'static>(
    theme: &Theme,
    label: &str,
    value: &str,
    (copied, height): (bool, f32),
    hooks: Hooks<S>,
) -> El {
    let c = &theme.c;
    let payload = value.to_owned();
    row()
        .w_full()
        .h(height)
        .px(12.0)
        .gap(12.0)
        .center_y()
        .hover_group_focus()
        .child(
            text(label.to_owned())
                .font(theme.ty.meta_11)
                .color(c.faint)
                .w(48.0),
        )
        .child(
            text(value.to_owned())
                .font(TextStyle::new(12.5, 400.0).family(theme.ty.mono.family))
                .color(c.foreground)
                .truncate()
                .selectable(true)
                .grow(1.0)
                .min_w(0.0),
        )
        .child(
            Button::new(format!("Copy {label} value"))
                .variant(Variant::Ghost)
                .size(Size::IconXs)
                .icon(if copied { &icons::CHECK } else { &icons::COPY })
                .help(format!("Copy {label}"))
                .on_press_tx(move |s: &mut S, tx| (hooks.copy_value)(s, payload.clone(), tx))
                .build(theme)
                .group_opacity(0.0, 1.0),
        )
}

pub fn view<S: 'static>(theme: &Theme, b: &Body, hooks: Hooks<S>) -> El {
    let value = b
        .entry
        .color
        .as_deref()
        .or(b.entry.text.as_deref())
        .unwrap_or("");
    let Some(info) = color::card(value) else {
        return col().key("clip-color-body");
    };
    let mut values = card(theme).key("clip-color-values");
    for (i, (label, shown)) in info.values.iter().enumerate() {
        if i > 0 {
            values = values.child(divider(theme));
        }
        values = values.child(value_row(
            theme,
            label,
            shown,
            // `divide-y` puts each divider inside its row's `h-9`: 36 pt per row in all.
            (
                b.copied_value == Some(shown.as_str()),
                if i == 0 { 36.0 } else { 35.0 },
            ),
            hooks,
        ));
    }
    col()
        .key("clip-color-body")
        .w_full()
        .gap(16.0)
        .child(swatch(theme, value, &info, b.large))
        .child(values)
}
