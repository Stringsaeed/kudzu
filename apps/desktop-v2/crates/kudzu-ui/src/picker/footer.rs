//! The footer: the Return hint (or the message that replaced it), then the hints of the keys
//! that work on the active clip, then Tab and Escape on the right. Its accessible name is
//! "Copy", as the capture's is (the runner read the second hint's text).

use rune::{icon, row, text, El};

use super::model::Picker;
use crate::theme::{icons, Theme};
use crate::widgets::key_hints::{accent_cap, key_hint};

pub(super) fn footer(p: &Picker, theme: &Theme) -> El {
    let c = &theme.c;
    let flash = p.flash_text();
    let label = flash.map_or_else(|| p.paste_label(), str::to_owned);
    // The check replaces the Return glyph for a copy only ("Copied").
    let copied = flash == Some("Copied");
    let mark = if copied {
        &icons::CHECK
    } else {
        &icons::CORNER_DOWN_LEFT
    };
    let ink = if flash.is_some() {
        c.foreground
    } else {
        c.muted_foreground
    };
    let mut hints = row().gap(16.0).center_y().grow(1.0).min_w(0.0);
    if flash.is_none() {
        if p.can_paste {
            hints = hints.child(key_hint(
                theme,
                "mod+enter",
                "mod+enter",
                Some("Copy"),
                p.held.is("mod+enter"),
            ));
        }
        if p.active().is_some_and(|e| e.rich) {
            hints = hints.child(key_hint(
                theme,
                "shift+enter",
                "shift+enter",
                Some(if p.prefs.paste_plain {
                    "Formatted"
                } else {
                    "Plain text"
                }),
                p.held.is("shift+enter"),
            ));
        }
        hints = hints
            .child(key_hint(
                theme,
                "mod+p",
                "mod+p",
                Some(if p.active().is_some_and(|e| e.pinned) {
                    "Unpin"
                } else {
                    "Pin"
                }),
                p.held.is("mod+p"),
            ))
            .child(key_hint(
                theme,
                "mod+backspace",
                "mod+backspace",
                Some("Delete"),
                p.held.is("mod+backspace"),
            ));
    }
    row()
        .key("picker-footer")
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
                .key("picker-footer-label")
                .gap(6.0)
                .center_y()
                .min_w(0.0)
                .child(accent_cap(theme, icon(mark, 16.0), p.held.is("enter")))
                .child(text(label).font(theme.ty.meta_11_5).color(ink).truncate()),
        )
        .child(hints)
        .child(key_hint(
            theme,
            "tab",
            "tab",
            Some("Filter"),
            p.held.is("tab"),
        ))
        .child(key_hint(
            theme,
            "escape",
            "escape",
            None,
            p.held.is("escape"),
        ))
}
