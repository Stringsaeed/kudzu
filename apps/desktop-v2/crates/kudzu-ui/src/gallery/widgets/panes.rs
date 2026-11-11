//! `secret-text`, `empty-state`, `settings-card`.

use rune::{col, row, text, El};

use crate::gallery::components::line;
use crate::gallery::GalleryState;
use crate::theme::{icons, Theme};
use crate::widgets::empty_state::{glyph_tile, EmptyState};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::switch::Switch;
use crate::widgets::secret_text::{cover, row_secret};
use crate::widgets::settings_card::{setting_row, SettingsCard};
use crate::widgets::shortcut::Shortcut;

/// A concealed and a revealed row; the preview's cover.
pub fn secret_text(theme: &Theme) -> El {
    let (style, ink) = (theme.ty.body, theme.c.foreground);
    let cover = cover(
        theme,
        "Looks like a password",
        false,
        || text("never built while covered"),
        |_: &mut GalleryState, _| {},
    );
    col()
        .gap(24.0)
        .child(line(
            theme,
            "Row: concealed, revealed",
            [
                row_secret("hunter2-correct-horse", true, false, style, ink)
                    .key("info:secret-text/concealed"),
                row_secret("hunter2-correct-horse", true, true, style, ink)
                    .key("info:secret-text/revealed"),
            ],
        ))
        .child(line(
            theme,
            "Preview cover (min-h-28)",
            [col()
                .w(296.0)
                .child(cover)
                .key("spec:secret-text/cover:296x112")],
        ))
}

/// The editor's empty pane.
pub fn empty_state(theme: &Theme) -> El {
    let action = Button::new("New snippet")
        .size(Size::Sm)
        .icon(&icons::PLUS)
        .build(theme);
    let empty = EmptyState::new(&icons::CORNER_DOWN_LEFT, "No snippet selected")
        .description("Pick one from the list, or start a new one.")
        .action(action)
        .build(theme);
    col().gap(24.0).child(line(
        theme,
        "Glyph tile; the pane",
        [
            glyph_tile(theme, &icons::CORNER_DOWN_LEFT).key("spec:empty-state/glyph:48x48"),
            col()
                .size(420.0, 260.0)
                .child(empty)
                .key("info:empty-state/pane"),
        ],
    ))
}

/// The Quick search group as Settings ▸ General draws it, and a group of switch rows.
pub fn settings_card(theme: &Theme) -> El {
    let c = &theme.c;
    let caps = |combo: &str| Shortcut::combo(combo).build(theme);
    let words = |t: &str| {
        text(t.to_owned())
            .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
            .color(c.muted_foreground)
    };
    let description = row()
        .gap(4.0)
        .center_y()
        .child(words("Opens the search tray over any app."))
        .child(caps("enter"))
        .child(words("inserts,"))
        .child(caps("mod+enter"))
        .child(words("copies."));
    let recorder = Button::new("⌃ ⌥ Space")
        .name("Quick search shortcut: ⌃ ⌥ Space. Press to change.")
        .variant(Variant::Outline)
        .build(theme)
        .w(144.0);
    let quick = SettingsCard::new("Quick search")
        .row(shortcut_row(theme, description, recorder))
        .build(theme)
        .w(616.0)
        .key("parity:main:settings-general:region:1:Quick search");
    let switches = SettingsCard::new("Privacy")
        .description("Secrets stay hidden until you hover over them.")
        .row(setting_row(
            theme,
            "Show secrets",
            Some("Show passwords and keys in clipboard history."),
            vec![Switch::new(false).label("Show secrets").build(theme)],
            false,
        ))
        .row(setting_row(
            theme,
            "Paste as plain text",
            None,
            vec![Switch::new(true).label("Paste as plain text").build(theme)],
            true,
        ))
        .build(theme)
        .w(616.0)
        .key("info:settings-card/switches");
    col()
        .gap(24.0)
        .child(line(theme, "Quick search (reference)", [quick]))
        .child(line(theme, "Switch rows, one dimmed", [switches]))
}

/// A settings row whose description holds keycaps (a row of words and caps, not one text).
fn shortcut_row(theme: &Theme, description: El, control: El) -> El {
    row()
        .w_full()
        .min_h(56.0)
        .py(12.0)
        .gap(24.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .gap(2.0)
                .child(
                    text("Shortcut")
                        .font(theme.ty.body_medium)
                        .color(theme.c.foreground),
                )
                .child(description),
        )
        .child(control)
}
