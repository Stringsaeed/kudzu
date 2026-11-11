//! Sidebar screen stub. Owned and filled out by S42 (`crates/kudzu-ui/src/main/sidebar/**`).

use rune::{col, text, El};

use crate::shell::MainState;
use crate::theme::Theme;

/// Per-screen state owned by the sidebar module (S42).
#[derive(Clone, Debug, Default)]
pub struct State;

impl State {
    pub fn is_composing(&self) -> bool {
        false
    }

    pub fn is_editing_text(&self) -> bool {
        false
    }
}

/// Placeholder sidebar slot until S42 lands. Exposes the `"Library"` navigation landmark inside
/// the frame's `"Sidebar"` complementary region.
pub fn view(s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let snippet_count = s.library_snapshot().snippets.len();

    col()
        .key("sidebar-nav")
        .role("navigation")
        .label("Library")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .px(10.0)
        .py(8.0)
        .gap(4.0)
        .child(text("Snippets").font(ty.label).color(c.faint))
        .child(
            text(format!("All snippets ({snippet_count})"))
                .font(ty.control_sm)
                .color(c.sidebar_foreground),
        )
}
