//! `components/ui/popover.tsx` (Radix Popover), plus the plumbing every anchored overlay in kit/
//! shares: the `bg-popover` surface, the `animate-in fade-in-0 zoom-in-95 slide-in-from-*-2` /
//! `animate-out fade-out-0 zoom-out-95` presence, and mounting next to a keyed trigger.
//!
//! ```ignore
//! let trigger = Popover::new("add-app")
//!     .open(s.picker_open)
//!     .on_open_change(|s: &mut MainUi, open, _| s.picker_open = open)
//!     .trigger(button)
//!     .align(Align::Start).width(288.0).padding(0.0).ring(false)
//!     .child(body)
//!     .build(cx, &mut layer);
//! ```

use std::rc::Rc;

use rune::{Align, Cx, Effect, El, Shadow, Side, Spring, SpringPair, Transition, Tx, Vec2, col, el, text};

use crate::ui::theme::{radius, text as ty, Theme, Utilities, WeightOf, MOTION_MODERATE, MOTION_SLOW};
use crate::ui::theme_for;

/// `duration-(--motion-moderate) ease-pop`: how menus, popovers, selects and tooltips open.
pub const OPEN: Spring = Spring::new(MOTION_MODERATE, 0.3);
/// `data-closed:duration-100 data-closed:ease-out`: how they close.
pub const CLOSE: Spring = Spring::new(0.1, 0.0);
/// `duration-(--motion-slow) ease-pop`: how dialogs open.
pub const DIALOG_OPEN: Spring = Spring::new(MOTION_SLOW, 0.3);
/// `data-closed:duration-(--motion-moderate) data-closed:ease-out`: how dialogs close.
pub const DIALOG_CLOSE: Spring = Spring::new(MOTION_MODERATE, 0.0);

pub(crate) type OpenChange<S> = Rc<dyn Fn(&mut S, bool, &mut Tx)>;
pub(crate) type Handler<S> = Rc<dyn Fn(&mut S, &mut Tx)>;

/// `origin-(--radix-*-content-transform-origin)`: the corner or edge facing the trigger.
pub(crate) fn origin(side: Side, align: Align) -> Vec2 {
    let along = match align {
        Align::Start => 0.0,
        Align::Center => 0.5,
        Align::End => 1.0,
    };
    match side {
        Side::Top => Vec2::new(along, 1.0),
        Side::Bottom => Vec2::new(along, 0.0),
        Side::Left => Vec2::new(1.0, along),
        Side::Right => Vec2::new(0.0, along),
    }
}

/// tw-animate's `animate-in fade-in-0 zoom-in-95 data-[side=*]:slide-in-from-*-2` on `enter`,
/// and `animate-out fade-out-0 zoom-out-95` on `exit`, about the trigger-facing origin.
pub(crate) fn pop(side: Side, align: Align, enter: Spring, exit: Spring) -> Transition {
    let anchor = origin(side, align);
    let slide = match side {
        Side::Bottom => Vec2::new(0.0, -8.0),
        Side::Top => Vec2::new(0.0, 8.0),
        Side::Left => Vec2::new(8.0, 0.0),
        Side::Right => Vec2::new(-8.0, 0.0),
    };
    Transition {
        insertion: Effect { opacity: 0.0, scale: 0.95, anchor, offset: slide, ..Effect::IDENTITY },
        removal: Effect { opacity: 0.0, scale: 0.95, anchor, ..Effect::IDENTITY },
        pop_layout: true,
        springs: SpringPair::new(enter, exit),
    }
}

/// `fade-in-0 zoom-in-95` / `fade-out-0 zoom-out-95` about the centre (dialogs).
pub(crate) fn zoom(enter: Spring, exit: Spring) -> Transition {
    Transition {
        insertion: Effect { opacity: 0.0, scale: 0.95, ..Effect::IDENTITY },
        removal: Effect { opacity: 0.0, scale: 0.95, ..Effect::IDENTITY },
        pop_layout: true,
        springs: SpringPair::new(enter, exit),
    }
}

/// `fade-in-0` / `fade-out-0` on one spring (the dialog scrim's `duration-100`).
pub(crate) fn fade(spring: Spring) -> Transition {
    Transition { springs: SpringPair::new(spring, spring), ..Transition::opacity() }
}

/// `ring-1 ring-foreground/10`.
pub(crate) fn ring(th: &Theme) -> Shadow {
    Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: th.t.foreground.alpha(0.1), inset: false }
}

/// `rounded-lg bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10`: the
/// surface of menus, selects and popovers (`shadow-md` is `--elev-float`).
pub fn surface(th: &Theme) -> El {
    col().radius(radius::LG).fill(th.t.popover).box_shadow(ring(th)).elevate(&th.t.elev_float)
}

/// [`surface`] with `ring-0`.
pub fn surface_no_ring(th: &Theme) -> El {
    col().radius(radius::LG).fill(th.t.popover).elevate(&th.t.elev_float)
}

/// Mounts `content` on `side` of the element keyed `anchor`, `gap` points away, at whole points
/// as Radix's popper rounds, sized to the room the window leaves (`max-h-(--radix-*-available-height)`),
/// entering and leaving with [`pop`]. Its children land at once; only the surface animates.
pub(crate) fn mount(content: El, anchor: &str, side: Side, align: Align, gap: f32) -> El {
    let placed = if matches!(side, Side::Top | Side::Bottom) {
        content.anchored_fit(anchor, side, align, gap)
    } else {
        content.anchored(anchor, side, align, gap)
    };
    placed.anchored_round().transition(pop(side, align, OPEN, CLOSE)).instant_changes()
}

/// The window's size, from the kit root (falls back to the main window's default size).
pub(crate) fn window_size<S>(cx: &Cx<S>) -> (f32, f32) {
    cx.layout_size("kit-root").map_or((1180.0, 760.0), |s| (s[0], s[1]))
}

/// A Popover: Radix's root, trigger and content in one builder. The screen owns `open`.
pub struct Popover<S> {
    key: String,
    open: bool,
    change: Option<OpenChange<S>>,
    trigger: Option<El>,
    side: Side,
    align: Align,
    side_offset: f32,
    width: f32,
    padding: f32,
    gap: f32,
    ring: bool,
    blur: f32,
    label: Option<String>,
    body: Vec<El>,
}

impl<S: 'static> Popover<S> {
    pub fn new(key: impl Into<String>) -> Self {
        Popover {
            key: key.into(),
            open: false,
            change: None,
            trigger: None,
            side: Side::Bottom,
            align: Align::Center,
            side_offset: 4.0,
            width: 288.0,
            padding: 10.0,
            gap: 10.0,
            ring: true,
            blur: 0.0,
            label: None,
            body: Vec::new(),
        }
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// `onOpenChange`: the trigger toggles, Escape and presses outside close.
    pub fn on_open_change(mut self, f: impl Fn(&mut S, bool, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }
    /// `PopoverTrigger asChild`: the element pressing toggles. Its own `on_press` is replaced.
    pub fn trigger(mut self, t: El) -> Self {
        self.trigger = Some(t);
        self
    }
    pub fn side(mut self, s: Side) -> Self {
        self.side = s;
        self
    }
    /// `align` (default `center`).
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    /// `sideOffset` (default 4).
    pub fn side_offset(mut self, g: f32) -> Self {
        self.side_offset = g;
        self
    }
    /// `w-72` by default (288).
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    /// `p-2.5` by default (10); `p-0` for edge-to-edge bodies.
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = p;
        self
    }
    /// `gap-2.5` between children by default (10).
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    /// `ring-0` turns the 1 pt `ring-foreground/10` off.
    pub fn ring(mut self, on: bool) -> Self {
        self.ring = on;
        self
    }
    /// `shadow-float` (the same `--elev-float` as `shadow-md`; kept for call-site parity).
    pub fn float(self) -> Self {
        self
    }
    /// `backdrop-blur-*`: blur sigma behind the surface.
    pub fn backdrop_blur(mut self, sigma: f32) -> Self {
        self.blur = sigma;
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn child(mut self, c: El) -> Self {
        self.body.push(c);
        self
    }
    pub fn children(mut self, c: impl IntoIterator<Item = El>) -> Self {
        self.body.extend(c);
        self
    }

    /// Returns the trigger; the open content goes to `layer` (mounted by `kit::root`).
    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
        let th = theme_for(cx);
        let anchor = format!("{}-trigger", self.key);
        let open = self.open;
        let mut trigger = self.trigger.unwrap_or_else(el).expanded(open);
        if let Some(f) = self.change.clone() {
            trigger = trigger.on_press_tx(move |s: &mut S, tx| f(s, !open, tx));
        }
        let wrapped = el().key(anchor.clone()).child(trigger);
        if !open {
            return wrapped;
        }
        let base = if self.ring { surface(th) } else { surface_no_ring(th) };
        let mut content = base
            .key(format!("{}-content", self.key))
            .role("dialog")
            .w(self.width)
            .padding(self.padding)
            .gap(self.gap)
            .min_h(0.0)
            .focus_return(anchor.clone());
        if self.blur > 0.0 {
            content = content.backdrop_blur(self.blur);
        }
        if let Some(l) = self.label {
            content = content.label(l);
        }
        if let Some(f) = self.change {
            content = content.modal(true, move |s: &mut S| f(s, false, &mut Tx::default()));
        }
        content = content.children(self.body);
        layer.push(mount(content, &anchor, self.side, self.align, self.side_offset));
        wrapped
    }
}

/// `PopoverHeader`: `flex flex-col gap-0.5 text-sm`.
pub fn header(children: impl IntoIterator<Item = El>) -> El {
    col().gap(2.0).children(children)
}

/// `PopoverTitle`: `font-medium`, text-sm.
pub fn title<S>(cx: &Cx<S>, s: impl Into<String>) -> El {
    let th = theme_for(cx);
    text(s).font(ty::SM.weight_of(crate::ui::theme::weight::MEDIUM)).color(th.t.popover_foreground)
}

/// `PopoverDescription`: `text-muted-foreground`, text-sm.
pub fn description<S>(cx: &Cx<S>, s: impl Into<String>) -> El {
    let th = theme_for(cx);
    text(s).font(ty::SM).color(th.t.muted_foreground).wrap()
}

