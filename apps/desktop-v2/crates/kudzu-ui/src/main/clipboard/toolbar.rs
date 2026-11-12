//! The list column's header (title, recording badge, options menu), search field, paused
//! banner and retention footer, and the preview column's item action bar.

use kudzu_core::clipboard::facts::{format_bytes, has_text, retention_label};
use kudzu_core::clipboard::query::ClipFilter;
use kudzu_core::clipboard::ClipEntry;
use kudzu_core::clipboard::ClipKind;
use rune::{col, icon, row, text, El};
use rune_kit::{Menu, MenuEntry, MenuItem, Placement};

use super::{actions, SEARCH_KEY};
use crate::clip::filters::heading;
use crate::main::state::MainView;
use crate::shell::{MainState, SettingsPane};
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::input::Input;
use crate::widgets::kit_ext::menu;
use crate::widgets::shortcut::{Shortcut, Tone};

/// Retention choices of the options menu (`RETENTION_OPTIONS`).
pub const RETENTION: [(u32, &str); 6] = [
    (1, "1 day"),
    (7, "1 week"),
    (30, "1 month"),
    (90, "3 months"),
    (365, "1 year"),
    (0, "Forever"),
];

fn retention_name(days: u32) -> String {
    RETENTION
        .iter()
        .find(|(d, _)| *d == days)
        .map_or_else(|| format!("{days} days"), |(_, l)| (*l).to_owned())
}

/// The 48 pt header: title and count, the recording badge, the options button.
pub fn header(s: &MainState, theme: &Theme, filter: ClipFilter, layer: &mut Vec<El>) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let on = s.clipboard.prefs.recording;
    let title = row()
        .role("heading")
        .label(heading(filter))
        .grow(1.0)
        .min_w(0.0)
        .gap(4.0)
        .child(text(heading(filter)).font(ty.body_strong).color(c.foreground).single_line())
        .child(
            text(s.clipboard.clips.list.len().to_string())
                .font(ty.meta_11_5)
                .color(c.faint),
        );
    let badge_ink = if on { c.muted_foreground } else { c.warning };
    let dot = row().size(6.0, 6.0).radius(3.0).fill(if on { c.success } else { c.warning });
    let mut badge = row()
        .key("clip-recording")
        .role("button")
        .label(if on {
            "Recording copies. Press to pause."
        } else {
            "Paused. Press to resume recording."
        })
        .focusable(true)
        .h(24.0)
        .px(8.0)
        .gap(6.0)
        .center_y()
        .radius(12.0)
        .child(dot)
        .child(text(if on { "Recording" } else { "Paused" }).font(ty.meta_11).color(badge_ink))
        .on_press(|st: &mut MainState| actions::toggle_recording(st));
    badge = if on {
        badge.hover(c.foreground.alpha(0.05))
    } else {
        badge.fill(c.warning.alpha(0.12)).hover(c.warning.alpha(0.18))
    };
    let options = options_menu(s, theme, layer);
    row()
        .key("clip-header")
        .w_full()
        .h(48.0)
        .pl(20.0)
        .pr(8.0)
        .gap(8.0)
        .center_y()
        .child(title)
        .child(badge)
        .child(options)
}

fn options_menu(s: &MainState, theme: &Theme, layer: &mut Vec<El>) -> El {
    let on = s.clipboard.prefs.recording;
    let days = s.clipboard.prefs.retention_days;
    let empty = s.clipboard.clips.list.history().entries.is_empty();
    let trigger = Button::new("Clipboard options")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .icon(&icons::MORE_HORIZONTAL)
        .expanded(s.clipboard.options_open)
        .help("Options")
        .on_press(|st: &mut MainState| st.clipboard.options_open = !st.clipboard.options_open)
        .build(theme);
    let retention: Vec<MenuEntry> = RETENTION
        .iter()
        .map(|&(d, label)| {
            MenuEntry::item(MenuItem::new(label).checked(d == days).on_select(
                move |st: &mut MainState| {
                    actions::set_retention(st, d);
                    st.clipboard.options_open = false;
                    st.clipboard.retention_open = false;
                },
            ))
        })
        .collect();
    let m = Menu::new("clip-options")
        .trigger(trigger)
        .item(
            MenuItem::new(if on { "Pause recording" } else { "Resume recording" })
                .icon(if on { &icons::PAUSE } else { &icons::PLAY })
                .on_select(|st: &mut MainState| {
                    actions::toggle_recording(st);
                    st.clipboard.options_open = false;
                }),
        )
        .item(
            MenuItem::new("Keep history")
                .icon(&icons::CLIPBOARD_LIST)
                .badge(retention_name(days))
                .submenu(retention, s.clipboard.retention_open),
        )
        .item(
            MenuItem::new("Clipboard settings…")
                .icon(&icons::SETTINGS_2)
                .on_select(|st: &mut MainState| {
                    st.clipboard.options_open = false;
                    st.navigate_to(MainView::Settings(SettingsPane::Clipboard));
                }),
        )
        .separator()
        .item(
            MenuItem::new("Clear history…")
                .icon(&icons::TRASH_2)
                .danger(true)
                .disabled(empty)
                .on_select(|st: &mut MainState| {
                    st.clipboard.options_open = false;
                    actions::open_clear(st);
                }),
        )
        .placement(Placement::BottomEnd)
        .min_width(240.0)
        .open(s.clipboard.options_open)
        .on_close(|st: &mut MainState| {
            st.clipboard.options_open = false;
            st.clipboard.retention_open = false;
        });
    menu::build(m, theme, layer)
}

/// The search field: ⌘F hint at rest, a clear button with text.
pub fn search(s: &MainState, theme: &Theme) -> El {
    let trailing = if s.clipboard.has_query() {
        row()
            .role("button")
            .label("Clear search")
            .focusable(true)
            .size(20.0, 20.0)
            .center()
            .radius(4.0)
            .child(icon(&icons::X, 14.0).color(theme.c.faint))
            .on_press(|st: &mut MainState| st.clipboard.search.replace_text(""))
            // The field's trailing slot ends 11 pt in; the reference's `right-2` is 8.
            .offset(3.0, 0.0)
    } else {
        Shortcut::combo("mod+f").build(theme)
    };
    let field = Input::new(&s.clipboard.search)
        .leading_icon(&icons::SEARCH)
        .icon_at(10.0)
        .text_style(theme.ty.body)
        .with(|t| {
            t.placeholder("Search history")
                .on_edit(|st: &mut MainState| &mut st.clipboard.search)
                .on_key(super::keys::handle)
                .trailing(trailing)
        })
        .build(theme)
        .key(SEARCH_KEY)
        .role("textbox")
        .label("Search clipboard history");
    col().w_full().px(12.0).pb(8.0).child(field)
}

/// "Recording is paused. New copies aren't saved." with Resume.
pub fn paused_banner(theme: &Theme) -> El {
    let c = &theme.c;
    row()
        .key("clip-paused")
        .role("status")
        .label("Recording is paused. New copies aren't saved.")
        .mx(12.0)
        .mb(8.0)
        .px(12.0)
        .py(8.0)
        .gap(8.0)
        .center_y()
        .radius(8.0)
        .fill(c.warning.alpha(0.10))
        .child(icon(&icons::PAUSE, 14.0).color(c.warning))
        .child(
            text("Recording is paused. New copies aren't saved.")
                .font(theme.ty.control_xs)
                .color(c.warning)
                .grow(1.0),
        )
        .child(
            row()
                .role("button")
                .label("Resume")
                .focusable(true)
                .child(text("Resume").font(theme.ty.control_xs).color(c.warning))
                .on_press(|st: &mut MainState| actions::toggle_recording(st)),
        )
}

/// The 36 pt footer: how long clips are kept, how much room they take, and a way to change it.
pub fn footer(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let mut kept = format!("Kept {}", retention_label(s.clipboard.prefs.retention_days));
    if let Some(bytes) = s.clipboard.usage.filter(|&b| b > 0) {
        kept.push_str(&format!(" · {}", format_bytes(bytes)));
    }
    let bar = row()
        .w_full()
        .h(35.0)
        .px(16.0)
        .gap(4.0)
        .center_y()
        .child(text(kept.clone()).font(theme.ty.meta_11).color(c.faint).single_line().grow(1.0))
        .child(
            row()
                .role("button")
                .label("Change")
                .focusable(true)
                .px(4.0)
                .h(20.0)
                .center_y()
                .radius(4.0)
                .hover(c.foreground.alpha(0.05))
                .child(text("Change").font(theme.ty.meta_11).color(c.muted_foreground))
                .on_press(|st: &mut MainState| st.navigate_to(MainView::Settings(SettingsPane::Clipboard))),
        );
    col()
        .key("clip-footer")
        .role("contentinfo")
        .label(kept)
        .w_full()
        .h(36.0)
        .child(rule_h(theme))
        .child(bar)
}

/// A 1 pt hairline across.
pub(super) fn rule_h(theme: &Theme) -> El {
    row().w_full().h(1.0).fill(theme.c.hairline)
}

/// A 1 pt hairline down.
pub(super) fn rule_v(theme: &Theme) -> El {
    row().w(1.0).h_full().fill(theme.c.hairline)
}

/// The preview column's 56 pt action bar for the active clip (`aria-label="Item actions"`).
pub fn item_actions(s: &MainState, theme: &Theme, e: &ClipEntry) -> El {
    let copied = s.clipboard.clips.look.copied == Some(e.id);
    let mut bar = row()
        .key("clip-item-actions")
        .role("toolbar")
        .label("Item actions")
        .w_full()
        .h(55.0)
        .px(16.0)
        .gap(4.0)
        .center_y();
    bar = bar.child(
        Button::new("Copy")
            .size(Size::Sm)
            .icon(if copied { &icons::CHECK } else { &icons::COPY })
            .on_press_tx(|st: &mut MainState, tx| actions::copy_active(st, false, tx))
            .build(theme)
            .child(Shortcut::combo("enter").tone(Tone::OnAccent).build(theme)),
    );
    if e.rich {
        let label = if s.clipboard.prefs.paste_plain { "With formatting" } else { "As plain text" };
        bar = bar.child(
            Button::new(label)
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .icon(&icons::TYPE)
                .on_press_tx(|st: &mut MainState, tx| actions::copy_active(st, true, tx))
                .build(theme)
                .child(Shortcut::combo("shift+enter").tone(Tone::Ghost).build(theme).ml(-3.0)),
        );
    }
    bar = bar.child(row().grow(1.0));
    let icon_button = |label: &str, glyph: &'static rune::Icon, f: fn(&mut MainState, &mut rune::Tx)| {
        Button::new(label)
            .variant(Variant::Ghost)
            .size(Size::IconSm)
            .icon(glyph)
            .help(label)
            .on_press_tx(f)
            .build(theme)
    };
    if has_text(e) {
        bar = bar.child(icon_button("Save as snippet", &icons::TEXT_CURSOR_INPUT, |st, _| {
            actions::save_as_snippet(st)
        }));
    }
    if matches!(e.kind, ClipKind::Link | ClipKind::File) {
        let label = if e.kind == ClipKind::Link { "Open link" } else { "Open" };
        bar = bar.child(icon_button(label, &icons::EXTERNAL_LINK, actions::open_active));
    }
    if e.kind == ClipKind::File {
        bar = bar.child(icon_button("Show in Finder", &icons::FOLDER_OPEN, |st, _| {
            actions::reveal_active(st)
        }));
    }
    if has_text(e) {
        let (label, glyph) = if e.is_secret() {
            ("Not a secret — show it", &icons::EYE_OFF)
        } else {
            ("Hide as a secret", &icons::KEY_ROUND)
        };
        bar = bar.child(icon_button(label, glyph, |st, _| actions::toggle_secret(st)));
    }
    let (pin_label, pin_glyph) = if e.pinned {
        ("Unpin", &icons::PIN_OFF)
    } else {
        ("Pin — keep forever", &icons::PIN)
    };
    bar = bar.child(icon_button(pin_label, pin_glyph, |st, _| actions::toggle_pin(st)));
    let bar = bar.child(icon_button("Delete", &icons::TRASH_2, actions::delete_selection));
    col().w_full().h(56.0).child(rule_h(theme)).child(bar)
}
