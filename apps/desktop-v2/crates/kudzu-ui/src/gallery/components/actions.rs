//! `button` and `toggle`: `widgets::kit_ext::button` (decorated rune-kit `Button`).

use rune::{col, El};
use rune_kit::State;

use super::line;
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

/// Every variant, size and state the Tauri app shows, plus reference landmarks rebuilt as they
/// are in the app (same label, icon, variant and size).
pub fn button(theme: &Theme) -> El {
    let variants = [
        ("default", Variant::Default),
        ("outline", Variant::Outline),
        ("secondary", Variant::Secondary),
        ("ghost", Variant::Ghost),
        ("destructive", Variant::Destructive),
    ];
    let by_variant = variants.map(|(name, v)| {
        Button::new(name)
            .variant(v)
            .size(Size::Sm)
            .build(theme)
            .key(format!("spec:button/{name}/sm:_x28"))
    });
    let sizes = [
        ("xs", Size::Xs, 24.0),
        ("sm", Size::Sm, 28.0),
        ("default", Size::Default, 32.0),
        ("lg", Size::Lg, 36.0),
    ];
    let by_size = sizes.map(|(name, s, h)| {
        Button::new(format!("Size {name}"))
            .variant(Variant::Outline)
            .size(s)
            .icon(&icons::PLUS)
            .build(theme)
            .key(format!("spec:button/outline/{name}:_x{h}"))
    });
    let squares = [
        ("icon-xs", Size::IconXs, 24.0),
        ("icon-sm", Size::IconSm, 28.0),
        ("icon", Size::Icon, 32.0),
    ];
    let by_square = squares.map(|(name, s, h)| {
        Button::new(format!("Square {name}"))
            .variant(Variant::Ghost)
            .size(s)
            .icon(&icons::MORE_HORIZONTAL)
            .build(theme)
            .key(format!("spec:button/ghost/{name}:{h}x{h}"))
    });
    let states = [
        ("rest", State::Rest),
        ("hover", State::Hover),
        ("pressed", State::Pressed),
        ("focus", State::Focus),
        ("disabled", State::Disabled),
    ];
    let by_state = states.map(|(name, st)| {
        Button::new(name)
            .variant(Variant::Outline)
            .size(Size::Sm)
            .state(st)
            .build(theme)
            .key(format!("spec:button/outline/sm/{name}:_x28"))
    });
    let reference = [
        Button::new("Skip for now")
            .variant(Variant::Ghost)
            .build(theme)
            .key("parity:main:onboarding-access:button:1:Skip for now"),
        Button::new("Continue")
            .icon_end(&icons::ARROW_RIGHT)
            .build(theme)
            .key("parity:main:onboarding-access:button:1:Continue"),
        Button::new("Choose file…")
            .name("Choose file… for Raycast")
            .variant(Variant::Secondary)
            .size(Size::Sm)
            .build(theme)
            .key("parity:main:settings-data:button:1:Choose file… for Raycast"),
        Button::new("Sync now")
            .variant(Variant::Ghost)
            .size(Size::Sm)
            .icon(&icons::REFRESH_CW)
            .build(theme)
            .key("parity:main:settings-general:button:1:Sync now"),
        Button::new("Clear")
            .variant(Variant::Outline)
            .size(Size::Sm)
            .build(theme)
            .key("parity:main:settings-activity:button:1:Clear"),
        Button::new("Copy")
            .size(Size::Sm)
            .icon(&icons::COPY)
            .build(theme)
            .key("spec:button/default/sm/icon:_x28"),
        Button::new("More actions")
            .variant(Variant::Ghost)
            .size(Size::IconSm)
            .icon(&icons::MORE_HORIZONTAL)
            .build(theme)
            .key("parity:main:library:button:1:More actions"),
        Button::new("Hide sidebar")
            .variant(Variant::Ghost)
            .size(Size::Icon)
            .icon(&icons::PANEL_LEFT)
            .expanded(true)
            .build(theme)
            .key("parity:main:library:button:1:Hide sidebar"),
    ];
    col()
        .gap(24.0)
        .child(line(theme, "Variants, sm", by_variant))
        .child(line(theme, "Sizes, outline with an icon", by_size))
        .child(line(theme, "Icon sizes, ghost", by_square))
        .child(line(theme, "States, outline sm", by_state))
        .child(line(theme, "Reference landmarks", reference))
}

/// shadcn's `Toggle` (the formatting toolbar): a ghost square that, when on, takes
/// `bg-surface-raised shadow-soft`. S07's `widgets::toggle_button` composes it the same way.
pub fn toggle(theme: &Theme) -> El {
    let square = |name: &str, glyph, on: bool| {
        let b = Button::new(name)
            .variant(Variant::Ghost)
            .size(Size::IconSm)
            .icon(glyph)
            .build(theme);
        let b = if on {
            theme
                .depth
                .soft
                .iter()
                .fold(b.fill(theme.c.surface_raised), |b, s| b.box_shadow(*s))
        } else {
            b
        };
        b.key(format!("spec:toggle/sm/{name}:28x28"))
    };
    col().child(line(
        theme,
        "Toggle sm: off, on",
        [
            square("Bold", &icons::BOLD, false),
            square("Italic", &icons::ITALIC, true),
            square("Underline", &icons::UNDERLINE, false),
        ],
    ))
}
