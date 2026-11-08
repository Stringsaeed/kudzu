//! `components/ui/dialog.tsx` (Radix Dialog): the `bg-black/10 backdrop-blur-xs` scrim fading
//! over `duration-100`, and a centred `rounded-xl bg-popover p-4 shadow-panel ring-1` panel that
//! zooms in from 95 % on `--motion-slow` `ease-pop` and out on `--motion-moderate` `ease-out`.
//! Focus moves into the panel, Tab stays in it, Escape and a press on the scrim close it, and
//! focus returns to whatever had it before.
//!
//! ```ignore
//! Dialog::new("group-settings")
//!     .open(s.group_settings.is_some())
//!     .on_open_change(|s: &mut MainUi, open, _| if !open { s.group_settings = None })
//!     .max_width(520.0).gap(20.0).radius(radius::XXL).float().ring(false)
//!     .child(dialog::header(cx, "Group settings", Some("Snippets in this group …")))
//!     .child(body)
//!     .child(dialog::footer(cx, [cancel, save]))
//!     .build(cx, &mut layer);
//! ```

use std::rc::Rc;

use rune::{Color, Cx, El, Spring, Tx, col, el, row, text};

use super::button::{Button, Size, Variant};

use super::popover::{self, fade, window_size, zoom, OpenChange, DIALOG_CLOSE, DIALOG_OPEN};
use crate::ui::icons;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// `bg-black/10 supports-backdrop-filter:backdrop-blur-xs`, fading over `duration-100`.
pub(crate) fn scrim(key: String) -> El {
    el().key(key)
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .fill(Color::BLACK.alpha(0.1))
        .backdrop_blur(4.0)
        .transition(fade(Spring::new(0.1, 0.0)))
        .a11y_hidden()
}

/// A panel's dismissal (Escape, and a press outside when allowed).
pub(crate) type Close<S> = Option<Rc<dyn Fn(&mut S)>>;

/// Where a panel sits: centred, or its top at a fraction of the window (`top-[18%] translate-y-0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    Center,
    Top(f32),
}

/// Mounts a modal panel over a scrim. `panel` is laid out at `width`; its height is read back
/// from layout (`key`) to centre it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mount<S: 'static>(
    cx: &mut Cx<S>,
    layer: &mut Vec<El>,
    key: &str,
    panel: El,
    width: f32,
    placement: Placement,
    outside_closes: bool,
    close: Close<S>,
) {
    let (w, h) = window_size(cx);
    let width = width.min(w - 32.0).max(0.0);
    let panel_key = format!("{key}-panel");
    let ph = cx.layout_size(&panel_key).map_or(200.0, |s| s[1]);
    let x = ((w - width) / 2.0).round();
    let y = match placement {
        Placement::Center => ((h - ph) / 2.0).round(),
        Placement::Top(f) => (h * f).round(),
    };
    let mut panel = panel.key(panel_key).absolute(x, y).w(width).transition(zoom(DIALOG_OPEN, DIALOG_CLOSE)).instant_changes();
    if let Some(close) = close {
        panel = panel.modal(outside_closes, move |s: &mut S| close(s));
    }
    layer.push(scrim(format!("{key}-overlay")));
    layer.push(panel);
}

/// A Dialog: Radix's root, overlay and content. The screen owns `open`.
pub struct Dialog<S> {
    key: String,
    open: bool,
    change: Option<OpenChange<S>>,
    body: Vec<El>,
    close_button: bool,
    max_w: f32,
    radius: f32,
    padding: f32,
    gap: f32,
    shadow_float: bool,
    ring: bool,
    placement: Placement,
    blur: f32,
    label: Option<String>,
    description: Option<String>,
}

impl<S: 'static> Dialog<S> {
    pub fn new(key: impl Into<String>) -> Self {
        Dialog {
            key: key.into(),
            open: false,
            change: None,
            body: Vec::new(),
            close_button: true,
            max_w: 384.0,
            radius: radius::XL,
            padding: 16.0,
            gap: 16.0,
            shadow_float: false,
            ring: true,
            placement: Placement::Center,
            blur: 0.0,
            label: None,
            description: None,
        }
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    /// `onOpenChange(false)`: Escape, a press on the scrim, or the close button.
    pub fn on_open_change(mut self, f: impl Fn(&mut S, bool, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }
    /// `showCloseButton` (default true): the ghost `icon-sm` X at `top-2 right-2`.
    pub fn close_button(mut self, on: bool) -> Self {
        self.close_button = on;
        self
    }
    /// `sm:max-w-sm` (384) by default.
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_w = w;
        self
    }
    /// `rounded-xl` by default.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    /// `p-4` by default.
    pub fn padding(mut self, p: f32) -> Self {
        self.padding = p;
        self
    }
    /// `gap-4` by default.
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = g;
        self
    }
    /// `shadow-float` in place of `shadow-panel`.
    pub fn float(mut self) -> Self {
        self.shadow_float = true;
        self
    }
    /// `ring-0` turns the 1 pt `ring-foreground/10` off.
    pub fn ring(mut self, on: bool) -> Self {
        self.ring = on;
        self
    }
    /// `top-[18%] translate-y-0` and the like.
    pub fn top(mut self, fraction: f32) -> Self {
        self.placement = Placement::Top(fraction);
        self
    }
    /// `backdrop-blur-*` on the panel.
    pub fn backdrop_blur(mut self, sigma: f32) -> Self {
        self.blur = sigma;
        self
    }
    /// The accessible title (`DialogTitle`, also when it's `sr-only`).
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// The accessible description (`DialogDescription`, also when it's `sr-only`).
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
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

    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) {
        if !self.open {
            return;
        }
        let th = theme_for(cx);
        let mut panel = col()
            .role("dialog")
            .radius(self.radius)
            .fill(th.t.popover)
            .padding(self.padding)
            .gap(self.gap);
        if self.ring {
            panel = panel.box_shadow(popover::ring(th));
        }
        panel = panel.elevate(if self.shadow_float { &th.t.elev_float } else { &th.t.elev_panel });
        if self.blur > 0.0 {
            panel = panel.backdrop_blur(self.blur);
        }
        if let Some(l) = self.label {
            panel = panel.label(l);
        }
        if let Some(d) = self.description {
            panel = panel.description(d);
        }
        panel = panel.children(self.body);
        let close: Close<S> = self.change.clone().map(|f| Rc::new(move |s: &mut S| f(s, false, &mut Tx::default())) as Rc<dyn Fn(&mut S)>);
        if self.close_button {
            let mut b = Button::icon_only(&icons::X, "Close").variant(Variant::Ghost).size(Size::IconSm).id(format!("{}-close", self.key));
            if let Some(f) = self.change.clone() {
                b = b.on_press_tx(move |s: &mut S, tx| f(s, false, tx));
            }
            let b = b.build(th);
            panel = panel.child(el().key("close-slot").absolute(0.0, 8.0).w_full().justify_end().pr(8.0).child(b));
        }
        mount(cx, layer, &self.key, panel, self.max_w, self.placement, true, close);
    }
}

/// `DialogHeader` with `DialogTitle` and `DialogDescription`: `flex flex-col gap-2`.
pub fn header<S>(cx: &Cx<S>, title_text: impl Into<String>, description_text: Option<&str>) -> El {
    let mut h = col().gap(8.0).child(title(cx, title_text));
    if let Some(d) = description_text {
        h = h.child(description(cx, d));
    }
    h
}

/// `DialogTitle`: `font-heading text-base leading-none font-medium`.
pub fn title<S>(cx: &Cx<S>, s: impl Into<String>) -> El {
    let th = theme_for(cx);
    text(s).font(ty::LG.weight_of(weight::MEDIUM).lh(16.0)).color(th.t.popover_foreground)
}

/// `DialogDescription`: `text-sm text-muted-foreground`.
pub fn description<S>(cx: &Cx<S>, s: impl Into<String>) -> El {
    let th = theme_for(cx);
    text(s).font(ty::SM).color(th.t.muted_foreground).wrap()
}

/// `DialogFooter`: `-mx-4 -mb-4 flex flex-row justify-end gap-2 rounded-b-xl border-t bg-muted/50 p-4`.
/// `inset` is the panel's padding (16 for `p-4`), `r` its corner radius (`rounded-b-xl`).
pub fn footer<S>(cx: &Cx<S>, inset: f32, r: f32, children: impl IntoIterator<Item = El>) -> El {
    let th = theme_for(cx);
    row()
        .key("footer")
        .ml(-inset)
        .mr(-inset)
        .mb(-inset)
        .padding(16.0)
        .gap(8.0)
        .justify_end()
        .center_y()
        .fill(Theme::tint(th.t.muted, 0.5))
        .corners([0.0, 0.0, r, r])
        .border_sides([1.0, 0.0, 0.0, 0.0], th.t.border)
        .children(children)
}
