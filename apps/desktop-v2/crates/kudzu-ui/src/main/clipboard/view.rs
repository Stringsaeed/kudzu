//! The screen's layout: the list column (header, search, banner, list or empty state, bulk bar
//! or footer) and the preview column (breadcrumb, a body S59 fills, the item action bar).

use kudzu_core::clipboard::facts::describe;
use kudzu_core::clipboard::query::ClipFilter;
use kudzu_core::clipboard::ClipEntry;
use rune::{col, icon, row, text, El, Tx};

use super::{actions, bulk, list_hooks, toolbar, LIST_KEY};
use crate::clip::filters::nothing_yet;
use crate::clip::list;
use crate::main::state::{clip_filter_label, MainPane};
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::breadcrumbs::{Breadcrumbs, Crumb};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::shortcut::{Shortcut, Tone};

/// `w-[clamp(300px,30vw,372px)]` of the window, hairline on its right.
fn column_width(s: &MainState) -> f32 {
    let chrome = if s.frame.sidebar_open { crate::main::SIDEBAR_W + 8.0 } else { 16.0 };
    (0.30 * (s.clipboard.card_w + chrome)).clamp(300.0, 372.0)
}

/// The screen. Menus and the dialog go on `layer`.
pub fn view(
    s: &MainState,
    theme: &'static Theme,
    filter: ClipFilter,
    pane_focused: bool,
    layer: &mut Vec<El>,
) -> El {
    let c_state = &s.clipboard;
    let header = toolbar::header(s, theme, filter, layer);
    let mut left = col()
        .key("clip-list-column")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .child(header)
        .child(toolbar::search(s, theme));
    let entries = c_state.clips.list.history().entries.len();
    if !c_state.prefs.recording && entries > 0 {
        left = left.child(toolbar::paused_banner(theme));
    }
    left = left.child(body(s, theme, filter));
    left = if c_state.clips.list.is_multi() {
        left.child(bulk::bar(s, theme))
    } else {
        left.child(toolbar::footer(s, theme))
    };
    if pane_focused {
        left = left.focus_selected(true);
    }
    let left = row()
        .key("clip-list-section")
        .role("region")
        .label("Clipboard history")
        .w(column_width(s))
        .h_full()
        .min_h(0.0)
        .child(left)
        .child(toolbar::rule_v(theme));
    bulk::clear_dialog(s, theme, layer);
    row()
        .key("clipboard-screen")
        .w_full()
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .on_drawn_layout_tx(|st: &mut MainState, r, tx: &mut Tx| {
            st.clipboard.card_w = r[2];
            tx.live_only();
        })
        .child(left)
        .child(preview_column(s, theme, filter, layer))
}

/// The list, or what stands in for it.
fn body(s: &MainState, theme: &'static Theme, filter: ClipFilter) -> El {
    let st = &s.clipboard;
    let wrap = |inner: El| col().key(LIST_KEY).grow(1.0).w_full().min_h(0.0).child(inner);
    if st.seen_nothing() {
        return wrap(col()); // loading: the first sync has not landed
    }
    if st.clips.list.history().entries.is_empty() {
        return wrap(welcome(s, theme));
    }
    if st.clips.list.is_empty() {
        return wrap(nothing(s, theme, filter));
    }
    let el = list::view(theme, list_hooks(), "clip-listbox", &st.clips);
    let mut el = el.label("Clipboard history");
    if s.frame.focus.is_active(MainPane::List) {
        el = el.focus_selected(true);
    }
    wrap(el)
}

fn glyph_tile(theme: &Theme, glyph: &'static rune::Icon, size: f32) -> El {
    let c = &theme.c;
    row()
        .size(size, size)
        .center()
        .radius(size * 0.3)
        .fill(c.foreground.alpha(0.04))
        .child(icon(glyph, size * 0.45).color(c.faint))
}

fn welcome(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let prefs = &s.clipboard.prefs;
    let mut body = col()
        .key("clip-welcome")
        .w_full()
        .items_center()
        .px(32.0)
        .pt(64.0)
        .gap(16.0)
        .child(glyph_tile(theme, &icons::CLIPBOARD_LIST, 48.0));
    if prefs.recording {
        let hotkey = s.svc.settings.watch().get().clipboard_hotkey.clone();
        body = body
            .child(
                col()
                    .items_center()
                    .gap(6.0)
                    .child(text("Everything you copy, in one place").font(ty.body_medium).color(c.foreground))
                    .child(
                        text(format!(
                            "Copy text, links, images or files and they appear here with the time you copied them. Kudzu keeps them {}, only on this Mac.",
                            kudzu_core::clipboard::facts::retention_label(prefs.retention_days)
                        ))
                        .font(ty.control_xs)
                        .color(c.muted_foreground)
                        .center(),
                    ),
            )
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(text("Paste from history anywhere").font(ty.control_xs).color(c.muted_foreground))
                    .child(Shortcut::accelerator(&hotkey).tone(Tone::Accent).build(theme)),
            );
    } else {
        body = body
            .child(
                col()
                    .items_center()
                    .gap(6.0)
                    .child(text("Clipboard history is off").font(ty.body_medium).color(c.foreground))
                    .child(
                        text("Turn it on to find anything you copied, from any app.")
                            .font(ty.control_xs)
                            .color(c.muted_foreground),
                    ),
            )
            .child(
                Button::new("Turn on")
                    .size(Size::Sm)
                    .icon(&icons::PLAY)
                    .on_press(|st: &mut MainState| actions::toggle_recording(st))
                    .build(theme),
            );
    }
    body
}

fn nothing(s: &MainState, theme: &Theme, filter: ClipFilter) -> El {
    let c = &theme.c;
    let query = s.clipboard.query().trim();
    let (glyph, message) = if query.is_empty() {
        let glyph = if filter == ClipFilter::Pinned { &icons::PIN } else { &icons::CLIPBOARD_LIST };
        (glyph, text(nothing_yet(filter)).font(theme.ty.body).color(c.muted_foreground))
    } else {
        (
            &icons::SEARCH_X,
            text(format!("Nothing matches “{query}”")).font(theme.ty.body).color(c.muted_foreground),
        )
    };
    col()
        .key("clip-nothing")
        .w_full()
        .items_center()
        .px(24.0)
        .pt(64.0)
        .gap(12.0)
        .child(glyph_tile(theme, glyph, 36.0))
        .child(message)
        .child(
            Button::new("Show everything")
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .on_press_tx(actions::show_everything)
                .build(theme),
        )
}

fn preview_column(s: &MainState, theme: &'static Theme, filter: ClipFilter, layer: &mut Vec<El>) -> El {
    let c = &theme.c;
    let active = s.clipboard.clips.list.active_entry();
    let mut head = row().w_full().h(47.0).px(24.0).gap(8.0).center_y();
    if let Some(e) = active {
        let secret = e.is_secret() && !s.clipboard.prefs.show_secrets;
        let leaf = if secret { "Secret".to_owned() } else { describe(e).title };
        head = head.child(
            Breadcrumbs::new(vec![
                Crumb::new(clip_filter_label(filter)).id("root"),
                Crumb::new(leaf).id(e.id.to_string()),
            ])
            .label("Clipboard item breadcrumb")
            .hug()
            .build(
                theme,
                layer,
                |st: &mut MainState, _| {
                    st.clipboard.blur_inputs();
                    st.frame.focus.move_to(MainPane::List);
                },
                |_: &mut MainState, _| {},
            ),
        );
    }
    let mut column = col()
        .key("clip-preview-column")
        .role("region")
        .label("Preview")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .child(head)
        .child(toolbar::rule_h(theme));
    column = match active {
        Some(e) => column.child(preview_body(s, theme, e)).child(toolbar::item_actions(s, theme, e)),
        None => column.child(
            col()
                .grow(1.0)
                .w_full()
                .center()
                .child(
                    text(if s.clipboard.clips.list.history().entries.is_empty() { "" } else { "Select an item to preview it" })
                        .font(theme.ty.control_xs)
                        .color(c.faint),
                ),
        ),
    };
    column
}

/// S59 replaces this body with the previews: heading, content and details for `entry`. The
/// large preview (`State::large_preview`, set by Space) is S59's too.
fn preview_body(_s: &MainState, _theme: &Theme, _entry: &ClipEntry) -> El {
    col().key("clip-preview-body").grow(1.0).w_full().min_h(0.0)
}
