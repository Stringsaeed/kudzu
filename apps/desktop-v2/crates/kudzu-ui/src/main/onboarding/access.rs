//! The permission card reads engine status independently of the Continue readiness gate.
use crate::shell::MainState;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Variant};
use kudzu_app::permissions::ready;
use rune::{col, icon, row, text, El};

pub fn card(s: &MainState, t: &Theme) -> El {
    let status = s.svc.expansion.status().get();
    let allowed = ready(&status);
    let title = if allowed {
        "Permissions are set"
    } else {
        "Enable typing in other apps"
    };
    let subtitle = if !allowed {
        "A guided setup, with every step beside you."
    } else if status.can_expand {
        "Kudzu is ready to expand your snippets."
    } else {
        &status.summary
    };
    let header = row()
        .gap(12.0)
        .items_center()
        .child(
            col()
                .size(40.0, 40.0)
                .center()
                .radius(12.0)
                .fill(t.c.iris_soft)
                .child(icon(&icons::SHIELD_CHECK, 20.0).color(t.c.iris_foreground)),
        )
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(text(title).font(t.ty.body_strong).color(t.c.foreground))
                .child(
                    text(subtitle)
                        .wrap()
                        .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                        .color(t.c.muted_foreground),
                ),
        );
    let permissions = row()
        .gap(8.0)
        .items_center()
        .child(permission(
            "Accessibility",
            status.permissions.accessibility,
            &icons::SHIELD_CHECK,
            t,
        ))
        .child(text("·").color(t.c.faint))
        .child(permission(
            "Input Monitoring",
            status.permissions.input_monitoring,
            &icons::KEYBOARD,
            t,
        ));
    let action = Button::new(if s.onboarding.permission_opening {
        "Opening…"
    } else if allowed {
        "Review permissions"
    } else {
        "Set up permissions"
    })
    .variant(if allowed {
        Variant::Outline
    } else {
        Variant::Default
    })
    .icon_end(if s.onboarding.permission_opening {
        &icons::LOADER_2
    } else {
        &icons::ARROW_UP_RIGHT
    })
    .disabled(s.onboarding.permission_opening)
    .on_press(super::actions::open_permissions)
    .build(t)
    .key("onboarding-permissions")
    .w_full();
    let mut card = t.depth.card.iter().fold(
        col()
            .w_full()
            .padding(16.0)
            .gap(16.0)
            .radius(12.0)
            .fill(t.c.surface)
            .child(header)
            .child(permissions)
            .child(action),
        |el, shadow| el.box_shadow(*shadow),
    );
    if let Some(error) = &s.onboarding.permission_error {
        card = card.child(
            text(error)
                .wrap()
                .role("alert")
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                .color(t.c.destructive),
        );
    }
    card
}
fn permission(name: &str, granted: bool, missing: &'static rune::Icon, t: &Theme) -> El {
    row()
        .gap(8.0)
        .items_center()
        .child(
            icon(if granted { &icons::CHECK } else { missing }, 14.0).color(if granted {
                t.c.success
            } else {
                t.c.muted_foreground
            }),
        )
        .child(
            text(name)
                .font(rune::TextStyle::new(12.0, 400.0).lh(16.0))
                .color(t.c.muted_foreground),
        )
}
