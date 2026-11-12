//! Fixed navigation and expansion status footer.

use super::{actions, item::item, overlays};
use crate::main::state::MainView;
use crate::shell::{MainState, SettingsPane};
use crate::theme::{icons, Theme};
use rune::{col, icon, row, text, El};

pub(super) fn view(s: &MainState, theme: &Theme) -> El {
    let status = s.svc.expansion.status().get();
    let paused = status.paused;
    let (status_label, detail) = if paused {
        ("Paused", "Nothing expands until you resume.")
    } else if status.secure_input {
        (
            "Secure Input on",
            "A password field or app has Secure Input on, so macOS blocks expansion.",
        )
    } else if status.tap == kudzu_core::system::TapMode::Off
        || (!status.permissions.input_monitoring && !status.permissions.accessibility)
    {
        (
            "Needs permission",
            "Kudzu can't see your typing yet. Open Settings → Permissions.",
        )
    } else if !status.permissions.accessibility
        || status.tap == kudzu_core::system::TapMode::ListenOnly
    {
        (
            "Limited",
            "Grant Accessibility so Kudzu can type expansions reliably.",
        )
    } else {
        (
            "Expanding",
            "Kudzu is listening for your triggers in every app.",
        )
    };
    let detail = if status_label == "Secure Input on" {
        status
            .secure_input_owner
            .as_ref()
            .map(|owner| format!("{owner} has Secure Input on, so macOS blocks expansion."))
            .unwrap_or_else(|| detail.to_owned())
    } else {
        detail.to_owned()
    };
    let status_opens_settings = matches!(status_label, "Limited" | "Needs permission");
    let mut footer = col()
        .key("sidebar-footer")
        .w_full()
        .h(119.0)
        .min_h(119.0)
        .px(10.0)
        .pt(9.0)
        .pb(8.0)
        .gap(1.0)
        .child(item("Insights", MainView::Insights, theme))
        .child(item(
            "Settings",
            MainView::Settings(SettingsPane::General),
            theme,
        ))
        .child(
            row()
                .w_full()
                .h(28.0)
                .mt(16.0)
                .gap(4.0)
                .center_y()
                .child(
                    row()
                        .key("sidebar-status")
                        .grow(1.0)
                        .role("button")
                        .label(format!("Status: {status_label}. {detail}"))
                        .on_press(move |st: &mut MainState| {
                            if status_opens_settings {
                                st.navigate_to(MainView::Settings(SettingsPane::General));
                            }
                        })
                        .h(24.0)
                        .gap(8.0)
                        .center_y()
                        .child(
                            rune::el()
                                .size(6.0, 6.0)
                                .radius(3.0)
                                .fill(match status_label {
                                    "Expanding" => theme.c.success,
                                    "Needs permission" => theme.c.destructive,
                                    _ => theme.c.warning,
                                })
                                .a11y_hidden(),
                        )
                        .child(
                            text(status_label)
                                .font(theme.ty.control_sm)
                                .color(theme.c.sidebar_foreground),
                        ),
                )
                .child(
                    row()
                        .key("sidebar-pause")
                        .focusable(true)
                        .role("button")
                        .label(if paused {
                            "Resume expansion"
                        } else {
                            "Pause expansion"
                        })
                        .size(28.0, 28.0)
                        .center()
                        .on_press(actions::toggle_pause)
                        .child(
                            icon(if paused { &icons::PLAY } else { &icons::PAUSE }, 14.0)
                                .color(theme.c.sidebar_foreground)
                                .a11y_hidden(),
                        ),
                ),
        );
    if let Some(pill) = overlays::selection(s, theme, "sidebar-footer") {
        footer = footer.child(pill);
    }
    footer
}
