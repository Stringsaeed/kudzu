//! Settings screen stub. Owned and filled out by S61–S63 (`crates/kudzu-ui/src/main/settings/**`).

use rune::{col, text, El};

use crate::main::state::settings_pane_label;
use crate::shell::{MainState, SettingsPane};
use crate::theme::Theme;

/// Per-screen state owned by the settings module (S61–S63).
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

/// Placeholder settings screen until S61 lands.
pub fn view(_s: &MainState, theme: &Theme, pane: SettingsPane) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    col()
        .key("settings-view-slot")
        .grow(1.0)
        .w_full()
        .h_full()
        .min_h(0.0)
        .padding(24.0)
        .gap(8.0)
        .child(
            text(settings_pane_label(pane))
                .role("heading")
                .font(ty.headline)
                .color(c.foreground),
        )
        .child(text("Settings").font(ty.body).color(c.muted_foreground))
}
