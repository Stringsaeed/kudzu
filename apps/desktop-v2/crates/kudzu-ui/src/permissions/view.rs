//! Rune view for the `420 × 660` permission companion (`S66`).

use std::path::Path;
use std::sync::LazyLock;

use kudzu_app::ports::PermissionKind;
use rune::{col, el, icon, row, text, Color, Cx, DragItem, El, Pixels, TextStyle};

use crate::actions::permissions as actions;
use crate::permissions::model::PrimaryAction;
use crate::permissions::overlays::{help_sheet, status_region};
use crate::shell::windows::PermissionsState;
use crate::theme::{icons, motion, radius, Theme};
use crate::widgets::kit_ext::button::Button;

pub const ICON_KEY: &str = "app-icon";

static ICON_PIXELS: LazyLock<Pixels> = LazyLock::new(|| {
    let image = image::load_from_memory(include_bytes!("../../../../design/icon.png"))
        .expect("committed app icon")
        .to_rgba8();
    let pixels = Pixels::new();
    pixels.set(image.width(), image.height(), image.into_raw());
    pixels
});

/// Builds a native `.app` file drag item only when running from an application bundle.
pub fn drag_item(bundle: Option<&Path>) -> Option<DragItem> {
    let path = bundle?;
    if !kudzu_appkit::app_bundle::is_app_bundle_path(path) {
        return None;
    }
    Some(DragItem::new().file(path))
}

pub fn render(s: &PermissionsState, cx: &mut Cx<PermissionsState>) -> El {
    let theme = s.model.theme.theme(cx.system_dark());
    let c = &theme.c;

    let mut backdrop = el()
        .key("region-card")
        .role("region")
        .label("Kudzu permissions")
        .radius(radius::XL2)
        .fill(c.popover);
    let mut content = col().key("content");

    if s.model.scene_frame {
        backdrop = backdrop
            .pin(Some(16.0), Some(16.0), None, None)
            .size(388.0, 660.0)
            .border(1.0, c.hairline);
        for sh in &theme.depth.raised {
            backdrop = backdrop.box_shadow(*sh);
        }
        content = content
            .pin(Some(17.0), Some(17.0), None, None)
            .size(386.0, 658.0);
    } else {
        backdrop = backdrop.pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
        content = content.pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0));
    }

    content = content.child(header(s, theme)).child(body(s, theme));

    let mut root = el().key("permissions-root").w_full().h_full();
    if s.model.scene_frame {
        root = root.fill(c.background);
    }
    root.child(backdrop).child(content)
}

fn header(s: &PermissionsState, theme: &Theme) -> El {
    let c = &theme.c;
    let title = format!("Permissions for {}", s.model.app_name());
    row()
        .key("header")
        .role("banner")
        .label(title.clone())
        .w_full()
        .h(48.0)
        .px(20.0)
        .gap(8.0)
        .center_y()
        .child(icon(&icons::SHIELD_CHECK, 14.0).color(c.muted_foreground))
        .child(
            row().grow(1.0).h_full().center_y().window_drag().child(
                text(title)
                    .font(TextStyle::new(12.0, 400.0).lh(16.0))
                    .color(c.muted_foreground),
            ),
        )
        .child(
            row()
                .key("close")
                .role("button")
                .label("Close permission setup")
                .focusable(true)
                .size(24.0, 24.0)
                .center()
                .radius(999.0)
                .hover_group()
                .hover(crate::theme::kit::hover(theme))
                .press_scale(motion::press::SCALE)
                .focus_ring(3.0, 0.0, c.ring.alpha(0.5))
                .on_press(actions::dismiss)
                .child(
                    icon(&icons::X, 14.0)
                        .color(c.muted_foreground)
                        .group_color(c.foreground),
                ),
        )
}

fn body(s: &PermissionsState, theme: &Theme) -> El {
    let c = &theme.c;
    let mut main = col()
        .key("body")
        .w_full()
        .grow(1.0)
        .px(24.0)
        .pb(20.0)
        .child(
            el().key("iris-glow")
                .pin(Some(0.0), Some(0.0), Some(0.0), None)
                .h(176.0)
                .gradient(c.iris_soft, Color::TRANSPARENT),
        )
        .child(hero(s, theme))
        .child(permission_group(s, theme))
        .child(status_region(s, theme))
        .child(el().grow(1.0))
        .child(footer_actions(s, theme))
        .child(
            row()
                .key("privacy-footer")
                .mt(12.0)
                .w_full()
                .center()
                .child(
                    text("Your typing is never recorded or sent anywhere.")
                        .font(theme.ty.meta_10)
                        .color(c.faint),
                ),
        );

    if s.model.help_open {
        main = main.child(help_sheet(s, theme));
    }
    main
}

fn hero(s: &PermissionsState, theme: &Theme) -> El {
    let c = &theme.c;
    let app_name = s.model.app_name();
    let headline = s.model.headline();
    col()
        .key("hero")
        .w_full()
        .pt(12.0)
        .items_center()
        .child(
            el().key(ICON_KEY)
                .role("button")
                .label(format!(
                    "Show {app_name} in Finder to add it to System Settings"
                ))
                .help(if s.model.drag_available || s.model.scene_frame {
                    "Drag this app into System Settings"
                } else {
                    s.model.drag_hint()
                })
                .focusable(true)
                .size(80.0, 80.0)
                .radius(radius::APP_ICON)
                .pixels(&ICON_PIXELS)
                .focus_ring(2.0, 4.0, c.ring)
                .on_press(actions::press_app_icon)
                .drag_out(|state: &PermissionsState| drag_item(state.model.bundle_path.as_deref()))
                .on_drag_start(|state: &mut PermissionsState, _| actions::on_icon_drag_start(state))
                .on_drag_end(|state: &mut PermissionsState, _, _| actions::on_icon_drag_end(state))
                .on_drawn_layout_tx(|state: &mut PermissionsState, rect, tx| {
                    state.guide.record_icon_rect(rect);
                    tx.live_only();
                }),
        )
        .child(if s.model.drag_available || s.model.scene_frame {
            row()
                .mt(8.0)
                .h(16.0)
                .gap(4.0)
                .center_y()
                .child(icon(&icons::MOUSE_POINTER_2, 12.0).color(c.muted_foreground))
                .child(
                    text(s.model.drag_hint())
                        .key("drag-hint")
                        .font(TextStyle::new(10.0, 400.0).lh(16.0))
                        .color(c.muted_foreground),
                )
        } else {
            col().mt(8.0).max_w(310.0).items_center().child(
                text(s.model.drag_hint())
                    .key("drag-hint")
                    .font(TextStyle::new(10.0, 400.0).lh(15.0))
                    .color(c.muted_foreground)
                    .center()
                    .wrap(),
            )
        })
        .child(
            text(headline)
                .key("headline")
                .role("heading")
                .label(headline)
                .mt(20.0)
                .font(theme.ty.headline)
                .color(c.popover_foreground),
        )
        .child(
            col()
                .key("detail")
                .mt(8.0)
                .max_w(310.0)
                .items_center()
                .child(
                    text(s.model.detail())
                        .font(TextStyle::new(13.0, 400.0).lh(13.0 * 1.625))
                        .color(c.muted_foreground)
                        .wrap(),
                ),
        )
}

fn permission_group(s: &PermissionsState, theme: &Theme) -> El {
    col()
        .key("permission-cards")
        .mt(24.0)
        .w_full()
        .gap(8.0)
        .child(permission_card(s, theme, PermissionKind::Accessibility))
        .child(permission_card(s, theme, PermissionKind::InputMonitoring))
}

fn permission_card(s: &PermissionsState, theme: &Theme, kind: PermissionKind) -> El {
    let c = &theme.c;
    let selected = s.model.snap.selected == kind;
    let granted = s.model.card_granted(kind);
    let title = s.model.permission_name(kind);
    let desc = crate::permissions::model::PermissionsModel::permission_description(kind);
    let badge = if granted {
        "Allowed"
    } else if kind == PermissionKind::Accessibility {
        "Required"
    } else {
        "Optional"
    };
    let key = match kind {
        PermissionKind::Accessibility => "card-accessibility",
        PermissionKind::InputMonitoring => "card-input-monitoring",
    };
    let glyph = match kind {
        PermissionKind::Accessibility => &icons::SHIELD_CHECK,
        PermissionKind::InputMonitoring => &icons::KEYBOARD,
    };

    let mut card = row()
        .key(key)
        .role("button")
        .label(title)
        .checked(selected)
        .focusable(true)
        .w_full()
        .gap(12.0)
        .px(15.0)
        .py(13.0)
        .radius(12.0)
        .center_y()
        .focus_ring(3.0, 0.0, c.ring.alpha(0.5))
        .on_press(move |state: &mut PermissionsState| actions::select_kind(state, kind));

    if selected {
        card = card
            .fill(c.iris_soft.alpha(c.iris_soft.a * 0.5))
            .border(1.0, c.iris.alpha(0.4));
        for sh in &theme.depth.card {
            card = card.box_shadow(*sh);
        }
    } else {
        card = card
            .fill(c.surface.alpha(0.5))
            .hover(c.surface)
            .border(1.0, c.hairline);
    }

    let media = row()
        .size(32.0, 32.0)
        .radius(radius::LG)
        .center()
        .fill(if granted {
            c.success.alpha(0.10)
        } else {
            c.foreground.alpha(0.05)
        })
        .child(icon(glyph, 16.0).color(if granted {
            c.success
        } else {
            c.muted_foreground
        }));

    let copy = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(2.0)
        .child(
            text(title)
                .font(TextStyle::new(13.0, 500.0).lh(18.0))
                .color(c.popover_foreground),
        )
        .child(
            text(desc)
                .font(TextStyle::new(11.0, 400.0).lh(16.5))
                .color(c.muted_foreground),
        );

    let mut status = row().gap(4.0).center_y();
    if granted {
        status = status.child(icon(&icons::CHECK, 14.0).color(c.success));
    }
    status = status.child(
        text(badge)
            .font(TextStyle::new(10.0, 500.0).lh(14.0))
            .color(if granted {
                c.success
            } else {
                c.muted_foreground
            }),
    );

    card.child(media).child(copy).child(status)
}

fn footer_actions(s: &PermissionsState, theme: &Theme) -> El {
    let c = &theme.c;
    let cta = match s.model.primary_action() {
        PrimaryAction::Done => Button::new("Back to Kudzu")
            .icon_end(&icons::CHECK)
            .on_press(actions::close)
            .build(theme)
            .h(40.0)
            .w_full(),
        PrimaryAction::Restart { label } => Button::new(label)
            .on_press(actions::restart_app)
            .build(theme)
            .h(40.0)
            .w_full(),
        PrimaryAction::SelectAccessibility => Button::new("Set up app control")
            .icon_end(&icons::ARROW_UP_RIGHT)
            .on_press(|state: &mut PermissionsState| {
                actions::select_kind(state, PermissionKind::Accessibility);
            })
            .build(theme)
            .h(40.0)
            .w_full(),
        PrimaryAction::Opening => Button::new("Opening Settings…")
            .icon_end(&icons::LOADER_2)
            .disabled(true)
            .build(theme)
            .h(40.0)
            .w_full(),
        PrimaryAction::OpenSettings { enabled } => Button::new("Open System Settings")
            .icon_end(&icons::ARROW_UP_RIGHT)
            .disabled(!enabled)
            .on_press(actions::begin_selected)
            .build(theme)
            .h(40.0)
            .w_full(),
    };

    let help_label = if s.model.help_open {
        "Hide help"
    } else {
        "Don’t see the app, or already allowed?"
    };

    col()
        .key("footer-actions")
        .w_full()
        .pt(8.0)
        .gap(8.0)
        .child(cta.key("primary-cta"))
        .child(
            row()
                .key("help-toggle")
                .role("button")
                .label(help_label)
                .expanded(s.model.help_open)
                .focusable(true)
                .w_full()
                .py(6.0)
                .gap(6.0)
                .center()
                .radius(radius::MD)
                .hover_group()
                .press_scale(0.98)
                .focus_ring(2.0, 0.0, c.ring)
                .on_press(actions::toggle_help)
                .child(
                    icon(&icons::CIRCLE_HELP, 14.0)
                        .color(c.muted_foreground)
                        .group_color(c.foreground),
                )
                .child(
                    text(help_label)
                        .font(theme.ty.meta_11)
                        .color(c.muted_foreground)
                        .group_color(c.foreground),
                ),
        )
}
