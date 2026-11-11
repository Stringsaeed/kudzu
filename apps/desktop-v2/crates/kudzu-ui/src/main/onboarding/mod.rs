//! Onboarding screen stub. Owned and filled out by S65 (`crates/kudzu-ui/src/main/onboarding/**`).

use rune::{col, text, El};

use crate::shell::MainState;
use crate::theme::Theme;

/// Per-screen state owned by the first-run Onboarding module (S65).
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

/// Placeholder first-run Onboarding view until S65 lands. Renders without the sidebar or main
/// toolbar, matching `main-app.tsx` (`!settings.onboardingCompleted`).
pub fn view(_s: &MainState, theme: &Theme) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    col()
        .key("onboarding-view-slot")
        .w_full()
        .h_full()
        .center()
        .fill(c.background)
        .window_drag()
        .child(
            col()
                .key("onboarding-card")
                .role("region")
                .label("Setup: Welcome")
                .max_w(480.0)
                .w_full()
                .padding(24.0)
                .gap(12.0)
                .center()
                .no_window_drag()
                .child(
                    text("For things worth repeating")
                        .role("heading")
                        .font(ty.headline_large)
                        .color(c.foreground),
                )
                .child(
                    text("Set up Kudzu to expand your snippets anywhere.")
                        .font(ty.body)
                        .color(c.muted_foreground),
                ),
        )
}
