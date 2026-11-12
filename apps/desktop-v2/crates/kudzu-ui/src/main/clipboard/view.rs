//! The screen's layout: the list column (header, search, banner, list or empty state, bulk bar
//! or footer) and the preview column (breadcrumb, a body S59 fills, the item action bar).

use kudzu_core::clipboard::facts::describe;
use kudzu_core::clipboard::query::ClipFilter;
use kudzu_core::clipboard::ClipEntry;
use rune::{col, icon, row, text, El, ScrollSpec, Tx};

use super::{actions, bulk, list_hooks, toolbar, LIST_KEY};
use crate::clip::content::Body;
use crate::clip::filters::{glyph, nothing_yet};
use crate::clip::preview::{self, Preview};
use crate::clip::{large, list};
use crate::main::state::{clip_filter_key, clip_filter_label, MainPane};
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::breadcrumbs::{Breadcrumbs, Crumb};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::shortcut::{Shortcut, Tone};

/// `w-[clamp(300px,30vw,372px)]` of the window, hairline on its right.
fn column_width(s: &MainState) -> f32 {
    let chrome = if s.frame.sidebar_open {
        crate::main::SIDEBAR_W + 8.0
    } else {
        16.0
    };
    list_width(s.clipboard.card_w + chrome)
}

/// 30% of the window between 300 and 372 pt.
pub(super) fn list_width(window_w: f32) -> f32 {
    (0.30 * window_w).clamp(300.0, 372.0)
}

#[cfg(test)]
thread_local! {
    /// How many times the screen's view has run, for the scroll trace.
    pub(crate) static VIEWS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The screen. Menus and the dialog go on `layer`.
pub fn view(
    s: &MainState,
    theme: &'static Theme,
    filter: ClipFilter,
    pane_focused: bool,
    layer: &mut Vec<El>,
) -> El {
    #[cfg(test)]
    VIEWS.set(VIEWS.get() + 1);
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
    large_preview(s, theme, layer);
    row()
        .key("clipboard-screen")
        .w_full()
        .h_full()
        .min_w(0.0)
        .min_h(0.0)
        .on_drawn_layout_tx(|st: &mut MainState, r, tx: &mut Tx| {
            // A new width rebuilds the view (the column is a clamp of it); the same one doesn't.
            if (st.clipboard.card_w - r[2]).abs() > 0.5 || (st.clipboard.card_h - r[3]).abs() > 0.5
            {
                st.clipboard.card_w = r[2];
                st.clipboard.card_h = r[3];
            } else {
                tx.live_only();
            }
        })
        .child(left)
        .child(preview_column(s, theme, filter, layer))
        .child(status(s))
}

/// What the last pin, copy, delete or clear did, for assistive technology to speak. Present
/// but never drawn.
fn status(s: &MainState) -> El {
    row()
        .key("clip-announcement")
        .role("status")
        .label(s.clipboard.announcement.clone())
        .absolute(0.0, 0.0)
        .size(1.0, 1.0)
        .opacity(0.0)
}

/// The list, or what stands in for it.
fn body(s: &MainState, theme: &'static Theme, filter: ClipFilter) -> El {
    let st = &s.clipboard;
    let wrap = |inner: El| {
        col()
            .key(LIST_KEY)
            .grow(1.0)
            .w_full()
            .min_h(0.0)
            .child(inner)
    };
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
                    .child(
                        text("Clipboard history is off")
                            .font(ty.body_medium)
                            .color(c.foreground),
                    )
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
        let glyph = glyph(filter);
        (
            glyph,
            text(nothing_yet(filter))
                .font(theme.ty.body)
                .color(c.muted_foreground),
        )
    } else {
        (
            &icons::SEARCH_X,
            text(format!("Nothing matches “{query}”"))
                .font(theme.ty.body)
                .color(c.muted_foreground),
        )
    };
    col()
        .key("clip-nothing")
        .w_full()
        .items_center()
        .px(24.0)
        .pt(64.0)
        .gap(12.0)
        .child(
            glyph_tile(theme, glyph, 36.0).key(format!("clip-nothing-{}", clip_filter_key(filter))),
        )
        .child(message)
        .child(
            Button::new("Show everything")
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .on_press_tx(actions::show_everything)
                .build(theme),
        )
}

fn preview_column(
    s: &MainState,
    theme: &'static Theme,
    filter: ClipFilter,
    layer: &mut Vec<El>,
) -> El {
    let c = &theme.c;
    let active = s.clipboard.clips.list.active_entry();
    let mut head = row().w_full().h(47.0).px(24.0).gap(8.0).center_y();
    if let Some(e) = active {
        let secret = e.is_secret() && !s.clipboard.prefs.show_secrets;
        let leaf = if secret {
            "Secret".to_owned()
        } else {
            describe(e).title
        };
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
            )
            .shrink()
            .min_w(0.0),
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
        Some(e) => column
            .child(preview_body(s, theme, e))
            .child(toolbar::item_actions(s, theme, e)),
        None => column.child(
            col().grow(1.0).w_full().center().child(
                text(if s.clipboard.clips.list.history().entries.is_empty() {
                    ""
                } else {
                    "Select an item to preview it"
                })
                .font(theme.ty.control_xs)
                .color(c.faint),
            ),
        ),
    };
    column
}

fn preview_hooks() -> crate::clip::preview::Hooks<MainState> {
    use crate::clip::content::Hooks as Content;
    crate::clip::preview::Hooks {
        content: Content {
            open_link: |s, tx| actions::open_active(s, tx),
            open_file: |s, i, _| actions::open_file(s, i),
            reveal_file: |s, i, _| actions::reveal_file(s, i),
            copy_value: |s, v, tx| actions::copy_value(s, v, tx),
        },
        not_a_secret: |s, _| actions::not_a_secret(s),
        toggle_formats: |s, _| s.clipboard.formats_open = !s.clipboard.formats_open,
    }
}

/// What one preview needs from the screen's state.
pub(super) fn preview_of<'a>(s: &'a MainState, entry: &'a ClipEntry, large: bool) -> Preview<'a> {
    let st = &s.clipboard;
    Preview {
        body: Body {
            entry,
            full: st.full.get().filter(|_| st.seen_active_is(entry.id)),
            terms: &st.clips.look.terms,
            large,
            media: st.clips.media.as_ref(),
            copied_value: st.copied_value.as_ref().map(|(v, _)| v.as_str()),
        },
        show_secrets: st.prefs.show_secrets,
        retention_days: st.prefs.retention_days,
        now: st.clips.look.now,
        formats_open: st.formats_open,
        compact: false,
    }
}

/// The scrolling body of the preview column: heading, content and details of the clip.
fn preview_body(s: &MainState, theme: &Theme, entry: &ClipEntry) -> El {
    let p = preview_of(s, entry, false);
    col()
        .key("clip-preview-body")
        .grow(1.0)
        .w_full()
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .w_full()
                .px(24.0)
                .py(20.0)
                .child(preview::column(theme, &p, preview_hooks())),
        )
}

/// The large preview, when Space opened it.
fn large_preview(s: &MainState, theme: &Theme, layer: &mut Vec<El>) {
    let Some(entry) = s
        .clipboard
        .large_preview
        .then(|| s.clipboard.clips.list.active_entry())
        .flatten()
    else {
        return;
    };
    let chrome = if s.frame.sidebar_open {
        crate::main::SIDEBAR_W + 8.0
    } else {
        16.0
    };
    let window = (s.clipboard.card_w + chrome, s.clipboard.card_h + 58.0);
    let title = if entry.is_secret() && !s.clipboard.prefs.show_secrets {
        "Secret".to_owned()
    } else {
        describe(entry).title
    };
    let p = preview_of(s, entry, true);
    large::build(
        theme,
        &p,
        preview_hooks(),
        &title,
        window,
        |st: &mut MainState| st.clipboard.large_preview = false,
        layer,
    );
}

#[cfg(test)]
mod tests {
    use super::list_width;

    #[test]
    fn the_list_column_is_thirty_percent_of_the_window_between_300_and_372() {
        assert_eq!(
            list_width(900.0),
            300.0,
            "the minimum window gives the floor"
        );
        assert_eq!(list_width(1000.0), 300.0);
        assert!((list_width(1180.0) - 354.0).abs() < 0.001);
        assert_eq!(list_width(1240.0), 372.0);
        assert_eq!(
            list_width(2400.0),
            372.0,
            "wide windows stop at the ceiling"
        );
    }
}
