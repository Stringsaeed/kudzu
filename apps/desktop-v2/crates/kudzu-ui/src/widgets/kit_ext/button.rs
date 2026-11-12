//! shadcn's `Button` (`components/ui/button.tsx`) as rune-kit's [`rune_kit::Button`], decorated.
//!
//! Verdict in `PARITY_MATRIX.md`: **decorated**. rune-kit sizes push buttons as AppKit's
//! `NSButton` (16/20/24/28/36 points, its own padding and radii), and Kudzu's sizes are the
//! Tailwind ones (24/28/32/36, `px-2`/`px-2.5`, 8 or 10 point corners, 12 to 14 point labels).
//! The built element takes them as overrides: height, padding, radius, gap, the label (drawn
//! with `label_with` in Kudzu's type, the icon at Kudzu's size), the fill of the variants rune-kit
//! has no skin for, the sheen, the focus ring and the press scale. Upstream: Rune #573
//! (gap 17), "rune-kit `Button`: take height, padding and radius per size from `Kit`".
//! The depth is Kudzu's own (D6, added after S06): rune-kit's button shadows are left out (the
//! button is built with a kit whose button elevations are empty) and the `raised`,
//! `raised-hover` and `pressed` shadows, with the sheen, are layers inside the button that
//! follow hover and press ([`super::press`], `RUNE-GAP(#360)`).

use std::sync::LazyLock;

use rune::{icon, row, text, Color, El, Icon, TextStyle, Tx};
use rune_kit::{ButtonVariant, ControlSize, Kit, State};

use super::press::{raised_layers, Faces, Look, Press};

use crate::theme::kit::{destructive_soft, hover};
use crate::theme::{motion, palette::fixed, radius, Theme};

/// shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variant {
    /// Primary fill, strong sheen, raised.
    #[default]
    Default,
    /// The raised surface with a sheen.
    Outline,
    /// `bg-secondary` with a sheen.
    Secondary,
    /// No fill until hovered.
    Ghost,
    /// `bg-destructive/10` and destructive ink.
    Destructive,
    /// Primary ink, no fill.
    Link,
}

/// shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    Xs,
    Sm,
    #[default]
    Default,
    Lg,
    IconXs,
    IconSm,
    Icon,
    IconLg,
}

/// The metrics a size's classes give.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub height: f32,
    /// Horizontal padding, inside the 1 pt transparent border (`px-2.5` plus the border). The
    /// classes' tighter `has-data-[icon=inline-*]` padding never applies: no Kudzu icon sets
    /// `data-icon`, so an icon button keeps the plain padding (the reference measures it).
    pub px: f32,
    pub radius: f32,
    pub gap: f32,
    pub font: TextStyle,
    pub icon: f32,
    /// An icon-only square (`size-*`).
    pub square: bool,
}

impl Size {
    pub fn metrics(self) -> Metrics {
        let ty = crate::theme::type_::TYPE;
        // The 1 pt transparent border (`border border-transparent`) sits inside the padding box.
        let m = |height, px: f32, radius, gap, font, icon, square| Metrics {
            height,
            px: px + 1.0,
            radius,
            gap,
            font,
            icon,
            square,
        };
        match self {
            Size::Xs => m(
                24.0,
                8.0,
                radius::BUTTON_XS,
                4.0,
                ty.control_xs,
                12.0,
                false,
            ),
            Size::Sm => m(
                28.0,
                10.0,
                radius::BUTTON_SM,
                4.0,
                ty.control_sm,
                14.0,
                false,
            ),
            Size::Default => m(32.0, 10.0, radius::LG, 6.0, ty.control, 16.0, false),
            Size::Lg => m(36.0, 10.0, radius::LG, 6.0, ty.control, 16.0, false),
            Size::IconXs => m(24.0, 0.0, radius::BUTTON_XS, 0.0, ty.control, 12.0, true),
            Size::IconSm => m(28.0, 0.0, radius::BUTTON_SM, 0.0, ty.control, 16.0, true),
            Size::Icon => m(32.0, 0.0, radius::LG, 0.0, ty.control, 16.0, true),
            Size::IconLg => m(36.0, 0.0, radius::LG, 0.0, ty.control, 16.0, true),
        }
    }
}

type Attach = Box<dyn FnOnce(rune_kit::Button) -> rune_kit::Button>;
/// Space's press look, attached once the state type is known (`on_press`).
type Keys = Box<dyn FnOnce(El, &Press) -> El>;

/// The theme's kit without button shadows: the depth layers draw them.
fn flat(theme: &Theme) -> &'static Kit {
    static LIGHT: LazyLock<Kit> = LazyLock::new(|| flatten(Theme::light()));
    static DARK: LazyLock<Kit> = LazyLock::new(|| flatten(Theme::dark()));
    if theme.dark {
        &DARK
    } else {
        &LIGHT
    }
}

fn flatten(theme: &Theme) -> Kit {
    let mut k = theme.kit().clone();
    let input = k.elevation.input().to_vec();
    k.elevation = k.elevation.with_input(input);
    k.elevation.button_neutral.clear();
    k.elevation.button_contrast.clear();
    k
}

/// `Button::new("Save").variant(Variant::Outline).size(Size::Sm).build(theme)`. An icon-only size
/// uses the label as the accessible name.
pub struct Button {
    label: String,
    name: Option<String>,
    variant: Variant,
    size: Size,
    icon: Option<&'static Icon>,
    icon_end: Option<&'static Icon>,
    /// The label and icon colour, instead of the variant's (`text-muted-foreground`).
    ink: Option<Color>,
    state: State,
    attach: Vec<Attach>,
    keys: Option<Keys>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Button {
        Button {
            label: label.into(),
            name: None,
            variant: Variant::Default,
            size: Size::Default,
            icon: None,
            icon_end: None,
            ink: None,
            state: State::Rest,
            attach: Vec::new(),
            keys: None,
        }
    }
    /// Draws the label and icon in `c` (a ghost button with `text-muted-foreground`).
    pub fn ink(mut self, c: Color) -> Self {
        self.ink = Some(c);
        self
    }
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    /// The leading icon, or the whole face of an icon size.
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.icon = Some(i);
        self
    }
    /// An icon after the label (`data-icon="inline-end"`, `Continue →`).
    pub fn icon_end(mut self, i: &'static Icon) -> Self {
        self.icon_end = Some(i);
        self
    }
    /// The accessible name when it says more than the label (`aria-label`).
    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.name = Some(n.into());
        self
    }
    /// A forced visual state (galleries); at rest the runtime drives hover and press.
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn disabled(self, d: bool) -> Self {
        if d {
            self.state(State::Disabled)
        } else {
            self
        }
    }
    /// The trigger of a menu or popover (`aria-expanded`).
    pub fn expanded(mut self, open: bool) -> Self {
        self.attach.push(Box::new(move |b| b.expanded(open)));
        self
    }
    pub fn help(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        self.attach.push(Box::new(move |b| b.help(text)));
        self
    }
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        let s = s.into();
        self.attach.push(Box::new(move |b| b.shortcut(s)));
        self
    }
    pub fn on_press<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.attach.push(Box::new(move |b| b.on_press(f)));
        self.keys = Some(Box::new(|e, p: &Press| p.keyboard::<S>(e)));
        self
    }
    pub fn on_press_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.attach.push(Box::new(move |b| b.on_press_tx(f)));
        self.keys = Some(Box::new(|e, p: &Press| p.keyboard::<S>(e)));
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        let c = &theme.c;
        let m = self.size.metrics();
        // rune-kit's skin for the fill, shadows, hover overlay and focus; Kudzu's fill and ink.
        let (base, fill, ink) = match self.variant {
            Variant::Default => (ButtonVariant::Contrast, None, c.primary_foreground),
            Variant::Outline => (ButtonVariant::Neutral, None, c.foreground),
            Variant::Secondary => (
                ButtonVariant::Neutral,
                Some(c.secondary),
                c.secondary_foreground,
            ),
            Variant::Ghost => (ButtonVariant::Ghost, None, c.foreground),
            Variant::Destructive => (
                ButtonVariant::Ghost,
                Some(destructive_soft(theme)),
                c.destructive,
            ),
            Variant::Link => (ButtonVariant::Ghost, None, c.primary),
        };
        let ink = self.ink.unwrap_or(ink);
        let (label, glyph, end) = (self.label.clone(), self.icon, self.icon_end);
        let face = move |_: TextStyle, _: Color| {
            let mut face = row().gap(m.gap).center_y();
            if let Some(i) = glyph {
                face = face.child(icon(i, m.icon).key("icon").color(ink));
            }
            if !m.square {
                face = face.child(text(label).key("label").font(m.font).color(ink));
            }
            if let Some(i) = end {
                face = face.child(icon(i, m.icon).key("icon-end").color(ink));
            }
            face
        };
        let name = self.name.unwrap_or(self.label);
        let press = Press::new(format!("button:{name}"));
        let mut b = rune_kit::Button::new(name)
            .variant(base)
            .control_size(ControlSize::Regular)
            .state(self.state)
            .label_with(face);
        for f in self.attach {
            b = f(b);
        }
        let mut e = b
            .build(flat(theme))
            .h(m.height)
            .radius(m.radius)
            .gap(m.gap)
            .press_scale(motion::press::SCALE)
            .focus_ring(3.0, 0.0, c.ring.alpha(0.5));
        e = if m.square {
            e.w(m.height).px(0.0)
        } else {
            e.px(m.px)
        };
        let hovered = self.state == State::Hover;
        if let Some(f) = fill {
            e = e.fill(if hovered { over(hover(theme), f) } else { f });
        }
        let d = &theme.depth;
        let look = match self.state {
            State::Hover => Look::Hover,
            State::Pressed => Look::Pressed,
            State::Disabled => Look::Rest,
            _ => Look::Live,
        };
        let sheen = |stops: [(f32, crate::theme::palette::Oklch); 2]| {
            let [(_, top), (_, bottom)] = stops;
            Some((top.color(), bottom.color()))
        };
        // The face at rest and hovered (`button.tsx`'s `hover:` fills).
        let faces = match self.variant {
            Variant::Default => Some(Faces {
                rest: c.primary,
                hover: over(c.primary.alpha(0.9), c.background),
                sheen: sheen(fixed::SHEEN_STRONG),
            }),
            Variant::Outline => Some(Faces {
                rest: c.surface_raised,
                hover: if theme.dark {
                    over(Color::WHITE.alpha(0.04), c.surface_raised)
                } else {
                    c.muted
                },
                sheen: sheen(fixed::SHEEN),
            }),
            Variant::Secondary => Some(Faces {
                rest: c.secondary,
                hover: over(c.foreground.alpha(0.05), c.secondary),
                sheen: sheen(fixed::SHEEN),
            }),
            Variant::Ghost => Some(Faces {
                rest: Color::TRANSPARENT,
                hover: hover(theme),
                sheen: None,
            }),
            Variant::Destructive | Variant::Link => None,
        };
        e = match (faces, self.variant) {
            (Some(f), Variant::Ghost) => {
                raised_layers(e, m.radius, f, &[], &[], &d.pressed, look, &press)
            }
            (Some(f), _) => raised_layers(
                e,
                m.radius,
                f,
                &d.raised,
                &d.raised_hover,
                &d.pressed,
                look,
                &press,
            ),
            (None, _) => e,
        };
        if let (Some(keys), Look::Live) = (self.keys, look) {
            e = keys(e, &press);
        }
        match self.variant {
            Variant::Destructive => e = e.hover(c.destructive.alpha(0.1)),
            Variant::Link => e = e.hover(Color::TRANSPARENT).press(Color::TRANSPARENT),
            _ => {}
        }
        match self.state {
            State::Focus => e.outline(3.0, 0.0, c.ring.alpha(0.5)),
            State::Disabled => e.layer_opacity(0.5),
            _ => e,
        }
    }
}

/// `top` over `bottom` in sRGB, as the browser composites a hover tint.
fn over(top: Color, bottom: Color) -> Color {
    let a = top.a + bottom.a * (1.0 - top.a);
    if a <= 0.0 {
        return Color::TRANSPARENT;
    }
    let (enc, dec) = (rune::theme::srgb_encode, rune::theme::srgb_decode);
    let mix = |t: f32, b: f32| dec((enc(t) * top.a + enc(b) * bottom.a * (1.0 - top.a)) / a);
    Color {
        r: mix(top.r, bottom.r),
        g: mix(top.g, bottom.g),
        b: mix(top.b, bottom.b),
        a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `button.tsx`'s size classes: `h-6 px-2` (xs), `h-7 px-2.5` (sm), `h-8 px-2.5`
    /// (default), `h-9 px-2.5` (lg), the `size-*` squares, and their radii.
    #[test]
    fn sizes_are_the_tailwind_classes() {
        let hw = |s: Size| {
            let m = s.metrics();
            (m.height, m.px, m.radius, m.font.size, m.icon)
        };
        assert_eq!(hw(Size::Xs), (24.0, 9.0, 8.0, 12.0, 12.0));
        assert_eq!(hw(Size::Sm), (28.0, 11.0, 8.0, 12.8, 14.0));
        assert_eq!(hw(Size::Default), (32.0, 11.0, 10.0, 14.0, 16.0));
        assert_eq!(hw(Size::Lg), (36.0, 11.0, 10.0, 14.0, 16.0));
        assert_eq!(Size::IconXs.metrics().icon, 12.0);
        assert!(Size::IconLg.metrics().square && Size::IconLg.metrics().height == 36.0);
    }

    #[test]
    fn hover_tint_composites_like_the_browser() {
        let white = Color::WHITE;
        assert_eq!(over(Color::TRANSPARENT, white), white);
        let half = over(Color::BLACK.alpha(0.5), white);
        // sRGB-encoded 0.5 grey, not linear 0.5.
        assert!((rune::theme::srgb_encode(half.r) - 0.5).abs() < 1e-4);
    }
}
