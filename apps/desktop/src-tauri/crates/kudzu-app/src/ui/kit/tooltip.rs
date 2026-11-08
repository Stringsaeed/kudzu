//! `components/ui/tooltip.tsx` (Radix Tooltip) under the main window's
//! `<TooltipProvider delayDuration={500} skipDelayDuration={200}>`: an inverted
//! `rounded-md bg-foreground px-3 py-1.5 text-xs text-background shadow-float` bubble with a
//! rotated-square arrow, `side="top"` by default, 10 pt off its trigger (Radix adds the arrow's
//! height to `sideOffset={0}`). It opens after the pointer rests 500 ms, at once if another
//! tooltip closed under 200 ms ago, and zooms and fades in on `--motion-moderate` `ease-pop`
//! (`slide-in-from-*-2` toward its trigger), out the same way.
//!
//! Hover state is the kit's own (one pointer, one open tooltip), so screens keep no state:
//!
//! ```ignore
//! let fav = Tooltip::new("fav").text("Favorite").shortcut("mod+shift+f")
//!     .build(cx, favorite_button, &mut layer);
//! ```
//!
//! Keycaps inside use Kbd's tooltip look (`bg-background/20 text-background`, `rounded-sm`),
//! and the bubble's right padding drops to `pr-1.5` when it ends in one.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use rune::{Align, Cx, El, Side, Spring, row, el, text};

use super::popover::{pop, OPEN};
use super::shortcut::{Shortcut, Tone};
use crate::ui::theme::{radius, text as ty};
use crate::ui::theme_for;

/// The provider's `delayDuration` (ms).
pub const DELAY_MS: u64 = 500;
/// The provider's `skipDelayDuration` (ms).
pub const SKIP_MS: u64 = 200;

#[derive(Default)]
struct Tip {
    /// Tooltips a gallery holds open.
    forced: Vec<String>,
    open: Option<String>,
    opened: Option<Instant>,
    delay: f32,
    hidden_at: Option<Instant>,
}

thread_local! {
    static TIP: RefCell<Tip> = RefCell::new(Tip::default());
}

fn enter(key: &str, delay: Duration) {
    TIP.with(|t| {
        let mut t = t.borrow_mut();
        let now = Instant::now();
        let skip = t.open.is_some() || t.hidden_at.is_some_and(|h| now.duration_since(h) < Duration::from_millis(SKIP_MS));
        t.delay = if skip { 0.0 } else { delay.as_secs_f32() };
        t.open = Some(key.to_string());
        t.opened = Some(now);
    });
}

fn leave(key: &str) {
    TIP.with(|t| {
        let mut t = t.borrow_mut();
        if t.open.as_deref() != Some(key) {
            return;
        }
        let now = Instant::now();
        let shown = t.opened.is_some_and(|o| now.duration_since(o).as_secs_f32() >= t.delay);
        if shown {
            t.hidden_at = Some(now);
        }
        t.open = None;
    });
}

/// Forces a tooltip open (galleries, snapshots): shown at once.
pub fn force_open(key: &str) {
    TIP.with(|t| {
        let mut t = t.borrow_mut();
        if !t.forced.iter().any(|k| k == key) {
            t.forced.push(key.to_string());
        }
    });
}

enum Part {
    Text(String),
    Shortcut(String),
    El(Box<El>),
}

pub struct Tooltip {
    key: String,
    side: Side,
    align: Align,
    parts: Vec<Part>,
    max_w: f32,
    delay: Duration,
}

impl Tooltip {
    pub fn new(key: impl Into<String>) -> Self {
        Tooltip { key: key.into(), side: Side::Top, align: Align::Center, parts: Vec::new(), max_w: 320.0, delay: Duration::from_millis(DELAY_MS) }
    }
    /// `TooltipContent side` (default `top`).
    pub fn side(mut self, s: Side) -> Self {
        self.side = s;
        self
    }
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.parts.push(Part::Text(s.into()));
        self
    }
    /// `<Shortcut combo />` inside the bubble.
    pub fn shortcut(mut self, combo: impl Into<String>) -> Self {
        self.parts.push(Part::Shortcut(combo.into()));
        self
    }
    pub fn child(mut self, e: El) -> Self {
        self.parts.push(Part::El(Box::new(e)));
        self
    }
    /// `max-w-xs` (320) by default; `className="max-w-64"` is 256.
    pub fn max_width(mut self, w: f32) -> Self {
        self.max_w = w;
        self
    }
    /// A different `delayDuration` for this tooltip.
    pub fn delay(mut self, d: Duration) -> Self {
        self.delay = d;
        self
    }

    /// Wraps `trigger` (`TooltipTrigger asChild`); the bubble goes to `layer` while open.
    pub fn build<S>(self, cx: &mut Cx<S>, trigger: El, layer: &mut Vec<El>) -> El {
        let th = theme_for(cx);
        let t = &th.t;
        let anchor = format!("{}-tip", self.key);
        let (k1, k2, delay) = (self.key.clone(), self.key.clone(), self.delay);
        let described = self.parts.iter().filter_map(|p| if let Part::Text(s) = p { Some(s.clone()) } else { None }).collect::<Vec<_>>().join(" ");
        let wrapped = el()
            .key(anchor.clone())
            .on_hover_change_any(move |inside, _| if inside { enter(&k1, delay) } else { leave(&k2) })
            .child(if described.is_empty() { trigger } else { trigger.description(described.clone()) });
        let (open, wait) = TIP.with(|tip| {
            let tip = tip.borrow();
            if tip.forced.iter().any(|k| k == &self.key) {
                return (true, 0.0);
            }
            (tip.open.as_deref() == Some(self.key.as_str()), tip.delay)
        });
        if !open {
            return wrapped;
        }
        let ends_in_kbd = matches!(self.parts.last(), Some(Part::Shortcut(_)));
        let mut bubble = row()
            .key(format!("{}-tip-content", self.key))
            .role("tooltip")
            .label(described)
            .max_w(self.max_w)
            .pl(12.0)
            .pr(if ends_in_kbd { 6.0 } else { 12.0 })
            .py(6.0)
            .gap(6.0)
            .center_y()
            .radius(radius::MD)
            .fill(t.foreground)
            .elevate_float(th);
        for p in self.parts {
            bubble = bubble.child(match p {
                Part::Text(s) => text(s).font(ty::XS).color(t.background).wrap().shrink().a11y_hidden(),
                Part::Shortcut(c) => Shortcut::new(c).tone(Tone::Tooltip).build(th),
                Part::El(e) => *e,
            });
        }
        // `size-2.5 rotate-45 rounded-[2px] bg-foreground translate-y-[calc(-50%_-_2px)]` at the
        // edge facing the trigger: its centre 2 pt inside the bubble, its tip 5 pt out.
        let diamond = el().size(10.0, 10.0).radius(2.0).fill(t.foreground).rotate(std::f32::consts::FRAC_PI_4).motion(Spring::NONE);
        let arrow = match self.side {
            Side::Top => el().pin(Some(0.0), None, Some(0.0), Some(-3.0)).h(10.0).justify_center().child(diamond),
            Side::Bottom => el().pin(Some(0.0), Some(-3.0), Some(0.0), None).h(10.0).justify_center().child(diamond),
            Side::Left => el().pin(None, Some(0.0), Some(-3.0), Some(0.0)).w(10.0).center().child(diamond),
            Side::Right => el().pin(Some(-3.0), Some(0.0), None, Some(0.0)).w(10.0).center().child(diamond),
        };
        bubble = bubble.child(arrow.a11y_hidden());
        let placed = bubble
            .anchored(anchor.clone(), self.side, self.align, 10.0)
            .anchored_round()
            .transition(pop(self.side, self.align, OPEN, OPEN))
            .instant_changes()
            .enter_delay(wait)
            .inert();
        layer.push(placed);
        wrapped
    }
}

trait ElevateFloat {
    fn elevate_float(self, th: &crate::ui::theme::Theme) -> Self;
}

impl ElevateFloat for El {
    /// `shadow-float`.
    fn elevate_float(self, th: &crate::ui::theme::Theme) -> El {
        use crate::ui::theme::Utilities;
        self.elevate(&th.t.elev_float)
    }
}
