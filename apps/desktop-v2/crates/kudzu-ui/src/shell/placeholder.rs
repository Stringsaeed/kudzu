//! Placeholder content for the picker window until S60 replaces it.
//! Shows the window's name, its state and the theme it resolved, so headless trees can check
//! routing and theme without the real screen.
use super::theme_sync::ThemeSync;
use super::windows::PickerState;
use crate::theme::Theme;
use rune::{col, text, Cx, El};

fn frame(key: &str, theme: &Theme, title: &str, detail: String, mode: &ThemeSync) -> El {
    col()
        .key(key)
        .w_full()
        .h_full()
        .padding(32.0)
        .gap(8.0)
        .fill(theme.c.background)
        .child(
            text(title)
                .key(format!("{key}-title"))
                .font(theme.ty.label)
                .color(theme.c.foreground),
        )
        .child(
            text(detail)
                .key(format!("{key}-detail"))
                .color(theme.c.muted_foreground),
        )
        .child(
            text(format!("{:?}", mode.appearance()))
                .key(format!("{key}-theme"))
                .color(theme.c.muted_foreground),
        )
}

pub(super) fn picker(s: &PickerState, cx: &mut Cx<PickerState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    let detail = if s.model.open { "Open" } else { "Hidden" };
    frame(
        "picker",
        theme,
        "Clipboard History",
        detail.into(),
        &s.model.theme,
    )
}
