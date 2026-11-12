//! The list column (`main-app.tsx`, `snippet-list.tsx`): `clamp(260px, 27vw, 320px)` wide with
//! a hairline on its right. A 48 point header (the view's title and count, Sort, New snippet),
//! the filter inset 12, the virtual rows (padding 4 above and 8 below), then the bulk bar.

use std::borrow::Cow;

use rune::{col, row, text, Cx, El, GrowthSpec, ScrollSpec, TextStyle};
use rune_kit::{Menu, MenuItem, Placement};

use super::model::Sort;
use super::row::{self, Look, ROW_H};
use super::{bulk, empty, keys, row_key, sync};
use super::{FILTER_KEY, LIST_KEY, NEW_KEY, SCROLL_KEY, SORT_KEY};
use crate::main::state::{LibraryFilter, MainPane};
use crate::shell::{MainState, NewSnippet};
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};
use crate::widgets::kit_ext::input::Input;
use crate::widgets::kit_ext::menu;
use crate::widgets::shortcut::{Shortcut, Tone};

/// The column's width in a window `window` points wide (`clamp(260px, 27vw, 320px)`).
pub fn column_width(window: f32) -> f32 {
    (window * 0.27).clamp(260.0, 320.0)
}

/// The header's title for a view (`viewTitle`).
pub fn title(s: &MainState, scope: &LibraryFilter) -> String {
    match scope {
        LibraryFilter::All => "All snippets".into(),
        LibraryFilter::Favorites => "Favorites".into(),
        LibraryFilter::Recent => "Recently used".into(),
        LibraryFilter::Disabled => "Turned off".into(),
        LibraryFilter::Group(id) => s
            .library_snapshot()
            .groups
            .iter()
            .find(|g| g.id == *id)
            .map_or_else(|| "Group".into(), |g| g.name.clone()),
    }
}

pub fn view(
    s: &MainState,
    cx: &Cx<MainState>,
    theme: &Theme,
    scope: &LibraryFilter,
    pane_focused: bool,
    overlays: &mut Vec<El>,
) -> El {
    let c = &theme.c;
    let window = cx.layout_with("main-toolbar", |r| r[2]).unwrap_or(1180.0);
    let width = column_width(window);
    let inner = width - 1.0;
    let title = title(s, scope);
    let count = s.library.listing(s).map_or(0, |l| l.ids.len());

    let mut region = col()
        .key("library-pane")
        .role("region")
        .label("Snippets")
        .w(inner)
        .h_full()
        .min_h(0.0)
        .child(header(s, theme, &title, count, overlays))
        .child(filter(s, theme, inner))
        .child(list(s, theme, scope, &title, inner, pane_focused));
    if let Some(bar) = bulk::bar(s, theme, overlays) {
        region = region.child(bar);
    }
    if pane_focused {
        region = region.focus_selected(true);
    }
    col()
        .key("library-column")
        .w(width)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], c.hairline)
        .child(region)
}

fn header(s: &MainState, theme: &Theme, title: &str, count: usize, overlays: &mut Vec<El>) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let heading = row()
        .key("library-heading")
        .role("heading")
        .label(title.to_owned())
        .grow(1.0)
        .min_w(0.0)
        .h(20.0)
        .center_y()
        .child(
            text(title.to_owned())
                .font(TextStyle {
                    weight: 600.0,
                    ..ty.body
                })
                .color(c.foreground)
                .truncate()
                .min_w(0.0),
        )
        .child(
            text(count.to_string())
                .font(ty.body.features("'tnum' 1"))
                .color(c.faint)
                .ml(4.0),
        );
    let sort = Button::new("")
        .icon(&icons::ARROW_DOWN_UP)
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .ink(c.muted_foreground)
        .name("Sort")
        .expanded(s.library.sort_open)
        .on_press(|s: &mut MainState| s.library.sort_open = !s.library.sort_open)
        .build(theme)
        .key(SORT_KEY)
        .no_window_drag();
    let mut menu_items = Menu::new("library-sort-menu")
        .trigger(sort)
        .open(s.library.sort_open)
        .placement(Placement::BottomEnd)
        .min_width(176.0)
        .on_close(|s: &mut MainState| s.library.sort_open = false)
        .group_label("Sort by");
    for option in Sort::ALL {
        menu_items = menu_items.item(
            MenuItem::new(option.label())
                .checked(s.library.sort == option)
                .on_select(move |s: &mut MainState| {
                    s.library.sort = option;
                    s.library.sort_open = false;
                    sync(s);
                }),
        );
    }
    let sort = menu::build(menu_items, theme, overlays);
    let new = Button::new("")
        .icon(&icons::PLUS)
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .ink(c.muted_foreground)
        .name("New snippet")
        .on_press(|s: &mut MainState| {
            let _ = super::actions::new_snippet(s, NewSnippet::default());
        })
        .build(theme)
        .key(NEW_KEY)
        .no_window_drag();
    row()
        .key("library-header")
        .w_full()
        .h(48.0)
        .pl(20.0)
        .pr(8.0)
        .gap(8.0)
        .center_y()
        .window_drag()
        .child(heading)
        .child(sort)
        .child(new)
}

fn filter(s: &MainState, theme: &Theme, inner: f32) -> El {
    let c = &theme.c;
    let typed = !s.library.filter.text.is_empty();
    let trailing = if typed {
        Button::new("")
            .icon(&icons::X)
            .variant(Variant::Ghost)
            .size(Size::IconXs)
            .ink(c.faint)
            .name("Clear filter")
            .on_press(super::actions::clear_filter)
            .build(theme)
            .size(20.0, 20.0)
            .mr(8.0)
    } else {
        row()
            .pr(8.0)
            .child(Shortcut::combo("mod+f").tone(Tone::Ghost).build(theme))
    };
    let field = Input::new(&s.library.filter)
        .leading_icon(&icons::SEARCH)
        .icon_at(10.0)
        .with(|t| {
            t.placeholder("Filter by name, trigger or text")
                .label("Filter snippets")
                .width(Some(inner - 24.0))
                .leading_icon_size(14.0)
                .spellcheck(false)
                .trailing(trailing)
                .on_edit(|st: &mut MainState| &mut st.library.filter)
                .on_change(|st: &mut MainState, _| {
                    sync(st);
                })
                .on_key(keys::filter)
        })
        .text_style(theme.ty.body)
        .build(theme)
        .key(FILTER_KEY);
    let field = if s.library.filter.focused {
        field
    } else {
        field.fill(c.foreground.alpha(0.03))
    };
    row()
        .key("library-filter-row")
        .w_full()
        .px(12.0)
        .pb(8.0)
        .child(field)
}

fn list(
    s: &MainState,
    theme: &Theme,
    scope: &LibraryFilter,
    title: &str,
    inner: f32,
    pane_focused: bool,
) -> El {
    let listbox = col()
        .key(LIST_KEY)
        .role("listbox")
        .label(format!("{title} snippets"))
        .focusable(true)
        .on_key(keys::list)
        .w_full()
        .grow(1.0)
        .min_h(0.0);
    let Some(listing) = s.library.listing(s) else {
        return listbox;
    };
    if listing.ids.is_empty() {
        return listbox.child(empty::view(s, theme, scope));
    }
    let motion = s.library.motion();
    let dark = theme.dark;
    let list_focused =
        pane_focused && s.frame.focus.is_active(MainPane::List) && !s.library.filter.focused;
    // `max-w-[45%]` of the item's content: the column less the item's margins and padding.
    let trigger_max = ((inner - 16.0 - 26.0) * 0.45).max(0.0) as u16;
    let mut rows = col()
        .key("library-rows")
        .w_full()
        .pt(4.0)
        .pb(8.0)
        .growth(GrowthSpec::default())
        .virtual_rows(
            listing.ids.len(),
            ROW_H,
            |s: &MainState, i| {
                let id = s.library.listing(s).and_then(|l| l.ids.get(i).copied());
                Cow::Owned(id.map(row_key).unwrap_or_default())
            },
            move |s: &MainState, i| {
                let theme = if dark { Theme::dark() } else { Theme::light() };
                let Some(listing) = s.library.listing(s) else {
                    return col();
                };
                let Some(hit) = listing.hits.get(i) else {
                    return col();
                };
                let data = listing.indexed.row(hit);
                let cursor = s.frame.selected_snippet == Some(data.id);
                let sel = &s.library.selection;
                let look = Look {
                    cursor,
                    selected: sel.includes(data.id, s.frame.selected_snippet),
                    active: list_focused || sel.multi().len() <= 1,
                    fresh: motion.fresh.contains(&data.id),
                    dark,
                    trigger_max,
                };
                row::build(data, look, theme)
            },
        )
        .virtual_rows_version(listing.version);
    // Still: rows land at once, and so does the list itself when it comes back from an empty
    // state (a cleared filter), as the Tauri list's rows appear without an entrance.
    let listbox = if motion.still {
        rows = rows.instant_changes();
        listbox.instant_changes()
    } else {
        listbox
    };
    listbox.child(
        col()
            .key(SCROLL_KEY)
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(rows),
    )
}
