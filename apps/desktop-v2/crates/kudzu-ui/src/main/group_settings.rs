//! Group settings dialog stub. Owned and filled out by S55 (`crates/kudzu-ui/src/main/group_settings.rs`).

use rune::{text, El};

use crate::shell::MainState;
use crate::theme::Theme;
use crate::widgets::kit_ext::dialog::Dialog;

/// Placeholder group settings dialog until S55 lands.
pub fn view(s: &MainState, theme: &Theme, overlays: &mut Vec<El>) {
    let Some(gid) = s.frame.group_settings_id else {
        return;
    };
    let lib = s.library_snapshot();
    let name = lib
        .groups
        .iter()
        .find(|g| g.id == gid)
        .map(|g| g.name.as_str())
        .unwrap_or("Group settings");
    Dialog::new("group-settings", name)
        .kudzu()
        .open(true)
        .description("Configure group expansion behavior and app rules.")
        .width(520.0)
        .on_close(|st: &mut MainState| st.frame.group_settings_id = None)
        .child(
            text("Group settings")
                .font(theme.ty.body)
                .color(theme.c.muted_foreground),
        )
        .build(theme, overlays);
}
