//! The selection bar that replaces the footer while several clips are selected, and the
//! clear-history dialog.

use rune::{col, row, text, El};

use super::actions;
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::checkbox::Checkbox;
use crate::widgets::kit_ext::dialog::Dialog;

/// "3 selected" with Pin and Delete, over the whole multi-selection.
pub fn bar(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let list = &s.clipboard.clips.list;
    let ids = list.selection();
    let all_pinned = list.entries(&ids).all(|e| e.pinned);
    let (pin_label, pin_glyph) = if all_pinned {
        ("Unpin", &icons::PIN_OFF)
    } else {
        ("Pin", &icons::PIN)
    };
    let bar = row()
        .key("clip-bulk")
        .role("toolbar")
        .label("Selection actions")
        .mx(8.0)
        .mt(8.0)
        .mb(8.0)
        .py(6.0)
        .pr(6.0)
        .pl(12.0)
        .gap(4.0)
        .center_y()
        .radius(14.0)
        .fill(c.surface_raised)
        .child(
            text(format!("{} selected", ids.len()))
                .font(theme.ty.control_xs)
                .color(c.foreground)
                .grow(1.0),
        )
        .child(
            Button::new(pin_label)
                .variant(Variant::Ghost)
                .size(Size::IconSm)
                .icon(pin_glyph)
                .help(pin_label)
                .on_press(|st: &mut MainState| actions::toggle_pin(st))
                .build(theme),
        )
        .child(
            Button::new("Delete")
                .variant(Variant::Ghost)
                .size(Size::IconSm)
                .icon(&icons::TRASH_2)
                .help("Delete")
                .on_press_tx(actions::delete_selection)
                .build(theme),
        );
    theme
        .depth
        .float
        .iter()
        .fold(bar, |el, shadow| el.box_shadow(*shadow))
}

/// "Clear clipboard history?" with the pinned choice.
pub fn clear_dialog(s: &MainState, theme: &Theme, layer: &mut Vec<El>) {
    let Some(dialog) = s.clipboard.clear else {
        return;
    };
    let pinned = s
        .clipboard
        .clips
        .list
        .history()
        .entries
        .iter()
        .filter(|e| e.pinned)
        .count();
    let choice = Checkbox::new(dialog.include_pinned)
        .label(format!("Also delete pinned items ({pinned})"))
        .on_toggle(|st: &mut MainState| {
            if let Some(d) = &mut st.clipboard.clear {
                d.include_pinned = !d.include_pinned;
            }
        })
        .build(theme);
    let buttons = row()
        .w_full()
        .gap(8.0)
        .justify_end()
        .child(
            Button::new("Cancel")
                .variant(Variant::Outline)
                .on_press(|st: &mut MainState| st.clipboard.clear = None)
                .build(theme),
        )
        .child(
            Button::new("Clear history")
                .variant(Variant::Destructive)
                .on_press(|st: &mut MainState| actions::confirm_clear(st))
                .build(theme),
        );
    Dialog::new("clip-clear", "Clear clipboard history?")
        .description("Everything Kudzu remembered is deleted from this Mac. This can't be undone.")
        .open(true)
        .kudzu()
        .child(col().w_full().gap(16.0).child(choice).child(buttons))
        .on_close(|st: &mut MainState| st.clipboard.clear = None)
        .build(theme, layer);
}
