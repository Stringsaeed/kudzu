//! The picker panel. The card is inset 12 points so the captured landmarks (dialog, field,
//! pills, preview, footer) line up; the live vibrancy material is full-bleed, and that frame
//! is an S71 check.

use kudzu_core::clipboard::ClipEntry;
use rune::{col, icon, row, text, Cx, El, ScrollSpec};

use super::drive::{list_hooks, on_input_key, FIELD_KEY};
use super::footer::footer;
use super::model::{Empty, Picker};
use crate::clip::content::{Body, Hooks as Content};
use crate::clip::filters::pills;
use crate::clip::list;
use crate::clip::preview::{self, Hooks as PreviewHooks, Preview};
use crate::shell::windows::PickerState;
use crate::theme::{icons, radius, Appearance, Theme};
use crate::widgets::key_hints::release_on_key_up;

#[cfg(test)]
thread_local! {
    /// How many times the panel's view has run, for the scroll trace.
    pub(crate) static VIEWS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub fn view(s: &PickerState, cx: &mut Cx<PickerState>) -> El {
    #[cfg(test)]
    VIEWS.set(VIEWS.get() + 1);
    // Headless `--theme` is the system appearance. A running app follows Settings.
    let appearance = if rune::is_headless() {
        Appearance::System
    } else {
        s.model.theme.appearance()
    };
    let theme = Theme::resolve(appearance, cx.system_dark());
    let p = &s.model.picker;
    let mut overlays: Vec<El> = Vec::new();
    let card = col()
        .key("picker-card")
        .role("dialog")
        .label("Clipboard history")
        .w_full()
        .h_full()
        .radius(radius::XL2)
        .clip_rounded(radius::XL2)
        .fill(theme.c.popover)
        .child(search_row(p, theme))
        .child(
            row()
                .key("picker-body")
                .grow(1.0)
                .min_h(0.0)
                .w_full()
                .child(results(p, theme))
                .child(aside(p, theme, &mut overlays)),
        )
        .child(footer(p, theme));
    let root = col()
        .key("picker")
        .w_full()
        .h_full()
        .padding(12.0)
        .child(card);
    let root = release_on_key_up(root, |s: &mut PickerState| &mut s.model.picker.held);
    rune_kit::root(root, overlays)
}

fn search_row(p: &Picker, theme: &Theme) -> El {
    let c = &theme.c;
    // The icon is centred in the 58 pt row; the line box shares that centre, and the padding
    // is inside the border box so the combobox rect stays put.
    let line = theme.ty.search.line_px();
    let pad = ((58.0 - line) / 2.0).max(0.0);
    let field = rune::text_input(&p.edit, theme.ty.search)
        .key(FIELD_KEY)
        .grow(1.0)
        .h_full()
        .min_w(0.0)
        .py(pad)
        .single_line()
        .color(c.foreground)
        .placeholder("Search clipboard history…", c.faint)
        .caret(c.foreground, theme.kit().selection())
        .on_edit(|s: &mut PickerState| &mut s.model.picker.edit)
        .on_input_key(on_input_key)
        .autofocus()
        .role("combobox")
        .label("Search clipboard history");
    row()
        .key("picker-search")
        .h(58.0)
        .w_full()
        .px(16.0)
        .gap(12.0)
        .center_y()
        .border_sides([0.0, 0.0, 0.5, 0.0], c.hairline)
        .child(icon(&icons::SEARCH, 18.0).color(c.muted_foreground))
        .child(field)
        .child(pills(
            theme,
            "picker-pills",
            p.filter,
            |s: &mut PickerState, f, _| {
                s.model.picker.filter = f;
                super::drive::pull(s);
            },
        ))
}

fn results(p: &Picker, theme: &'static Theme) -> El {
    let c = &theme.c;
    let column = col()
        .key("picker-results")
        .w_frac(0.5)
        .h_full()
        .min_h(0.0)
        .pt(6.0);
    let rows = p.clips.list.len();
    if let Some(empty) = Empty::of(p.prefs.recording, p.total, p.query(), rows) {
        let glyph = if empty.searching() {
            &icons::SEARCH_X
        } else {
            &icons::CLIPBOARD_LIST
        };
        return column.child(
            col()
                .key("picker-empty")
                .w_full()
                .grow(1.0)
                .center()
                .gap(8.0)
                .px(32.0)
                .child(
                    row()
                        .size(36.0, 36.0)
                        .center()
                        .radius(radius::XL)
                        .fill(c.foreground.alpha(0.04))
                        .child(icon(glyph, 16.0).color(c.faint)),
                )
                .child(
                    text(empty.title())
                        .font(theme.ty.body)
                        .color(c.muted_foreground)
                        .center(),
                )
                .child(
                    text(empty.hint())
                        .font(theme.ty.control_xs)
                        .color(c.faint)
                        .center(),
                ),
        );
    }
    let label = if p.query().is_empty() {
        "Recent copies"
    } else {
        "Matches"
    };
    column.child(list::view(theme, list_hooks(), "picker-listbox", &p.clips).label(label))
}

fn preview_hooks() -> PreviewHooks<PickerState> {
    PreviewHooks {
        content: Content {
            open_link: |s, tx| super::drive::apply(s, super::keys::Effect::Open, tx),
            open_file: |s, _, tx| super::drive::apply(s, super::keys::Effect::Open, tx),
            reveal_file: |_, _, _| {},
            copy_value: |_, value, tx| tx.copy(value),
        },
        not_a_secret: |s, _| {
            if let Some(id) = s.model.picker.clips.list.active() {
                let _ = s.svc.clipboard.set_secret(id, Some(false));
            }
        },
        toggle_formats: |_, _| {},
        reveal: |s, on, _| s.model.picker.cover_hover = on,
        hover_dot: |_, _, _| {},
    }
}

fn preview_of<'a>(p: &'a Picker, entry: &'a ClipEntry) -> Preview<'a> {
    Preview {
        body: Body {
            entry,
            full: p.full.get().filter(|_| {
                p.seen
                    .active
                    .as_ref()
                    .is_some_and(|(id, _)| *id == entry.id)
            }),
            terms: &p.clips.look.terms,
            large: false,
            media: p.clips.media.as_ref(),
            copied_value: None,
        },
        show_secrets: p.prefs.show_secrets,
        retention_days: p.prefs.retention_days,
        now: p.clips.look.now,
        formats_open: false,
        compact: true,
        revealed: p.cover_hover,
        hover_dot: None,
    }
}

/// The live preview: heading, the clip's body, and the compact details under a hairline.
fn aside(p: &Picker, theme: &Theme, layer: &mut Vec<El>) -> El {
    let c = &theme.c;
    let aside = row()
        .key("picker-aside")
        .role("complementary")
        .label("Preview")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .fill(c.foreground.alpha(0.015));
    let rule = row().w(1.0).h_full().fill(c.hairline);
    let Some(entry) = p.active() else {
        return aside.child(rule);
    };
    let hooks = preview_hooks();
    let pv = preview_of(p, entry);
    let body = col()
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .child(
            col()
                .w_full()
                .px(20.0)
                .pt(14.0)
                .pb(10.0)
                .child(preview::heading(theme, &pv, hooks)),
        )
        .child(
            col()
                .key("picker-preview-body")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    col()
                        .w_full()
                        .px(20.0)
                        .pb(16.0)
                        .child(preview::content(theme, &pv, hooks)),
                ),
        )
        .child(row().w_full().h(1.0).fill(c.hairline))
        .child(
            col()
                .w_full()
                .px(20.0)
                .py(12.0)
                .child(preview::details_section(theme, &pv, hooks, layer)),
        );
    aside.child(rule).child(body)
}
