//! Keyboard shortcuts cheat sheet (`⌘/` or `?`) modal stub. Owned and filled out by S43
//! (`crates/kudzu-ui/src/main/cheatsheet.rs`).

use rune::{text, El};

use crate::shell::MainState;
use crate::theme::Theme;
use crate::widgets::kit_ext::dialog::Dialog;

/// `⌘/` and `?`.
pub fn toggle(s: &mut MainState) {
    s.frame.shortcuts_open = !s.frame.shortcuts_open;
}

/// Placeholder shortcuts cheat sheet modal until S43 lands.
pub fn view(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) {
    Dialog::new("shortcuts-cheatsheet", "Keyboard shortcuts")
        .kudzu()
        .open(s.frame.shortcuts_open)
        .description("Every shortcut in Kudzu.")
        .width(720.0)
        .on_close(|st: &mut MainState| st.frame.shortcuts_open = false)
        .child(
            text("Keyboard shortcuts")
                .font(theme.ty.body)
                .color(theme.c.muted_foreground),
        )
        .build(theme, overlays);
}
