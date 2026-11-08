//! `components/ui/button.tsx` (shadcn radix-nova, restyled with Kudzu's depth tokens), plus the
//! painting every raised kit control shares: the layered surface that gives CSS's hover tint,
//! hover shadow, `active:` shadow and sheen removal, keyboard-only focus rings and the `press`
//! sink.
//!
//! ```ignore
//! Button::new("Save").variant(Variant::Outline).size(Size::Sm).icon(&icons::PLUS)
//!     .on_press(|s: &mut MainUi| s.save()).build(th)
//! ```
//!
//! How CSS states map onto Rune:
//! - `hover:` fills, shadows and text colours: the button is a `hover_group`; each look is its own
//!   layer whose `group_opacity` (or `group_color`) crossfades on the `press` utility's 80 ms ease.
//! - `active:` (`shadow-pressed`, `bg-none`): Rune has no pressed style, so the button tracks its
//!   own press ([`track_press`]) and the view re-runs with the pressed look while it is held.
//! - `bg-clip-padding` over the transparent 1 pt border: fills, the sheen and inset shadows paint in
//!   the padding box (a face inset by the border); outer shadows belong to the border box.
//! - `focus-visible:border-ring ring-3 ring-ring/50`: two keyboard-only rings, the 1 pt border on a
//!   child and the 3 pt ring on the button.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::cell::RefCell;

use rune::{icon, row, text, Anim, Color, Curve, Easing, El, Icon, Phase, Shadow, Spring, TextStyle, Tx};

use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};

use super::shortcut::{Shortcut, Tone};

// MARK: Shared control painting

/// A look forced for galleries and stories (the web's `data-state` / devtools pseudo-states). At
/// rest the runtime drives hover, press and focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    Rest,
    Hover,
    Pressed,
    Focus,
}

thread_local! {
    /// The control a pointer press is held on (CSS `:active`), by its press id.
    static PRESSED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Whether the control with press id `id` is held down right now.
pub(crate) fn is_pressed(id: &str) -> bool {
    PRESSED.with(|p| p.borrow().as_deref() == Some(id))
}

/// Notes a press phase on the control `id`, for a control whose own drag handler tracks its press.
pub(crate) fn note_press(id: &str, phase: Phase) {
    match phase {
        Phase::Down => PRESSED.with(|p| *p.borrow_mut() = Some(id.to_string())),
        Phase::Up => PRESSED.with(|p| {
            let mut p = p.borrow_mut();
            if p.as_deref() == Some(id) {
                *p = None;
            }
        }),
        Phase::Move => {}
    }
}

/// Tracks presses on `e` (Down to Up, cancelled included) so the next view shows its `:active`
/// look. Moves change nothing and skip the view.
pub(crate) fn track_press(e: El, id: String) -> El {
    e.on_drag_any(move |d, tx: &mut Tx| {
        if d.phase == Phase::Move {
            tx.live_only();
        }
        note_press(&id, d.phase);
    })
}

/// A press id from the builder's call site and what it shows, unless the caller names one.
pub(crate) fn site_id(site: &'static std::panic::Location<'static>, what: &str) -> String {
    format!("{}:{}:{}:{what}", site.file(), site.line(), site.column())
}

/// Handlers collected by a builder and attached only when the control is enabled.
#[derive(Default)]
pub(crate) struct Acts(Vec<Box<dyn FnOnce(El) -> El>>);

impl Acts {
    pub(crate) fn push(&mut self, f: impl FnOnce(El) -> El + 'static) {
        self.0.push(Box::new(f));
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(crate) fn apply(self, mut e: El, enabled: bool) -> El {
        if enabled {
            for f in self.0 {
                e = f(e);
            }
        }
        e
    }
}

/// CSS's default timing function (`ease`), which the `press` utility's transitions use.
pub(crate) const EASE: Easing = Easing::EASE;
/// Tailwind's `--default-transition-timing-function`.
pub(crate) const TW_EASE: Easing = Easing::cubic(0.4, 0.0, 0.2, 1.0);
/// `--spring-soft`, the CSS `linear()` curve globals.css defines.
pub(crate) const SPRING_SOFT: Easing = Easing::Piecewise(&[
    [0.0, 0.0], [0.0625, 0.053], [0.125, 0.175], [0.1875, 0.324], [0.25, 0.473], [0.3125, 0.609], [0.375, 0.723],
    [0.4375, 0.814], [0.5, 0.883], [0.5625, 0.934], [0.625, 0.968], [0.6875, 0.991], [0.75, 1.005], [0.8125, 1.012],
    [0.875, 1.015], [0.9375, 1.015], [1.0, 1.0],
]);
/// `--spring-pop`.
pub(crate) const SPRING_POP: Easing = Easing::Piecewise(&[
    [0.0, 0.0], [0.05, 0.067], [0.1, 0.225], [0.15, 0.422], [0.2, 0.618], [0.25, 0.789], [0.3, 0.923], [0.35, 1.018],
    [0.4, 1.075], [0.45, 1.102], [0.5, 1.106], [0.55, 1.096], [0.6, 1.077], [0.65, 1.056], [0.7, 1.035], [0.75, 1.018],
    [0.8, 1.005], [0.85, 0.996], [0.9, 0.991], [0.95, 0.989], [1.0, 1.0],
]);

/// A CSS transition: `duration` seconds on `easing`.
pub(crate) fn tween(duration: f32, easing: Easing) -> Anim {
    Anim::new(Curve::Tween { duration, easing })
}

/// The `press` utility's 80 ms colour and shadow transitions, as a spring for properties that
/// only take one (fills and shadows changing between commits).
pub(crate) fn fast_spring() -> Spring {
    Spring::new(crate::ui::theme::MOTION_FAST, 0.0)
}

/// Tailwind's `transition-colors` / `transition-all` (150 ms), as a spring.
pub(crate) fn tw_spring() -> Spring {
    Spring::new(0.15, 0.0)
}

/// Hover fades: the `press` utility's `var(--motion-fast)` with CSS's `ease`, both ways.
pub(crate) fn hover_fast(e: El) -> El {
    e.hover_group().hover_motion(tween(crate::ui::theme::MOTION_FAST, EASE), tween(crate::ui::theme::MOTION_FAST, EASE))
}

/// An out-of-flow paint layer covering the parent's padding box, `inset` points in.
pub(crate) fn layer(inset: f32) -> El {
    rune::el().pin(Some(inset), Some(inset), Some(inset), Some(inset)).a11y_hidden()
}

/// Source-over of straight-alpha colours, mixed in sRGB as a browser composites them.
pub(crate) fn over(top: Color, bottom: Color) -> Color {
    let a = top.a + bottom.a * (1.0 - top.a);
    if a <= 0.0 {
        return Color::TRANSPARENT;
    }
    let (enc, dec) = (rune::theme::srgb_encode, rune::theme::srgb_decode);
    let mix = |t: f32, b: f32| dec((enc(t) * top.a + enc(b) * bottom.a * (1.0 - top.a)) / a);
    Color { r: mix(top.r, bottom.r), g: mix(top.g, bottom.g), b: mix(top.b, bottom.b), a }
}

/// `color-mix(in oklch, a p%, b)`: lightness, chroma and hue (the shorter way round) mixed; a
/// colour without chroma takes the other's hue, as CSS treats its hue as powerless.
pub(crate) fn mix_oklch(a: Color, b: Color, p: f32) -> Color {
    let lch = |c: Color| {
        let [l, x, y, al] = c.to_oklab();
        let ch = (x * x + y * y).sqrt();
        (l, ch, y.atan2(x), al, ch < 1e-4)
    };
    let (la, ca, mut ha, aa, na) = lch(a);
    let (lb, cb, mut hb, ab, nb) = lch(b);
    if na {
        ha = hb;
    }
    if nb {
        hb = ha;
    }
    let mut dh = ha - hb;
    if dh > std::f32::consts::PI {
        dh -= std::f32::consts::TAU;
    } else if dh < -std::f32::consts::PI {
        dh += std::f32::consts::TAU;
    }
    let (l, c, h, al) = (la * p + lb * (1.0 - p), ca * p + cb * (1.0 - p), hb + dh * p, aa * p + ab * (1.0 - p));
    Color::from_oklab([l, c * h.cos(), c * h.sin(), al])
}

fn outer(list: &[Shadow]) -> Vec<Shadow> {
    list.iter().filter(|s| !s.inset).copied().collect()
}

fn inner(list: &[Shadow]) -> Vec<Shadow> {
    list.iter().filter(|s| s.inset).copied().collect()
}

/// Each layer of `list`, padded with clear layers to `n` so a change to a list of another length
/// springs layer by layer instead of popping.
pub(crate) fn shadows(e: El, list: &[Shadow], n: usize) -> El {
    let mut e = e.elevate(list);
    for _ in list.len()..n {
        e = e.box_shadow(Shadow { color: Color::TRANSPARENT, ..Shadow::default() });
    }
    e
}

/// How a raised control paints in each state.
#[derive(Clone)]
pub(crate) struct Skin {
    /// The border box's corner radius.
    pub radius: f32,
    /// A transparent border the fill doesn't paint under (`border border-transparent
    /// bg-clip-padding`); 0 for none.
    pub rim: f32,
    pub fill: Color,
    /// `hover:bg-*`, when it differs.
    pub hover_fill: Option<Color>,
    pub shadows: Vec<Shadow>,
    /// `hover:shadow-*`.
    pub hover_shadows: Option<Vec<Shadow>>,
    /// `active:shadow-*`.
    pub pressed_shadows: Option<Vec<Shadow>>,
    /// `sheen` (false) or `sheen-strong` (true), removed while pressed (`active:bg-none`).
    pub sheen: Option<bool>,
    /// `focus-visible:border-*` (drawn in the rim).
    pub focus_border: Option<Color>,
    /// `focus-visible:ring-3 ring-*` (3 pt outside the box).
    pub focus_ring: Option<Color>,
    /// `aria-invalid`: border and ring colours.
    pub invalid: Option<(Color, Color)>,
    /// Per-corner radii (top-left, top-right, bottom-right, bottom-left) instead of `radius`.
    pub corners: Option<[f32; 4]>,
}

impl Skin {
    pub(crate) fn flat(radius: f32, fill: Color) -> Skin {
        Skin {
            radius,
            rim: 0.0,
            fill,
            hover_fill: None,
            shadows: Vec::new(),
            hover_shadows: None,
            pressed_shadows: None,
            sheen: None,
            focus_border: None,
            focus_ring: None,
            invalid: None,
            corners: None,
        }
    }

    /// Rounds `e`, `inset` points inside the border box.
    pub(crate) fn round(&self, e: El, inset: f32) -> El {
        match self.corners {
            Some(c) => e.corners(c.map(|v| (v - inset).max(0.0))),
            None => e.radius((self.radius - inset).max(0.0)),
        }
    }
}

/// What a control shows now.
#[derive(Clone, Copy, Default)]
pub(crate) struct Look {
    /// Forced hover (a gallery's state, or held down).
    pub hover: bool,
    /// Held down (tracked or forced).
    pub pressed: bool,
    /// Forced keyboard focus.
    pub focus: bool,
    /// Enabled: hover follows the pointer and focus rings follow the keyboard.
    pub live: bool,
}

impl Look {
    pub(crate) fn of(state: State, pressed: bool, live: bool) -> Look {
        let pressed = live && (pressed || state == State::Pressed);
        Look { hover: live && (state == State::Hover || pressed), pressed, focus: live && state == State::Focus, live }
    }
}

/// The paint layers for `skin` in `look`, to go first among the control's children, and the
/// control's own rings. The control should be a [`hover_fast`] group.
pub(crate) fn paint(mut e: El, skin: &Skin, look: Look) -> El {
    let rim = skin.rim;
    let rest = if look.pressed {
        skin.pressed_shadows.as_deref().unwrap_or(&skin.shadows)
    } else if look.hover {
        skin.hover_shadows.as_deref().unwrap_or(&skin.shadows)
    } else {
        &skin.shadows
    };
    let hover = if look.pressed { rest } else { skin.hover_shadows.as_deref().unwrap_or(&skin.shadows) };
    // Live hover crossfades to the hover layers; a forced or held hover paints them outright.
    let live_hover = look.live && !look.hover;
    let n_outer = [&skin.shadows, skin.hover_shadows.as_ref().unwrap_or(&skin.shadows), skin.pressed_shadows.as_ref().unwrap_or(&skin.shadows)]
        .iter()
        .map(|l| outer(l).len())
        .max()
        .unwrap_or(0);
    let n_inner = [&skin.shadows, skin.hover_shadows.as_ref().unwrap_or(&skin.shadows), skin.pressed_shadows.as_ref().unwrap_or(&skin.shadows)]
        .iter()
        .map(|l| inner(l).len())
        .max()
        .unwrap_or(0);
    let shadow_hover = live_hover && skin.hover_shadows.is_some();
    // Outer shadows: the border box.
    {
        let mut l = shadows(skin.round(layer(0.0), 0.0), &outer(rest), n_outer).motion(fast_spring());
        if shadow_hover {
            l = l.group_opacity(1.0, 0.0);
        }
        e = e.child(l.key("kit-shadow"));
        let l = shadows(skin.round(layer(0.0), 0.0), &outer(hover), n_outer).motion(fast_spring()).key("kit-shadow-hover");
        e = e.child(if shadow_hover { l.group_opacity(0.0, 1.0) } else { l.opacity(0.0) });
    }
    // The face: fill and inset shadows in the padding box.
    let hover_fill = skin.hover_fill.unwrap_or(skin.fill);
    let face_fill = if look.hover { hover_fill } else { skin.fill };
    let fill_hover = live_hover && (skin.hover_fill.is_some() || shadow_hover);
    let mut face = shadows(skin.round(layer(rim), rim).fill(face_fill), &inner(rest), n_inner).motion(fast_spring());
    if fill_hover {
        // An opaque hover face covers the rest face; a translucent one replaces it.
        face = face.group_opacity(1.0, if hover_fill.a >= 0.999 { 1.0 } else { 0.0 });
    }
    e = e.child(face.key("kit-face"));
    let face = shadows(skin.round(layer(rim), rim).fill(hover_fill), &inner(hover), n_inner).motion(fast_spring()).key("kit-face-hover");
    e = e.child(if fill_hover { face.group_opacity(0.0, 1.0) } else { face.opacity(0.0) });
    // Keep paint slots even when disabled, pressed or toggled: callers can supply arbitrary
    // children (including intentionally dynamic keys) after these layers.
    e = e.child(skin.round(layer(rim), rim).sheen(skin.sheen.unwrap_or(false)).opacity(if skin.sheen.is_some() && !look.pressed { 1.0 } else { 0.0 }).opacity_motion(Anim::INSTANT).key("kit-sheen"));
    let focus_border = skin.round(layer(0.0), 0.0).key("kit-focus-border");
    e = e.child(match skin.focus_border.filter(|_| rim > 0.0 && look.live && skin.invalid.is_none()) {
        Some(c) if look.focus => focus_border.border(rim, c),
        Some(c) => focus_border.focus_ring(rim, -rim, c),
        None => focus_border.opacity(0.0),
    });
    if let Some((border, ring)) = skin.invalid {
        // `aria-invalid` outranks focus: its border and ring hold, focused or not.
        return e.border(rim.max(1.0), border).outline(3.0, 0.0, ring).no_focus_ring();
    }
    if !look.live {
        return e;
    }
    let ring = skin.focus_ring;
    if look.focus {
        return match ring {
            Some(c) => e.outline(3.0, 0.0, c),
            None => e,
        };
    }
    match ring {
        Some(c) => e.focus_ring(3.0, 0.0, c),
        None => e.no_focus_ring(),
    }
}

/// Text or icon colour `rest`, turning `hover` while the control is hovered (or forced).
pub(crate) fn ink(e: El, rest: Color, hover: Option<Color>, look: Look) -> El {
    match hover {
        Some(h) if look.hover => e.color(h),
        Some(h) if look.live => e.color(rest).group_color(h),
        _ => e.color(rest),
    }
}

// MARK: Button

/// `variant`: the cva variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Variant {
    #[default]
    Default,
    Outline,
    Secondary,
    Ghost,
    Destructive,
    Link,
}

/// `size`: the cva sizes. `Icon*` sizes are square.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Size {
    #[default]
    Default,
    Xs,
    Sm,
    Lg,
    Icon,
    IconXs,
    IconSm,
    IconLg,
}

impl Size {
    /// Height (or side), horizontal padding, gap, padding beside a `data-icon` icon, radius, icon
    /// size and font.
    fn metrics(self) -> (f32, f32, f32, f32, f32, f32, TextStyle) {
        let md = ty::SM.weight_of(weight::MEDIUM);
        let small_r = radius::MD.min(10.0);
        match self {
            Size::Default => (32.0, 10.0, 6.0, 8.0, radius::LG, 16.0, md),
            Size::Xs => (24.0, 8.0, 4.0, 6.0, small_r, 12.0, ty::XS.weight_of(weight::MEDIUM)),
            Size::Sm => (28.0, 10.0, 4.0, 6.0, radius::MD.min(12.0), 14.0, TextStyle { size: 12.8, ..md }),
            Size::Lg => (36.0, 10.0, 6.0, 8.0, radius::LG, 16.0, md),
            Size::Icon => (32.0, 0.0, 6.0, 0.0, radius::LG, 16.0, md),
            Size::IconXs => (24.0, 0.0, 4.0, 0.0, small_r, 12.0, md),
            Size::IconSm => (28.0, 0.0, 4.0, 0.0, radius::MD.min(12.0), 16.0, md),
            Size::IconLg => (36.0, 0.0, 6.0, 0.0, radius::LG, 16.0, md),
        }
    }

    fn square(self) -> bool {
        matches!(self, Size::Icon | Size::IconXs | Size::IconSm | Size::IconLg)
    }
}

/// The skin of a button `variant`, and its text colour at rest and on hover.
pub(crate) fn button_skin(th: &Theme, variant: Variant, radius: f32, expanded: bool) -> (Skin, Color, Option<Color>) {
    let t = &th.t;
    let dark = th.dark;
    let raised = |fill: Color, hover: Color, strong: bool| Skin {
        radius,
        rim: 1.0,
        fill,
        hover_fill: Some(hover),
        shadows: t.elev_raised.clone(),
        hover_shadows: Some(t.elev_raised_hover.clone()),
        pressed_shadows: Some(t.elev_pressed.clone()),
        sheen: Some(strong),
        focus_border: Some(t.ring),
        focus_ring: Some(Theme::tint(t.ring, 0.5)),
        invalid: None,
        corners: None,
    };
    let mut flat = Skin { rim: 1.0, focus_border: Some(t.ring), focus_ring: Some(Theme::tint(t.ring, 0.5)), ..Skin::flat(radius, Color::TRANSPARENT) };
    match variant {
        Variant::Default => (raised(t.primary, Theme::tint(t.primary, 0.9), true), t.primary_foreground, None),
        Variant::Outline => {
            let hover = if dark { mix_oklch(Color::WHITE, t.surface_raised, 0.04) } else { t.muted };
            let mut s = raised(t.surface_raised, hover, false);
            if expanded {
                s.fill = t.muted;
                s.hover_fill = Some(hover);
            }
            (s, t.foreground, Some(t.foreground))
        }
        Variant::Secondary => {
            let hover = mix_oklch(t.foreground, t.secondary, 0.05);
            (raised(t.secondary, hover, false), t.secondary_foreground, None)
        }
        Variant::Ghost => {
            let hover = if dark { Theme::tint(t.muted, 0.5) } else { t.muted };
            flat.hover_fill = Some(hover);
            flat.pressed_shadows = Some(t.elev_pressed.clone());
            if expanded {
                flat.fill = t.muted;
            }
            (flat, t.foreground, Some(t.foreground))
        }
        Variant::Destructive => {
            let (rest, hover, ring) = if dark { (0.2, 0.3, 0.4) } else { (0.1, 0.2, 0.2) };
            flat.fill = Theme::tint(t.destructive, rest);
            flat.hover_fill = Some(Theme::tint(t.destructive, hover));
            flat.focus_border = Some(Theme::tint(t.destructive, 0.4));
            flat.focus_ring = Some(Theme::tint(t.destructive, ring));
            (flat, t.destructive, None)
        }
        Variant::Link => (flat, t.primary, None),
    }
}

/// `aria-invalid`'s border and ring.
pub(crate) fn invalid_colors(th: &Theme) -> (Color, Color) {
    let d = th.t.destructive;
    if th.dark { (Theme::tint(d, 0.5), Theme::tint(d, 0.4)) } else { (d, Theme::tint(d, 0.2)) }
}

enum Part {
    Icon(&'static Icon),
    Child(Box<El>),
    Shortcut(String),
}

/// A button: `Button::new("Save").variant(Variant::Outline).size(Size::Sm).build(th)`.
pub struct Button {
    label: String,
    variant: Variant,
    size: Size,
    lead: Option<(&'static Icon, bool)>,
    trail: Option<(&'static Icon, bool)>,
    parts: Vec<Part>,
    icon_size: Option<f32>,
    font: Option<TextStyle>,
    color: Option<Color>,
    hover_color: Option<Color>,
    height: Option<f32>,
    pad_x: Option<f32>,
    radius: Option<f32>,
    full_width: bool,
    width: Option<f32>,
    justify_start: bool,
    show_label: bool,
    state: State,
    disabled: bool,
    invalid: bool,
    expanded: Option<bool>,
    haspopup: bool,
    help: Option<String>,
    id: String,
    acts: Acts,
}

impl Button {
    /// A text button; `label` is also its accessible name.
    #[track_caller]
    pub fn new(label: impl Into<String>) -> Button {
        let label = label.into();
        let id = site_id(std::panic::Location::caller(), &label);
        Button {
            label,
            variant: Variant::Default,
            size: Size::Default,
            lead: None,
            trail: None,
            parts: Vec::new(),
            icon_size: None,
            font: None,
            color: None,
            hover_color: None,
            height: None,
            pad_x: None,
            radius: None,
            full_width: false,
            width: None,
            justify_start: false,
            show_label: true,
            state: State::Rest,
            disabled: false,
            invalid: false,
            expanded: None,
            haspopup: false,
            help: None,
            id,
            acts: Acts::default(),
        }
    }

    /// An icon-only button (size `Icon` unless set): `label` names it for assistive technology
    /// (the web's `aria-label`).
    #[track_caller]
    pub fn icon_only(i: &'static Icon, label: impl Into<String>) -> Button {
        let mut b = Button::new(label);
        b.size = Size::Icon;
        b.show_label = false;
        b.lead = Some((i, false));
        b
    }

    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    /// A leading icon (a plain `<svg>` child: no `data-icon`, so padding stays).
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.lead = Some((i, false));
        self
    }
    /// A leading icon marked `data-icon="inline-start"`: the start padding tightens.
    pub fn icon_start(mut self, i: &'static Icon) -> Self {
        self.lead = Some((i, true));
        self
    }
    /// A trailing icon after the label (a plain `<svg>`).
    pub fn icon_after(mut self, i: &'static Icon) -> Self {
        self.trail = Some((i, false));
        self
    }
    /// A trailing icon marked `data-icon="inline-end"`: the end padding tightens.
    pub fn icon_end(mut self, i: &'static Icon) -> Self {
        self.trail = Some((i, true));
        self
    }
    /// Any element after the label (and before a trailing icon).
    pub fn child(mut self, e: El) -> Self {
        self.parts.push(Part::Child(Box::new(e)));
        self
    }
    /// A keycap hint after the label (`<Shortcut combo>`), restyled on a filled button as Kbd is.
    pub fn shortcut(mut self, combo: impl Into<String>) -> Self {
        self.parts.push(Part::Shortcut(combo.into()));
        self
    }
    /// An extra icon after the label.
    pub fn icon_child(mut self, i: &'static Icon) -> Self {
        self.parts.push(Part::Icon(i));
        self
    }
    /// `[&_svg]:size-*` on the icons.
    pub fn icon_size(mut self, s: f32) -> Self {
        self.icon_size = Some(s);
        self
    }
    /// A label font other than the size's (`text-xs`, `text-[13px] font-semibold`).
    pub fn font(mut self, f: TextStyle) -> Self {
        self.font = Some(f);
        self
    }
    /// `text-*`: the text colour at rest where the variant inherits it (outline, ghost) or a
    /// className overrides it.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// `hover:text-*`.
    pub fn hover_color(mut self, c: Color) -> Self {
        self.hover_color = Some(c);
        self
    }
    /// `h-*` other than the size's.
    pub fn height(mut self, h: f32) -> Self {
        self.height = Some(h);
        self
    }
    /// `px-*` other than the size's.
    pub fn padding_x(mut self, p: f32) -> Self {
        self.pad_x = Some(p);
        self
    }
    /// `rounded-*` other than the size's.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = Some(r);
        self
    }
    /// `w-full`.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }
    /// `w-*`.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// `justify-start`.
    pub fn justify_start(mut self) -> Self {
        self.justify_start = true;
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// `aria-invalid`.
    pub fn invalid(mut self, i: bool) -> Self {
        self.invalid = i;
        self
    }
    /// `aria-expanded` (a menu or popover trigger, open or not).
    pub fn expanded(mut self, open: bool) -> Self {
        self.expanded = Some(open);
        self
    }
    /// `aria-haspopup`: a menu trigger, which doesn't sink when pressed (its menu is anchored to it).
    pub fn haspopup(mut self) -> Self {
        self.haspopup = true;
        self
    }
    /// `title` / a tooltip.
    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = Some(h.into());
        self
    }
    /// The press id (unique in the window), for buttons built in a loop at one call site with
    /// the same label.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    pub fn on_press<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.acts.push(move |e| e.on_press(f));
        self
    }
    pub fn on_press_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_press_tx(f));
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let (h, px, gap, inline, r, icon_sz, font) = self.size.metrics();
        let h = self.height.unwrap_or(h);
        let r = self.radius.unwrap_or(r);
        let font = self.font.unwrap_or(font);
        let icon_sz = self.icon_size.unwrap_or(icon_sz);
        let enabled = !self.disabled;
        let look = Look::of(self.state, is_pressed(&self.id), enabled);
        let (mut skin, rest_ink, hover_ink) = button_skin(th, self.variant, r, self.expanded == Some(true));
        if self.invalid {
            skin.invalid = Some(invalid_colors(th));
        }
        let rest_ink = self.color.unwrap_or(rest_ink);
        let hover_ink = self.hover_color.or(hover_ink).filter(|&c| c != rest_ink || self.expanded == Some(true));
        let rest_ink = if self.expanded == Some(true) && matches!(self.variant, Variant::Outline | Variant::Ghost) { th.t.foreground } else { rest_ink };

        let mut e = hover_fast(row()).role("button").label(self.label.clone()).h(h).gap(gap).center().radius(r);
        e = if self.size.square() {
            e.w(h)
        } else {
            let px = self.pad_x.unwrap_or(px);
            let pl = if self.lead.is_some_and(|(_, d)| d) { inline } else { px };
            let pr = if self.trail.is_some_and(|(_, d)| d) { inline } else { px };
            e.pl(pl + skin.rim).pr(pr + skin.rim)
        };
        if self.full_width {
            e = e.w_full();
        }
        if let Some(w) = self.width {
            e = e.w(w);
        }
        if self.justify_start {
            e = e.justify_start();
        }
        e = paint(e, &skin, look);
        if look.pressed && self.state == State::Pressed && !self.haspopup {
            e = e.scale(0.97);
        }

        let glyph = |i: &'static Icon| ink(icon(i, icon_sz), rest_ink, hover_ink, look).a11y_hidden();
        if let Some((i, _)) = self.lead {
            e = e.child(glyph(i).key("icon"));
        } else {
            e = e.child(layer(0.0).key("icon").a11y_hidden());
        }
        if self.show_label {
            let mut label = ink(text(self.label.clone()).font(font), rest_ink, hover_ink, look).a11y_hidden().key("label");
            if self.variant == Variant::Link {
                label = link_underline(label, font, rest_ink, look);
            }
            e = e.child(label);
        } else {
            e = e.child(layer(0.0).key("label").a11y_hidden());
        }
        for p in self.parts {
            e = e.child(match p {
                Part::Icon(i) => glyph(i),
                Part::Child(c) => *c,
                Part::Shortcut(combo) => {
                    let tone = if self.variant == Variant::Default { Tone::OnAccent } else { Tone::Default };
                    Shortcut::new(combo).tone(tone).color(rest_ink).build(th)
                }
            });
        }
        if let Some((i, _)) = self.trail {
            e = e.child(glyph(i).key("icon-end"));
        }
        if let Some(h) = &self.help {
            e = e.help(h.clone());
        }
        if let Some(open) = self.expanded {
            e = e.expanded(open);
        }
        if !enabled {
            return e.disabled(true).layer_opacity(0.5);
        }
        e = e.focusable(true);
        if !self.haspopup {
            e = e.press_scale(0.97);
        }
        e = track_press(e, self.id.clone());
        self.acts.apply(e, true).invalid(self.invalid)
    }
}

/// `underline-offset-4 hover:underline`: a 1 pt rule 4 pt under the baseline, shown on hover.
pub(crate) fn link_underline(label: El, font: TextStyle, c: Color, look: Look) -> El {
    let line = font.line_px();
    // SF's baseline sits at about 0.8 of a centred line box's content for these sizes.
    let baseline = (line - font.size * 1.19) / 2.0 + font.size * 0.952;
    let mut rule = rune::el().pin(Some(0.0), Some(baseline + 4.0 - font.size * 0.07), Some(0.0), None).h(1.0).fill(c).a11y_hidden();
    rule = if look.hover { rule } else if look.live { rule.group_opacity(0.0, 1.0) } else { rule.opacity(0.0) };
    rune::el().key("label").child(label).child(rule)
}
