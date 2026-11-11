//! Placeholder content, one view per window, until the screen steps replace them (S41, S56,
//! S57, S60, S66). Each shows the window's name, its state and the theme it resolved, so
//! headless trees can check routing and theme without the real screens.
use super::theme_sync::ThemeSync;
use super::windows::{FormState, MainState, PaletteState, PermissionsState, PickerState};
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

pub(super) fn main(s: &MainState, cx: &mut Cx<MainState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    let detail = match &s.model.route {
        super::Route::NewSnippet(new) if new.content.is_empty() => "New snippet".to_owned(),
        super::Route::NewSnippet(_) => "New snippet from the selection".to_owned(),
        route => route.label().to_owned(),
    };
    frame(
        "main",
        theme,
        s.shell.identity().app_name,
        detail,
        &s.model.theme,
    )
}

pub(super) fn palette(s: &PaletteState, cx: &mut Cx<PaletteState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    let detail = if s.model.open { "Open" } else { "Hidden" };
    frame(
        "palette",
        theme,
        "Quick Search",
        detail.into(),
        &s.model.theme,
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

pub(super) fn form(s: &FormState, cx: &mut Cx<FormState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    // The snippet's name only: field values are user content.
    let detail = s
        .model
        .request
        .as_ref()
        .map_or_else(|| "No form".to_owned(), |r| r.snippet_name.clone());
    frame("form", theme, "Fill In", detail, &s.model.theme)
}

pub(super) fn permissions(s: &PermissionsState, cx: &mut Cx<PermissionsState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    let detail = if s.model.open { "Open" } else { "Hidden" };
    frame(
        "permissions",
        theme,
        "Permissions",
        detail.into(),
        &s.model.theme,
    )
}
