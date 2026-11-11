//! Insights screen stub. Owned and filled out by S64 (`crates/kudzu-ui/src/main/insights/**`).

use rune::{col, text, El};

use crate::shell::MainState;
use crate::theme::Theme;

/// Per-screen state owned by the Insights module (S64).
#[derive(Clone, Debug, Default)]
pub struct State {}

impl State {
    pub fn is_composing(&self) -> bool {
        false
    }

    pub fn is_editing_text(&self) -> bool {
        false
    }

    pub fn blur_inputs(&mut self) {}
}

/// Placeholder Insights screen until S64 lands.
pub fn view(_s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    col()
        .key("insights-view-slot")
        .grow(1.0)
        .w_full()
        .h_full()
        .min_h(0.0)
        .padding(24.0)
        .gap(8.0)
        .child(
            text("Insights")
                .role("heading")
                .font(ty.headline)
                .color(c.foreground),
        )
        .child(
            text("Usage statistics and time saved.")
                .font(ty.body)
                .color(c.muted_foreground),
        )
}
