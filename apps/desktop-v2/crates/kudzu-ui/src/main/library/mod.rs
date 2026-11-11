//! Library list screen stub. Owned and filled out by S50 (`crates/kudzu-ui/src/main/library/**`).

pub mod actions;

use rune::{col, text, El, TextEdit, Tx};

use crate::main::state::LibraryFilter;
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::input::Input;

pub const FILTER_KEY: &str = "library-filter";
pub const LIST_KEY: &str = "library-list-slot";

/// Per-screen state owned by the library list module (S50).
#[derive(Clone, Debug, Default)]
pub struct State {
    pub filter: TextEdit,
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.filter.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.filter.focused
    }

    pub fn focus_filter(&mut self, tx: &mut Tx) {
        self.filter.focused = true;
        tx.focus(FILTER_KEY, true);
    }

    pub fn blur_inputs(&mut self) {
        self.filter.focused = false;
    }
}

/// Placeholder library list pane until S50 lands. Provides the `"Snippets"` region, heading, and
/// `"Filter snippets"` input so frame shortcuts (`⌘F`, `⌘L`, `⌘⌫`, IME guards) operate against a
/// real `TextEdit`.
pub fn view(s: &MainState, theme: &Theme, filter: &LibraryFilter, pane_focused: bool) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let lib = s.library_snapshot();
    let heading = match filter {
        LibraryFilter::All => "All snippets".to_owned(),
        LibraryFilter::Favorites => "Favorites".to_owned(),
        LibraryFilter::Recent => "Recently used".to_owned(),
        LibraryFilter::Disabled => "Turned off".to_owned(),
        LibraryFilter::Group(gid) => lib
            .groups
            .iter()
            .find(|g| g.id == *gid)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Group".to_owned()),
    };

    let filter_box = Input::new(&s.library.filter)
        .leading_icon(&icons::SEARCH)
        .icon_at(10.0)
        .text_style(ty.body)
        .with(|t| {
            t.placeholder("Filter snippets")
                .on_edit(|st: &mut MainState| &mut st.library.filter)
        })
        .build(theme)
        .key(FILTER_KEY);

    let mut pane = col()
        .key(LIST_KEY)
        .role("region")
        .label("Snippets")
        .focusable(true)
        .w(300.0)
        .min_w(260.0)
        .max_w(320.0)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], c.hairline)
        .padding(12.0)
        .gap(10.0)
        .child(
            text(heading)
                .role("heading")
                .font(ty.headline)
                .color(c.foreground),
        )
        .child(filter_box)
        .child(
            text(format!("{} snippets", lib.snippets.len()))
                .font(ty.meta_11)
                .color(c.muted_foreground),
        );

    if pane_focused {
        pane = pane.focus_selected(true);
    }
    pane
}
