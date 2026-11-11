//! Command menu (`⌘K`) modal stub. Owned and filled out by S43 (`crates/kudzu-ui/src/main/command_menu.rs`).

use rune::{text, El};

use crate::shell::MainState;
use crate::theme::Theme;
use crate::widgets::kit_ext::dialog::Dialog;

/// `⌘K` only opens the menu (`main-app.tsx:110`). Closing is the open menu's own ⌘K and
/// Escape, which `keys::handle_key` routes before any action.
pub fn open(s: &mut MainState) {
    s.frame.command_open = true;
}

/// Placeholder command menu modal until S43 lands.
pub fn view(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) {
    Dialog::new("command-menu", "Search commands and snippets")
        .kudzu()
        .open(s.frame.command_open)
        .description("Type a command or snippet trigger.")
        .width(560.0)
        .on_close(|st: &mut MainState| st.frame.command_open = false)
        .child(
            text("Command menu")
                .font(theme.ty.body)
                .color(theme.c.muted_foreground),
        )
        .build(theme, overlays);
}
