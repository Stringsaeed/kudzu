//! Clipboard history screen stub. Owned and filled out by S58 (`crates/kudzu-ui/src/main/clipboard/**`).

use rune::{col, text, El, TextEdit, Tx};

use crate::main::state::ClipFilter;
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::input::Input;

pub const SEARCH_KEY: &str = "clipboard-search";
pub const LIST_KEY: &str = "clipboard-list-slot";

/// Per-screen state owned by the clipboard history module (S58).
#[derive(Clone, Debug, Default)]
pub struct State {
    pub search: TextEdit,
}

impl State {
    pub fn is_composing(&self) -> bool {
        self.search.is_composing()
    }

    pub fn is_editing_text(&self) -> bool {
        self.search.focused
    }

    pub fn focus_search(&mut self, tx: &mut Tx) {
        self.search.focused = true;
        tx.focus(SEARCH_KEY, true);
    }

    pub fn blur_inputs(&mut self) {
        self.search.focused = false;
    }
}

/// Placeholder clipboard history screen until S58 lands.
pub fn view(s: &MainState, theme: &Theme, filter: ClipFilter, pane_focused: bool) -> El {
    let c = &theme.c;
    let ty = &theme.ty;

    let search = Input::new(&s.clipboard.search)
        .leading_icon(&icons::SEARCH)
        .icon_at(10.0)
        .text_style(ty.body)
        .with(|t| {
            t.placeholder("Search clipboard history")
                .on_edit(|st: &mut MainState| &mut st.clipboard.search)
        })
        .build(theme)
        .key(SEARCH_KEY);

    let mut pane = col()
        .key(LIST_KEY)
        .role("region")
        .label("Clipboard history")
        .focusable(true)
        .grow(1.0)
        .w_full()
        .h_full()
        .min_h(0.0)
        .padding(24.0)
        .gap(12.0)
        .child(
            text(filter.label())
                .role("heading")
                .font(ty.headline)
                .color(c.foreground),
        )
        .child(search);

    if pane_focused {
        pane = pane.focus_selected(true);
    }
    pane
}
