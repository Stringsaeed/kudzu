//! What the list shows with no rows (`snippet-list.tsx` `EmptyList`, LIB-12): a filter that
//! matches nothing offers Clear filter and Create it; otherwise each view has its own copy, and
//! All, Favorites and a group offer New snippet.

use rune::{col, icon, row, text, Align, El, Icon, TextStyle};

use super::actions;
use crate::main::state::LibraryFilter;
use crate::shell::{MainState, NewSnippet};
use crate::theme::{icons, radius, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

pub const EMPTY_KEY: &str = "library-empty";
pub const NEW_KEY: &str = "empty-new";
pub const CLEAR_KEY: &str = "empty-clear";
pub const CREATE_KEY: &str = "empty-create";

/// A view's title, body and icon.
pub fn copy(scope: &LibraryFilter) -> (&'static str, &'static str, &'static Icon) {
    match scope {
        LibraryFilter::All => (
            "Your library is empty",
            "Snippets turn a short trigger like ;sig into text you type often.",
            &icons::KEYBOARD,
        ),
        LibraryFilter::Favorites => (
            "No favorites yet",
            "Star the snippets you use most to find them faster — they also rank first in search.",
            &icons::STAR,
        ),
        LibraryFilter::Recent => (
            "Nothing used yet",
            "Snippets you expand will show up here.",
            &icons::HISTORY,
        ),
        LibraryFilter::Disabled => (
            "Everything is on",
            "Snippets you turn off appear here.",
            &icons::CIRCLE_CHECK,
        ),
        LibraryFilter::Group(_) => (
            "This group is empty",
            "Create a snippet here or drag snippets onto the group.",
            &icons::FOLDER_OPEN,
        ),
    }
}

/// A quiet icon tile, the editor's empty-state family (`EmptyIcon`).
fn tile(theme: &Theme, glyph: &'static Icon) -> El {
    let c = &theme.c;
    row()
        .size(40.0, 40.0)
        .center()
        .radius(radius::XL)
        .fill(c.foreground.alpha(0.04))
        .border(1.0, c.hairline)
        .child(icon(glyph, 16.0).color(c.muted_foreground).a11y_hidden())
}

/// `<Button size="sm"><Plus /> …</Button>`.
fn new_button(label: &str) -> Button {
    Button::new(label).icon(&icons::PLUS).size(Size::Sm)
}

pub(super) fn view(s: &MainState, theme: &Theme, scope: &LibraryFilter) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let filter = &s.library.filter.text;
    if !filter.is_empty() {
        let line = text(format!("No snippets match “{filter}”"))
            .font(ty.input)
            .color(c.muted_foreground)
            .align(Align::Center);
        let actions = row()
            .gap(8.0)
            .child(
                Button::new("Clear filter")
                    .variant(Variant::Secondary)
                    .size(Size::Sm)
                    .on_press(actions::clear_filter)
                    .build(theme)
                    .key(CLEAR_KEY),
            )
            .child(
                new_button("Create it")
                    .on_press(|s: &mut MainState| {
                        let _ = actions::create_from_filter(s);
                    })
                    .build(theme)
                    .key(CREATE_KEY),
            );
        return col()
            .key(EMPTY_KEY)
            .w_full()
            .px(24.0)
            .pt(64.0)
            .gap(12.0)
            .items_center()
            .child(tile(theme, &icons::SEARCH_X))
            .child(line)
            .child(actions);
    }
    let (title, body, glyph) = copy(scope);
    let mut page = col()
        .key(EMPTY_KEY)
        .w_full()
        .px(32.0)
        .pt(80.0)
        .gap(12.0)
        .items_center()
        .child(tile(theme, glyph))
        .child(
            col()
                .w_full()
                .gap(4.0)
                .items_center()
                .child(
                    text(title)
                        .font(TextStyle {
                            weight: 500.0,
                            ..ty.input
                        })
                        .color(c.foreground)
                        .align(Align::Center),
                )
                .child(
                    text(body)
                        .font(TextStyle {
                            weight: 400.0,
                            line_height: 19.5,
                            ..ty.control_xs
                        })
                        .color(c.muted_foreground)
                        .wrap()
                        .align(Align::Center)
                        .w_full(),
                ),
        );
    if matches!(
        scope,
        LibraryFilter::All | LibraryFilter::Group(_) | LibraryFilter::Favorites
    ) {
        page = page.child(
            new_button("New snippet")
                .on_press(|s: &mut MainState| {
                    let _ = actions::new_snippet(s, NewSnippet::default());
                })
                .build(theme)
                .key(NEW_KEY),
        );
    }
    page
}
