//! Kudzu's tokens as a [`rune_kit::Kit`], so rune-kit components wear the Tauri app's theme.
//!
//! Each `Palette` role is the shadcn token the matching `components/ui` primitive paints with:
//! overlays (menus, popovers, dialogs, toasts) are `popover`, the contrast button is shadcn's
//! `default` (primary), the neutral button is `outline` (raised surface), muted and faint text
//! are `muted-foreground` (placeholders, menu shortcuts, select hints), hover is `muted`
//! (`muted/50` in dark, as `ghost` paints it). Elevation maps onto the depth tokens
//! (`raised`, `well`, `float`, `panel`, `knob`). Translucent tokens stay as authored: every
//! Kudzu window blends in sRGB, as the CSS was written (`blend_over: None`).
//!
//! What the Kit cannot carry (button heights per size, the switch's track, the textarea's type)
//! is in `widgets::kit_ext`, with its evidence in `docs/migration/PARITY_MATRIX.md`.

use std::sync::LazyLock;

use rune::{Color, Shadow, TextStyle};
use rune_kit::{Elevation, Glyphs, Kit, Motion, Palette, Radius, StateColors, Type};

use super::{icons, motion, palette, radius, Theme};

static LIGHT: LazyLock<Kit> = LazyLock::new(|| kit(Theme::light()));
static DARK: LazyLock<Kit> = LazyLock::new(|| kit(Theme::dark()));

impl Theme {
    /// This theme's rune-kit tokens, built once per appearance.
    pub fn kit(&self) -> &'static Kit {
        if self.dark {
            &DARK
        } else {
            &LIGHT
        }
    }
}

/// Maps the theme's tokens onto rune-kit's schema.
pub fn kit(theme: &Theme) -> Kit {
    Kit {
        dark: theme.dark,
        c: colors(theme),
        elevation: elevation(theme),
        glyph: GLYPHS,
        ty: TYPE,
        radius: RADIUS,
        motion: MOTION,
        blend_over: None,
        ring: None,
        invalid_text: false,
    }
}

/// shadcn's `hover:bg-muted`, `dark:hover:bg-muted/50` (button `ghost`, menu items).
pub fn hover(theme: &Theme) -> Color {
    if theme.dark {
        theme.c.muted.alpha(0.5)
    } else {
        theme.c.muted
    }
}

/// `bg-destructive/10`, `dark:bg-destructive/20` (destructive buttons and badges).
pub fn destructive_soft(theme: &Theme) -> Color {
    theme
        .c
        .destructive
        .alpha(if theme.dark { 0.2 } else { 0.1 })
}

fn state(fg: Color, theme: &Theme) -> StateColors {
    StateColors {
        bg: fg.alpha(if theme.dark { 0.2 } else { 0.1 }),
        fg,
        border: fg.alpha(0.4),
        icon: fg,
    }
}

fn colors(theme: &Theme) -> Palette {
    let c = &theme.c;
    let hover = hover(theme);
    let white = Color::WHITE;
    let black = Color::BLACK;
    Palette {
        page: c.background,
        bg_base: c.background,
        bg_deep: c.background,
        layer_01: c.popover,
        layer_02: c.muted,
        // `bg-foreground/[0.06]`: keycaps and subtle badges.
        layer_03: c.foreground.alpha(0.06),
        layer_04: c.input,
        bg_inverse: c.foreground,
        bg_contrast: c.primary,
        bg_button_neutral: c.surface_raised,
        bg_icon_button_contrast: c.primary,
        bg_accent: c.iris,
        // `sheen-strong` over primary; `hover:bg-primary/90` lifts it by a tenth of the page.
        submit_highlight: white.alpha(0.2),
        submit_hover: c.background.alpha(0.1),
        submit_fg: c.primary_foreground,
        submit_icon: c.primary_foreground,
        text_base: c.foreground,
        text_muted: c.muted_foreground,
        text_faint: c.muted_foreground,
        text_inverse: c.background,
        text_contrast: c.primary_foreground,
        text_accent: c.iris_foreground,
        icon_base: c.foreground,
        icon_muted: c.muted_foreground,
        icon_faint: c.faint,
        icon_inverse: c.background,
        icon_contrast: c.primary_foreground,
        icon_accent: c.iris,
        border_muted: c.hairline,
        border_base: c.border,
        border_strong: c.ring,
        border_focus: c.ring,
        hover,
        // shadcn sinks a pressed control (scale and `shadow-pressed`) rather than tinting it; an
        // open menu trigger holds `aria-expanded:bg-muted`, which rune-kit paints as `pressed`.
        pressed: c.muted,
        contrast_hover: c.background.alpha(0.1),
        contrast_pressed: c.background.alpha(0.1),
        // The dialog overlay: `bg-black/10`.
        scrim: black.alpha(0.1),
        depth_top: white,
        depth_bottom: white.alpha(0.0),
        white,
        black,
        // Switch thumb: `bg-background`, `dark:data-checked:bg-white`.
        knob: c.background,
        knob_on: if theme.dark { white } else { c.background },
        success: state(c.success, theme),
        warning: state(c.warning, theme),
        danger: state(c.destructive, theme),
        info: state(c.iris, theme),
        surface_base: c.muted,
        surface_weak: c.muted,
        surface_hover: c.muted,
        surface_raised: c.surface_raised,
        surface_raised_base: c.surface_raised,
        surface_raised_hover: c.muted,
        // Tooltip: `bg-foreground text-background`.
        surface_float: c.foreground,
        surface_disabled: c.input.alpha(0.5),
        border_weak: c.border,
        border_hover: c.ring,
        border_active: c.ring,
        border_disabled: c.border,
        text_strong: c.foreground,
        text_weak: c.muted_foreground,
        text_invert_strong: c.background,
        icon_weaker: c.faint,
        std_border: c.input,
        std_icon: c.foreground,
        std_icon_invert: c.background,
        std_icon_strong: c.iris,
        std_text: c.foreground,
    }
}

/// Tailwind's `shadow-md` under shadcn's `ring-1 ring-foreground/10` (popover, menus, select).
fn md(theme: &Theme) -> Vec<Shadow> {
    let drop = |y: f32, blur: f32, spread: f32| Shadow {
        y,
        blur,
        spread,
        color: Color::BLACK.alpha(0.1),
        ..Shadow::default()
    };
    vec![
        Shadow {
            spread: 1.0,
            color: theme.c.foreground.alpha(0.1),
            ..Shadow::default()
        },
        drop(4.0, 6.0, -1.0),
        drop(2.0, 4.0, -2.0),
    ]
}

fn elevation(theme: &Theme) -> Elevation {
    let d = &theme.depth;
    let checked = Shadow {
        y: 1.0,
        blur: 2.0,
        color: palette::Oklch::new(0.0, 0.0, 0.0).alpha(0.18).color(),
        inset: true,
        ..Shadow::default()
    };
    // `Elevation` has a private field, so it is filled in place rather than by struct update.
    let mut e = Elevation::default().with_input(d.well.clone());
    e.raised = d.card.clone();
    e.floating = md(theme);
    e.overlay = d.panel.clone();
    e.button_neutral = d.raised.clone();
    e.button_contrast = d.raised.clone();
    e.elements = d.knob.clone();
    e.switch_off = d.well.clone();
    e.switch_on = vec![checked];
    e.md = md(theme);
    e.knob = d.knob.clone();
    e
}

const fn lh(style: TextStyle, line: f32) -> TextStyle {
    TextStyle {
        line_height: line,
        ..style
    }
}

/// rune-kit's type roles in Kudzu's faces: 13/20 body, Tailwind's `text-sm` (14/20) where
/// rune-kit's "body" sits, `text-xs` (12/16) for its "xsmall".
const TYPE: Type = {
    let t = super::type_::TYPE;
    Type {
        ui: t.body,
        ui_medium: t.body_medium,
        ui_compact: lh(t.body, 16.0),
        small: lh(t.meta_11, 16.0),
        small_medium: lh(TextStyle::new(11.0, 500.0), 16.0),
        body: t.input,
        body_medium: t.control,
        text_small: t.body,
        text_small_medium: t.body_medium,
        text_xsmall: lh(TextStyle::new(12.0, 400.0), 16.0),
        text_xsmall_medium: t.control_xs,
        mono: t.mono,
    }
};

/// rune-kit reads `sm` for checkboxes and small icon buttons (Kudzu's 4 pt checkbox and
/// `rounded-sm` minis), `md` for regular controls (`rounded-md`, 8), `lg` and `xl` for larger
/// ones (`rounded-lg` 10, `rounded-xl` 14).
const RADIUS: Radius = Radius {
    xs: radius::MARK,
    sm: radius::CHECKBOX,
    md: radius::MD,
    lg: radius::LG,
    xl: radius::XL,
};

/// Colour changes on `--motion-fast`'s soft spring, overlays on the moderate pop, disclosure on
/// the slow soft spring.
const MOTION: Motion = Motion {
    control: motion::soft_spring(motion::MODERATE),
    overlay: motion::pop_spring(motion::MODERATE),
    disclosure: motion::SOFT,
};

/// rune-kit's component glyphs, drawn from Kudzu's Lucide set where it has the shape.
const GLYPHS: Glyphs = Glyphs {
    check: &icons::CHECK,
    check_small: &icons::CHECK,
    chevron_down: &icons::CHEVRON_DOWN,
    chevron_up: &icons::CHEVRON_UP,
    caret_right: &icons::CHEVRON_RIGHT,
    close: &icons::X,
    clear: &icons::X,
    copy: &icons::COPY,
    search: &icons::SEARCH,
    lock: &icons::LOCK,
    eye_off: &icons::EYE_OFF,
    ..rune_kit::glyphs::DEFAULT
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kit_is_built_once_per_appearance() {
        assert!(std::ptr::eq(Theme::dark().kit(), Theme::dark().kit()));
        assert!(Theme::dark().kit().dark);
        assert!(!Theme::light().kit().dark);
        assert!(Theme::light().kit().blend_over.is_none());
    }

    #[test]
    fn palette_roles_are_the_shadcn_tokens() {
        for theme in [Theme::light(), Theme::dark()] {
            let (k, c) = (theme.kit(), &theme.c);
            assert_eq!(k.c.page, c.background);
            assert_eq!(k.c.layer_01, c.popover, "overlays paint bg-popover");
            assert_eq!(k.c.bg_contrast, c.primary, "shadcn default button");
            assert_eq!(k.c.text_contrast, c.primary_foreground);
            assert_eq!(k.c.bg_button_neutral, c.surface_raised, "outline button");
            assert_eq!(k.c.text_faint, c.muted_foreground, "placeholders");
            assert_eq!(k.c.surface_float, c.foreground, "tooltip fill");
            assert_eq!(k.c.text_invert_strong, c.background, "tooltip ink");
            assert_eq!(k.c.danger.fg, c.destructive);
            assert_eq!(k.c.border_focus, c.ring);
        }
        assert_eq!(Theme::light().kit().c.hover, Theme::light().c.muted);
        assert_eq!(
            Theme::dark().kit().c.hover,
            Theme::dark().c.muted.alpha(0.5)
        );
        assert_eq!(Theme::dark().kit().c.knob_on, Color::WHITE);
    }

    #[test]
    fn elevation_is_the_depth_tokens() {
        for theme in [Theme::light(), Theme::dark()] {
            let (e, d) = (&theme.kit().elevation, &theme.depth);
            assert_eq!(e.button_neutral, d.raised);
            assert_eq!(e.input(), d.well.as_slice());
            assert_eq!(e.overlay, d.panel);
            assert_eq!(e.knob, d.knob);
            assert_eq!(e.md.len(), 3, "ring plus Tailwind's two shadow-md layers");
        }
    }

    #[test]
    fn type_and_radius_follow_the_tailwind_scale() {
        let k = Theme::light().kit();
        assert_eq!((k.ty.ui.size, k.ty.ui.line_height), (13.0, 20.0));
        assert_eq!((k.ty.body.size, k.ty.body.line_height), (14.0, 20.0));
        assert_eq!(
            k.ty.text_xsmall_medium,
            super::super::type_::TYPE.control_xs
        );
        assert_eq!(
            [k.radius.sm, k.radius.md, k.radius.lg, k.radius.xl],
            [4.0, 8.0, 10.0, 14.0]
        );
    }
}
