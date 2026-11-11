//! The Quick Search panel. The card is inset 12 points so the captured landmarks
//! (dialog, field, scopes, preview, footer) line up. The live vibrancy material is
//! full-bleed; that frame is an S71 check.
mod footer;
mod preview_pane;
mod rows;

use rune::{col, icon, row, text, Cx, El, TextStyle};

use crate::shell::windows::PaletteState;
use crate::theme::icons;
use crate::theme::{radius, Appearance, Theme};
use crate::widgets::key_hints::release_on_key_up;

use super::rank::Scope;

pub fn view(state: &PaletteState, cx: &mut Cx<PaletteState>) -> El {
    // Headless `--theme` is the system appearance. A running app follows Settings.
    let appearance = if rune::is_headless() {
        Appearance::System
    } else {
        state.model.theme.appearance()
    };
    let theme = Theme::resolve(appearance, cx.system_dark());
    let c = &theme.c;
    let card = col()
        .key("palette-card")
        .role("dialog")
        .label("Quick search")
        .w_full()
        .h_full()
        .radius(radius::XL2)
        .fill(c.popover)
        .child(search_row(state, theme))
        .child(
            row()
                .key("palette-body")
                .grow(1.0)
                .min_h(0.0)
                .w_full()
                .child(rows::results(state, theme))
                .child(preview_pane::pane(state, theme, preview_now_ms(state), 0)),
        )
        .child(footer::footer(state, theme));
    let root = col()
        .key("palette")
        .w_full()
        .h_full()
        .padding(12.0)
        .child(card);
    release_on_key_up(root, |s: &mut PaletteState| &mut s.model.search.held)
}

fn preview_now_ms(state: &PaletteState) -> i64 {
    // A frozen scene is the capture: "Used" has to stay on that clock.
    // Live rows use the injected clock and offset 0. The host zone is not a palette input.
    if state.model.search.frozen.is_some() {
        super::scenes::fixture_now().timestamp_millis()
    } else {
        super::model::utc_from(state.svc.clock.as_ref()).timestamp_millis()
    }
}

fn search_row(state: &PaletteState, theme: &Theme) -> El {
    let c = &theme.c;
    row()
        .key("palette-search")
        .h(58.0)
        .w_full()
        .px(16.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 0.5, 0.0], c.hairline)
        .child(icon(&icons::SEARCH, 18.0).color(c.muted_foreground))
        .child(field(state, theme))
        .child(scopes(theme, state.model.search.keys.scope))
}

fn field(state: &PaletteState, theme: &Theme) -> El {
    let c = &theme.c;
    // The icon is centered in the 58pt row. The line box shares that center.
    // Padding is inside the border box, so the combobox rect stays put.
    let line = theme.ty.search.line_px();
    let pad = ((58.0 - line) / 2.0).max(0.0);
    rune::text_input(&state.model.search.edit, theme.ty.search)
        .key("palette-field")
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .py(pad)
        .single_line()
        .color(c.foreground)
        .placeholder("Search snippets…", c.foreground)
        .caret(c.foreground, theme.kit().selection())
        .on_edit(|s: &mut PaletteState| &mut s.model.search.edit)
        .on_input_key(super::on_input_key)
        .autofocus()
        .role("combobox")
        .label("Search snippets")
}

fn scopes(theme: &Theme, scope: Scope) -> El {
    let c = &theme.c;
    let style = TextStyle::new(11.0, 500.0);
    let track = theme.depth.well.iter().fold(
        row()
            .key("palette-scopes")
            .role("radiogroup")
            .label("Scope")
            .shrink()
            .padding(2.0)
            .center_y()
            .radius(radius::LG)
            .fill(c.foreground.alpha(0.04)),
        |track, shadow| track.box_shadow(*shadow),
    );
    track.children(Scope::ALL.into_iter().map(|item| {
        let on = item == scope;
        // The pill is a child. The radio itself stays unfilled: the capture's
        // computed background is transparent, and the pill is a separate layer.
        let mut face = row().h(24.0).px(8.0).center().radius(radius::MD);
        if on {
            face = theme
                .depth
                .soft
                .iter()
                .fold(face.fill(c.surface_raised), |face, shadow| {
                    face.box_shadow(*shadow)
                });
        }
        row()
            .key(item.label())
            .role("radio")
            .label(item.label())
            .checked(on)
            .h(24.0)
            .center()
            .radius(radius::MD)
            .on_press(move |s: &mut PaletteState| s.model.search.keys.set_scope(item))
            .child(face.child(text(item.label()).font(style).color(if on {
                c.foreground
            } else {
                c.muted_foreground
            })))
    }))
}
