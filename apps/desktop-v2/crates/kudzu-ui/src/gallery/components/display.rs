//! `badge`, `separator`, `kbd`, `item`: rune-kit's components (and a plain row) as they are.

use rune::{col, row, text, El};
use rune_kit::{Badge, BadgeVariant, Divider, Keybind, KeybindVariant};

use super::line;
use crate::theme::{radius, Theme};

pub fn badge(theme: &Theme) -> El {
    let k = theme.kit();
    col().child(line(
        theme,
        "Neutral (default), subtle (ghost), accent",
        [
            Badge::new("Dev").build(k).key("info:badge/neutral"),
            Badge::new("Resolved")
                .variant(BadgeVariant::Subtle)
                .build(k)
                .key("info:badge/subtle"),
            Badge::new("New")
                .variant(BadgeVariant::Accent)
                .build(k)
                .key("info:badge/accent"),
        ],
    ))
}

pub fn separator(theme: &Theme) -> El {
    let k = theme.kit();
    col().gap(16.0).child(line(
        theme,
        "Horizontal, vertical",
        [
            col().w(240.0).child(
                Divider::horizontal()
                    .build(k)
                    .key("spec:separator/horizontal:240x1"),
            ),
            row().h(24.0).child(
                Divider::vertical()
                    .build(k)
                    .key("spec:separator/vertical:1x24"),
            ),
        ],
    ))
}

pub fn kbd(theme: &Theme) -> El {
    let k = theme.kit();
    col().child(line(
        theme,
        "Keycaps: neutral, ghost",
        [
            Keybind::new(&["⌘", "K"]).build(k).key("info:kbd/neutral"),
            Keybind::new(&["⌃", "⌥", "Space"])
                .variant(KeybindVariant::Ghost)
                .build(k)
                .key("info:kbd/ghost"),
        ],
    ))
}

/// shadcn's `Item` (`rounded-lg border px-2.5 py-2 gap-2`, a title over a description): no
/// rune-kit component is needed, it is a plain `row()`.
pub fn item(theme: &Theme) -> El {
    let c = &theme.c;
    let item = row()
        .w(360.0)
        .gap(8.0)
        .px(11.0)
        .py(9.0)
        .radius(radius::LG)
        .border(1.0, c.border)
        .child(
            col()
                .gap(4.0)
                .child(text("Espanso").font(theme.ty.control).color(c.foreground))
                .child(
                    text("Found 12 snippets in ~/Library/Application Support/espanso")
                        .font(theme.ty.input)
                        .color(c.muted_foreground)
                        .truncate(),
                ),
        );
    col().child(line(
        theme,
        "Item: title and description",
        [item.key("info:item")],
    ))
}
