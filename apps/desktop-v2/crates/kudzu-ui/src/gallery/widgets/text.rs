//! `shortcut`, `trigger-chip`, `key-hints`, `formatted-text`, `wordmark`.

use rune::{col, el, icon, row, text, El};

use kudzu_core::formatting::{FormatSpan, FormatStyle};
use kudzu_core::template::{ActionKind, Key, RenderAction, RenderResult};

use crate::gallery::components::line;
use crate::theme::{icons, radius, Theme};
use crate::widgets::formatted_text::formatted_text as rich_preview;
use crate::widgets::key_hints::{accent_cap, key_hint};
use crate::widgets::shortcut::{Shortcut, Size, Tone};
use crate::widgets::trigger_chip::trigger_chip as chip;
use crate::widgets::wordmark::{wordmark as mark, Variant};

/// Sizes (`sm` 18, `md` 20) and tones; a spoken shortcut in a sentence.
pub fn shortcut(theme: &Theme) -> El {
    let s = |combo: &str, tone, size: Size, key: &str| {
        Shortcut::combo(combo)
            .tone(tone)
            .size(size)
            .build(theme)
            .key(key.to_owned())
    };
    let on_accent = row()
        .padding(8.0)
        .radius(radius::LG)
        .fill(theme.c.iris)
        .child(
            Shortcut::combo("mod+enter")
                .tone(Tone::OnAccent)
                .ink(theme.c.iris_contrast)
                .build(theme)
                .key("spec:shortcut/on-accent:_x18"),
        );
    let spoken = row()
        .gap(4.0)
        .center_y()
        .child(
            text("Press")
                .font(theme.ty.body)
                .color(theme.c.muted_foreground),
        )
        .child(
            Shortcut::combo("mod+enter")
                .spoken(true)
                .build(theme)
                .key("spec:shortcut/spoken:_x18"),
        )
        .child(
            text("to save.")
                .font(theme.ty.body)
                .color(theme.c.muted_foreground),
        );
    col()
        .gap(24.0)
        .child(line(
            theme,
            "Sm (18), md (20)",
            [
                s(
                    "mod+shift+k",
                    Tone::Default,
                    Size::Sm,
                    "spec:shortcut/sm:_x18",
                ),
                s(
                    "mod+shift+k",
                    Tone::Default,
                    Size::Md,
                    "spec:shortcut/md:_x20",
                ),
                s("escape", Tone::Default, Size::Sm, "spec:shortcut/word:_x18"),
            ],
        ))
        .child(line(
            theme,
            "Ghost, accent, on an accent fill",
            [
                s("mod+k", Tone::Ghost, Size::Sm, "spec:shortcut/ghost:_x18"),
                s(
                    "ctrl+alt+space",
                    Tone::Accent,
                    Size::Sm,
                    "spec:shortcut/accent:_x18",
                ),
                on_accent,
            ],
        ))
        .child(line(theme, "Spoken inside a sentence", [spoken]))
}

/// Active, muted, and a trailing space shown as `␣`.
pub fn trigger_chip(theme: &Theme) -> El {
    col().child(line(
        theme,
        "Active, muted, trailing space, long",
        [
            chip(theme, ";sig", false).key("spec:trigger-chip/active:_x20"),
            chip(theme, ";sig", true).key("spec:trigger-chip/muted:_x20"),
            chip(theme, ";addr ", false).key("spec:trigger-chip/space:_x20"),
            row().w(96.0).child(
                chip(theme, ";a-very-long-trigger", false).key("spec:trigger-chip/truncated:_x20"),
            ),
        ],
    ))
}

/// The palette's footer: the accent Enter cap, keycap hints, one held.
pub fn key_hints(theme: &Theme) -> El {
    let c = &theme.c;
    let enter = row()
        .gap(6.0)
        .center_y()
        .child(
            accent_cap(theme, icon(&icons::CORNER_DOWN_LEFT, 12.0), false)
                .key("spec:key-hint/enter-cap:18x18"),
        )
        .child(
            text("Insert into Mail")
                .font(theme.ty.meta_11_5)
                .color(c.muted_foreground),
        );
    let footer = row()
        .w(640.0)
        .h(40.0)
        .gap(16.0)
        .px(16.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], c.hairline)
        .child(enter)
        .child(
            key_hint(theme, "mod+enter", "mod+enter", Some("Copy"), false)
                .key("spec:key-hint/copy:_x18"),
        )
        .child(key_hint(theme, "mod+e", "mod+e", Some("Edit"), false))
        .child(key_hint(theme, "tab", "tab", Some("Scope"), true).key("spec:key-hint/held:_x18"))
        .child(el().grow(1.0))
        .child(key_hint(theme, "escape", "escape", None, false));
    col().child(line(
        theme,
        "Footer: Tab held",
        [footer.key("spec:key-hints/footer:640x40")],
    ))
}

fn sample() -> RenderResult {
    RenderResult {
        text: "Hi Maya, thanks for the quick turnaround. Signed copy: https://kudzu.site/c\nBest, Saeed"
            .into(),
        formatting: Some(vec![
            FormatSpan { start: 0, end: 8, style: FormatStyle::Bold },
            FormatSpan { start: 24, end: 40, style: FormatStyle::Italic },
            FormatSpan {
                start: 55,
                end: 75,
                style: FormatStyle::Link { url: "https://kudzu.site/c".into() },
            },
        ]),
        cursor_offset: Some(76),
        diagnostics: Vec::new(),
        actions: vec![
            RenderAction { offset: 8, action: ActionKind::Key { key: Key::Tab, count: 2 } },
            RenderAction { offset: 41, action: ActionKind::Pause { ms: 300 } },
            RenderAction { offset: 76, action: ActionKind::Key { key: Key::Return, count: 1 } },
        ],
    }
}

/// A preview with bold, italic, a link, key and pause badges and the cursor.
pub fn formatted_text(theme: &Theme) -> El {
    let preview = rich_preview(theme, &sample(), theme.ty.preview, theme.c.foreground, None);
    col().child(line(
        theme,
        "Formatting, badges, cursor",
        [col()
            .w(420.0)
            .child(preview)
            .key("info:formatted-text/preview")],
    ))
}

/// The Echo wordmark, full and small.
pub fn wordmark(theme: &Theme) -> El {
    let ink = theme.c.foreground;
    let full = Variant::Full;
    col().child(line(
        theme,
        "Full (sidebar), small",
        [
            mark(full, 40.0, ink).key(format!("spec:wordmark/full:{}x40", 40.0 * full.aspect())),
            mark(Variant::Small, 20.0, ink).key(format!(
                "spec:wordmark/small:{}x20",
                20.0 * Variant::Small.aspect()
            )),
        ],
    ))
}
