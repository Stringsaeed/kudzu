//! The footer. Its accessible name is "Copy" because the capture named this bar from
//! the second hint, and the landmark runner reads the accessible name.
use rune::{icon, row, text, El, TextStyle};

use crate::shell::windows::PaletteState;
use crate::theme::icons;
use crate::theme::Theme;
use crate::widgets::key_hints::{accent_cap, key_hint};

pub(super) fn footer(state: &PaletteState, theme: &Theme) -> El {
    let c = &theme.c;
    let held = &state.model.search.held;
    let flash = state.model.search.keys.flash;
    let label = match flash {
        Some(word) => word.to_owned(),
        None => target_label(state),
    };
    let ink = if flash.is_some() {
        c.foreground
    } else {
        c.muted_foreground
    };
    let mark = if flash.is_some() {
        &icons::CHECK
    } else {
        &icons::CORNER_DOWN_LEFT
    };
    row()
        .key("palette-footer")
        // The captured footer is named "Copy": the runner took the second hint's text.
        .role("contentinfo")
        .label("Copy")
        .h(40.0)
        .w_full()
        .px(16.0)
        .gap(16.0)
        .center_y()
        .border_sides([0.5, 0.0, 0.0, 0.0], c.hairline)
        .child(
            row()
                .gap(6.0)
                .center_y()
                // 16 pt in the 18 pt cap: the tree measures the glyph, and a 12 pt
                // glyph centred in the cap starts 3 pt inside the 16 pt padding.
                .child(accent_cap(theme, icon(mark, 16.0), held.is("enter")))
                .child(text(label).font(theme.ty.meta_11_5).color(ink)),
        )
        .child(key_hint(
            theme,
            "mod+enter",
            "mod+enter",
            Some("Copy"),
            held.is("mod+enter"),
        ))
        .child(key_hint(
            theme,
            "mod+e",
            "mod+e",
            Some("Edit"),
            held.is("mod+e"),
        ))
        .child(key_hint(theme, "tab", "tab", Some("Scope"), held.is("tab")))
        .child(row().grow(1.0))
        .child(
            row()
                .center_y()
                .child(
                    text("#".to_owned())
                        .font(TextStyle::new(11.5, 400.0).family(rune::Family::Mono))
                        .color(c.faint),
                )
                .child(
                    text("group".to_owned())
                        .font(theme.ty.meta_11_5)
                        .color(c.faint),
                ),
        )
        .child(key_hint(theme, "escape", "escape", None, held.is("escape")))
}

fn target_label(state: &PaletteState) -> String {
    match state
        .model
        .session
        .as_ref()
        .and_then(|session| session.target())
        .map(|app| app.name.as_str())
    {
        Some(name) if !name.is_empty() => format!("Insert into {name}"),
        _ => "Insert".to_owned(),
    }
}
