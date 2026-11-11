//! Status banners and help sheet popover for the permission companion (`S66`).

use rune::{col, el, icon, rich, row, text, El, Span, TextStyle};

use crate::actions::permissions as actions;
use crate::permissions::model::StatusBanner;
use crate::shell::windows::PermissionsState;
use crate::theme::{icons, kit::destructive_soft, radius, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

pub(super) fn status_region(s: &PermissionsState, theme: &Theme) -> El {
    let c = &theme.c;
    let body = match s.model.status_banner() {
        StatusBanner::Error(msg) => col()
            .key("status-error")
            .role("alert")
            .w_full()
            .padding(12.0)
            .radius(radius::LG)
            .fill(destructive_soft(theme))
            .items_center()
            .child(
                text(msg)
                    .font(TextStyle::new(12.0, 400.0).lh(12.0 * 1.625))
                    .color(c.destructive)
                    .wrap(),
            ),
        StatusBanner::Ready { subtitle } => col()
            .key("status-ready")
            .w_full()
            .py(8.0)
            .gap(8.0)
            .items_center()
            .child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(icon(&icons::CHECK, 16.0).color(c.success))
                    .child(
                        text("Access confirmed")
                            .font(TextStyle::new(12.0, 500.0).lh(16.0))
                            .color(c.success),
                    ),
            )
            .child(
                text(subtitle)
                    .font(TextStyle::new(11.0, 400.0).lh(16.5))
                    .color(c.muted_foreground)
                    .wrap(),
            ),
        StatusBanner::Granted { subtitle } => col()
            .key("status-granted")
            .w_full()
            .py(4.0)
            .gap(8.0)
            .items_center()
            .child(
                row()
                    .gap(6.0)
                    .center_y()
                    .child(icon(&icons::CHECK, 16.0).color(c.success))
                    .child(
                        text("Access confirmed")
                            .font(TextStyle::new(12.0, 500.0).lh(16.0))
                            .color(c.success),
                    ),
            )
            .child(
                text(subtitle)
                    .font(TextStyle::new(11.0, 400.0).lh(11.0 * 1.625))
                    .color(c.muted_foreground)
                    .wrap(),
            ),
        StatusBanner::Waiting {
            app_name,
            permission_name,
        } => {
            let line1 = format!("Turn on {app_name} in {permission_name}.");
            let app_start = "Turn on ".len();
            let app_end = app_start + app_name.len();
            col()
                .key("status-waiting")
                .w_full()
                .py(4.0)
                .gap(8.0)
                .items_center()
                .child(
                    row()
                        .gap(6.0)
                        .center_y()
                        .child(icon(&icons::LOADER_2, 14.0).color(c.popover_foreground))
                        .child(
                            text("Waiting for macOS")
                                .font(TextStyle::new(12.0, 500.0).lh(16.0))
                                .color(c.popover_foreground),
                        ),
                )
                .child(
                    col()
                        .items_center()
                        .child(
                            rich(
                                line1,
                                vec![Span::new(app_start, app_end)
                                    .weight(500.0)
                                    .color(c.foreground)],
                            )
                            .font(TextStyle::new(11.0, 400.0).lh(11.0 * 1.625))
                            .color(c.muted_foreground),
                        )
                        .child(
                            text("If it isn’t listed, drag the icon into the list.")
                                .font(TextStyle::new(11.0, 400.0).lh(11.0 * 1.625))
                                .color(c.muted_foreground),
                        ),
                )
        }
        StatusBanner::Idle(msg) => col()
            .key("status-idle")
            .w_full()
            .px(12.0)
            .py(8.0)
            .items_center()
            .child(
                text(msg)
                    .font(TextStyle::new(11.0, 400.0).lh(11.0 * 1.625))
                    .color(c.muted_foreground)
                    .wrap(),
            ),
    };

    col()
        .key("status-area")
        .mt(16.0)
        .w_full()
        .min_h(72.0)
        .items_center()
        .child(body)
}

pub(super) fn help_sheet(s: &PermissionsState, theme: &Theme) -> El {
    let c = &theme.c;
    let app_name = s.model.app_name();
    let perm_name = s.model.permission_name(s.model.snap.selected);
    let heading = format!("Add {app_name} to the list");

    let mut sheet = col()
        .key("help-sheet")
        .pin(Some(16.0), None, Some(16.0), Some(16.0))
        .padding(16.0)
        .radius(radius::XL)
        .fill(c.popover)
        .border(1.0, c.hairline);
    for sh in &theme.depth.raised {
        sheet = sheet.box_shadow(*sh);
    }

    let step = |num: &str, body: String| {
        row()
            .w_full()
            .gap(6.0)
            .child(
                text(num.to_owned())
                    .font(TextStyle::new(12.0, 400.0).lh(19.5))
                    .color(c.muted_foreground),
            )
            .child(
                el().grow(1.0).min_w(0.0).child(
                    text(body)
                        .font(TextStyle::new(12.0, 400.0).lh(19.5))
                        .color(c.muted_foreground)
                        .wrap(),
                ),
            )
    };

    sheet
        .child(
            row()
                .w_full()
                .center_y()
                .child(
                    el().grow(1.0).child(
                        text(heading.clone())
                            .role("heading")
                            .label(heading)
                            .font(theme.ty.body_strong)
                            .color(c.popover_foreground),
                    ),
                )
                .child(
                    Button::new("Close help")
                        .variant(Variant::Ghost)
                        .size(Size::IconXs)
                        .icon(&icons::X)
                        .on_press(actions::close_help)
                        .build(theme),
                ),
        )
        .child(
            col()
                .mt(8.0)
                .w_full()
                .pl(4.0)
                .gap(8.0)
                .child(step("1.", format!("Open Privacy & Security → {perm_name}.")))
                .child(step(
                    "2.",
                    format!(
                        "Drag the app icon into the list, or use the + button to select {app_name}."
                    ),
                ))
                .child(step(
                    "3.",
                    "Turn on its switch and approve the macOS prompt.".to_owned(),
                )),
        )
        .child(
            text(format!(
                "Already on? Quit and reopen {app_name} if macOS hasn’t applied the change. Make sure you enabled this copy of the app."
            ))
            .mt(12.0)
            .font(TextStyle::new(12.0, 400.0).lh(19.5))
            .color(c.muted_foreground)
            .wrap(),
        )
        .child(
            Button::new("Show this app in Finder")
                .variant(Variant::Outline)
                .size(Size::Sm)
                .icon(&icons::FOLDER_OPEN)
                .on_press(actions::reveal_app)
                .build(theme)
                .mt(12.0)
                .w_full(),
        )
        .child(
            Button::new(format!("Restart {app_name}"))
                .variant(Variant::Ghost)
                .size(Size::Sm)
                .on_press(actions::restart_app)
                .build(theme)
                .mt(4.0)
                .w_full(),
        )
}
