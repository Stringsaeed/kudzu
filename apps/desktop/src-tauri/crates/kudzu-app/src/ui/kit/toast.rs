//! Toasts (`components/ui/sonner.tsx`, sonner's `toast()`), as `<Toaster position="bottom-center" />`
//! draws them: a 356 pt stack 24 pt above the window's bottom edge, newest in front.
//!
//! The API is what screens call; [`region`] draws them with sonner's look and behaviour:
//! - each toast `bg-popover` with a 1 pt `--border` edge, 10 pt corners (`--radius`), 16 pt
//!   padding and `0 4px 12px rgb(0 0 0 / .1)`; a 500-weight title over a 400-weight description,
//!   an optional `CircleCheck` / `OctagonX` icon for `success` / `error`, then Cancel (a translucent
//!   chip) and the action (an inverted chip);
//! - collapsed, older toasts tuck behind the front one, 14 pt up and 5 % smaller each, cut to its
//!   height with their content hidden, three at most; hovering the stack spreads them out 14 pt
//!   apart and pauses every timer, leaving resumes them with the time they had left;
//! - a toast slides up from below and fades in, and leaves down and out (the front one) or sinks
//!   40 % and fades (one behind), all on sonner's 400 ms transitions;
//! - each dismisses itself after `duration_ms` (4000 by default); its action or cancel button
//!   runs its handler and dismisses it.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rune::{AppHandle, Color, Cx, Effect, El, Shadow, Spring, SpringPair, Transition, Tx, Vec2, col, el, icon, row, text};

use crate::ui::icons;
use crate::ui::theme::{text as ty, weight, WeightOf, RADIUS};
use crate::ui::theme_for;

type Handler<S> = Arc<dyn Fn(&mut S, &mut Tx) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToastKind {
    #[default]
    Normal,
    Success,
    Error,
}

pub struct Toast<S> {
    pub id: u64,
    pub title: String,
    pub description: Option<String>,
    pub action: Option<(String, Handler<S>)>,
    pub cancel: Option<(String, Handler<S>)>,
    /// Milliseconds on screen; sonner's default is 4000.
    pub duration_ms: u64,
    pub kind: ToastKind,
    /// When it was shown, in the app clock's seconds (set by [`Toasts::push`]'s caller frame).
    pub shown_at: Option<f64>,
}

impl<S> Clone for Toast<S> {
    fn clone(&self) -> Self {
        Toast {
            id: self.id,
            title: self.title.clone(),
            description: self.description.clone(),
            action: self.action.clone(),
            cancel: self.cancel.clone(),
            duration_ms: self.duration_ms,
            kind: self.kind,
            shown_at: self.shown_at,
        }
    }
}

impl<S> Toast<S> {
    pub fn new(title: impl Into<String>) -> Self {
        Toast { id: 0, title: title.into(), description: None, action: None, cancel: None, duration_ms: 4000, kind: ToastKind::Normal, shown_at: None }
    }

    /// `toast.error(message)`.
    pub fn error(message: impl Into<String>) -> Self {
        Toast { kind: ToastKind::Error, ..Toast::new(message) }
    }

    /// `toast.success(message)`.
    pub fn success(message: impl Into<String>) -> Self {
        Toast { kind: ToastKind::Success, ..Toast::new(message) }
    }

    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn action(mut self, label: impl Into<String>, f: impl Fn(&mut S, &mut Tx) + Send + Sync + 'static) -> Self {
        self.action = Some((label.into(), Arc::new(f)));
        self
    }

    pub fn cancel(mut self, label: impl Into<String>, f: impl Fn(&mut S, &mut Tx) + Send + Sync + 'static) -> Self {
        self.cancel = Some((label.into(), Arc::new(f)));
        self
    }
}

/// A toast's auto-dismiss timer: time left, and when it last started counting.
#[derive(Clone, Copy, Debug)]
struct Timer {
    left: Duration,
    since: Option<Instant>,
    armed: bool,
}

pub struct Toasts<S> {
    pub items: Vec<Toast<S>>,
    next: u64,
    /// Hovered: spread out, timers paused.
    expanded: bool,
    timers: HashMap<u64, Timer>,
    /// Natural content heights, from layout.
    heights: HashMap<u64, f32>,
    /// Bumped on every pause, so timers armed before it do nothing.
    generation: u64,
}

impl<S> Default for Toasts<S> {
    fn default() -> Self {
        Toasts { items: Vec::new(), next: 1, expanded: false, timers: HashMap::new(), heights: HashMap::new(), generation: 0 }
    }
}

impl<S> Toasts<S> {
    /// Shows a toast.
    pub fn push(&mut self, toast: Toast<S>) {
        self.show(toast);
    }

    /// Shows a toast; returns its id, for dismissing or replacing it later.
    pub fn show(&mut self, mut toast: Toast<S>) -> u64 {
        toast.id = self.next;
        self.next += 1;
        let since = (!self.expanded).then(Instant::now);
        self.timers.insert(toast.id, Timer { left: Duration::from_millis(toast.duration_ms), since, armed: false });
        self.items.push(toast);
        self.next - 1
    }

    pub fn dismiss(&mut self, id: u64) {
        self.items.retain(|t| t.id != id);
        self.timers.remove(&id);
        self.heights.remove(&id);
    }

    /// Whether the stack is spread out (the pointer is over it).
    pub fn expanded(&self) -> bool {
        self.expanded
    }

    /// Hovering spreads the stack and pauses every timer; leaving resumes them with what they
    /// had left. Returns the timers to arm.
    fn set_expanded(&mut self, on: bool) -> Vec<(u64, u64, Duration)> {
        if self.expanded == on {
            return Vec::new();
        }
        self.expanded = on;
        self.generation += 1;
        let now = Instant::now();
        let mut arm = Vec::new();
        for (&id, t) in self.timers.iter_mut() {
            if on {
                if let Some(s) = t.since.take() {
                    t.left = t.left.saturating_sub(now.duration_since(s));
                }
                t.armed = false;
            } else {
                t.since = Some(now);
                t.armed = true;
                arm.push((id, self.generation, t.left));
            }
        }
        arm
    }

    /// Arms a running timer that hasn't been yet. Returns what to wait.
    fn arm(&mut self, id: u64) -> Option<(u64, u64, Duration)> {
        let g = self.generation;
        let t = self.timers.get_mut(&id)?;
        if t.armed || t.since.is_none() {
            return None;
        }
        t.armed = true;
        Some((id, g, t.left))
    }

    /// A timer ran out: dismisses the toast unless a pause came since it was armed.
    fn expire(&mut self, id: u64, generation: u64) -> bool {
        if generation != self.generation || self.expanded {
            return false;
        }
        let due = self.timers.get(&id).and_then(|t| t.since.map(|s| s.elapsed() + Duration::from_millis(5) >= t.left)).unwrap_or(false);
        if due {
            self.dismiss(id);
        }
        due
    }
}

/// A window state that hosts kit components needing the UI thread later: toasts' timers and the
/// app picker's installed-apps list land through its handle.
pub trait KitHost: Sized + 'static {
    fn toasts_mut(&mut self) -> &mut Toasts<Self>;
    fn kit_handle(&self) -> Option<AppHandle<Self>>;
}

impl KitHost for crate::ui::main::MainUi {
    fn toasts_mut(&mut self) -> &mut Toasts<Self> {
        &mut self.toasts
    }
    fn kit_handle(&self) -> Option<AppHandle<Self>> {
        self.me.clone()
    }
}

fn start_timers<S: KitHost>(handle: Option<AppHandle<S>>, timers: Vec<(u64, u64, Duration)>) {
    let Some(h) = handle else { return };
    for (id, generation, wait) in timers {
        let h = h.clone();
        std::thread::spawn(move || {
            std::thread::sleep(wait);
            h.update(move |s: &mut S, _| {
                s.toasts_mut().expire(id, generation);
            });
        });
    }
}

/// sonner's constants.
const WIDTH: f32 = 356.0;
const GAP: f32 = 14.0;
const OFFSET: f32 = 24.0;
const VISIBLE: usize = 3;
/// `transition: transform 400ms, opacity 400ms, height 400ms` (CSS `ease`).
const MOVE: Spring = Spring::new(0.4, 0.0);

/// The toast stack, mounted last over the window.
pub fn region<S: KitHost>(toasts: &Toasts<S>, cx: &mut Cx<S>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let dark = th.dark;
    let expanded = toasts.expanded();
    let (ww, wh) = super::popover::window_size(cx);
    // Newest in front.
    let order: Vec<&Toast<S>> = toasts.items.iter().rev().collect();
    let h_of = |id: u64| toasts.heights.get(&id).copied().unwrap_or(54.0);
    let front_h = order.first().map_or(0.0, |x| h_of(x.id));
    let shown = order.len().min(VISIBLE);
    let stack_h = if expanded {
        order.iter().take(VISIBLE).map(|x| h_of(x.id)).sum::<f32>() + GAP * shown.saturating_sub(1) as f32
    } else if shown > 0 {
        front_h + GAP * (shown - 1) as f32
    } else {
        0.0
    };
    let list = el()
        .key("kit-toaster")
        .role("region")
        .label("Notifications")
        .absolute(((ww - WIDTH) / 2.0).round(), (wh - OFFSET - stack_h).round())
        .w(WIDTH)
        .h(stack_h)
        .motion(MOVE)
        .on_hover_change(move |s: &mut S, inside, _| {
            let arm = s.toasts_mut().set_expanded(inside);
            start_timers(s.kit_handle(), arm);
        });
    let mut before = 0.0f32;
    let mut cards = Vec::new();
    for (i, toast) in order.iter().enumerate() {
        let id = toast.id;
        let front = i == 0;
        let natural = h_of(id);
        let (y, scale, height, content_on) = if expanded {
            let y = -(before + GAP * i as f32);
            (y, 1.0, natural, true)
        } else if front {
            (0.0, 1.0, natural, true)
        } else {
            (-GAP * i as f32, 1.0 - 0.05 * i as f32, front_h, false)
        };
        before += natural;
        let visible = i < VISIBLE;
        // How it leaves, decided now, as sonner's data attributes are when it's removed.
        let removal = if front || expanded {
            Effect { offset_fraction: Vec2::new(0.0, 1.0), opacity: 0.0, ..Effect::IDENTITY }
        } else {
            Effect { offset_fraction: Vec2::new(0.0, 0.4), opacity: 0.0, ..Effect::IDENTITY }
        };
        let exit = if front || expanded { MOVE } else { Spring::new(0.5, 0.0) };
        let transition = Transition {
            insertion: Effect { offset_fraction: Vec2::new(0.0, 1.0), opacity: 0.0, ..Effect::IDENTITY },
            removal,
            pop_layout: true,
            springs: SpringPair::new(MOVE, exit),
        };
        let content = toast_body(th, toast, dark)
            .opacity(if content_on { 1.0 } else { 0.0 })
            .opacity_motion(MOVE)
            .on_layout_tx(move |s: &mut S, r, _| {
                let host = s.toasts_mut();
                let h = r[3];
                if host.heights.get(&id).is_none_or(|old| (old - h).abs() > 0.5) {
                    host.heights.insert(id, h);
                }
                let arm = host.arm(id);
                start_timers(s.kit_handle(), arm.into_iter().collect());
            });
        let mut card = col()
            .key(format!("kit-toast-{id}"))
            .role("status")
            .label(toast.title.clone())
            .description(toast.description.clone().unwrap_or_default())
            .pin(Some(0.0), None, Some(0.0), Some(0.0))
            .h(height)
            .radius(RADIUS)
            .fill(t.popover)
            .border(1.0, t.border)
            .box_shadow(Shadow { x: 0.0, y: 4.0, blur: 12.0, spread: 0.0, color: Color::BLACK.alpha(0.1), inset: false })
            .offset(0.0, y)
            .scale(scale)
            .motion(MOVE)
            .opacity(if visible { 1.0 } else { 0.0 })
            .transition(transition)
            .child(content);
        if !visible {
            card = card.inert();
        }
        cards.push(card);
    }
    // The front toast paints last, over the ones tucked behind it (sonner's `z-index`).
    list.children(cards.into_iter().rev())
}

/// The toast's row: icon, title and description, Cancel, the action.
fn toast_body<S: KitHost>(th: &crate::ui::theme::Theme, toast: &Toast<S>, dark: bool) -> El {
    let t = &th.t;
    let id = toast.id;
    let mut r = row().w_full().padding(17.0).gap(6.0).center_y(); // 16 pt padding inside the 1 pt border
    match toast.kind {
        ToastKind::Success => r = r.child(el().size(16.0, 16.0).ml(-3.0).mr(4.0).child(icon(&icons::CIRCLE_CHECK, 16.0).ml(-1.0).color(t.popover_foreground)).a11y_hidden()),
        ToastKind::Error => r = r.child(el().size(16.0, 16.0).ml(-3.0).mr(4.0).child(icon(&icons::OCTAGON_X, 16.0).ml(-1.0).color(t.popover_foreground)).a11y_hidden()),
        ToastKind::Normal => {}
    }
    let mut words = col().gap(2.0).grow(1.0).shrink().min_w(0.0).child(
        text(toast.title.clone()).font(ty::BASE.weight_of(weight::MEDIUM).lh(19.5)).color(t.popover_foreground).wrap().a11y_hidden(),
    );
    if let Some(d) = &toast.description {
        let ink = if dark { rune::oklch(0.9306, 0.0, 0.0) } else { rune::oklch(0.3637, 0.0, 0.0) };
        words = words.child(text(d.clone()).font(ty::BASE.lh(18.2)).color(ink).wrap().a11y_hidden());
    }
    r = r.child(words);
    let chip = |label: &str, fill: Color, ink: Color| {
        row().role("button").label(label.to_string()).focusable(true).focus_ring(2.0, 0.0, Color::BLACK.alpha(0.4)).h(24.0).px(8.0).radius(4.0).fill(fill).center()
            .child(text(label.to_string()).font(ty::XS.weight_of(weight::MEDIUM)).color(ink).a11y_hidden())
    };
    if let Some((label, f)) = &toast.cancel {
        let f = f.clone();
        let fill = if dark { Color::WHITE.alpha(0.3) } else { Color::BLACK.alpha(0.08) };
        r = r.child(chip(label, fill, t.popover_foreground).key("cancel").on_press_tx(move |s: &mut S, tx| {
            f(s, tx);
            s.toasts_mut().dismiss(id);
        }));
    }
    if let Some((label, f)) = &toast.action {
        let f = f.clone();
        r = r.child(chip(label, t.popover_foreground, t.popover).key("action").on_press_tx(move |s: &mut S, tx| {
            f(s, tx);
            s.toasts_mut().dismiss(id);
        }));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    struct S;

    #[test]
    fn hovering_pauses_and_resumes_with_the_time_left() {
        let mut t: Toasts<S> = Toasts::default();
        let id = t.show(Toast::new("Deleted").duration_ms(6000));
        assert!(t.arm(id).is_some());
        assert!(t.arm(id).is_none(), "armed once");
        let g = t.generation;
        assert!(t.set_expanded(true).is_empty());
        assert!(!t.expire(id, g), "a pause cancels the armed timer");
        let resumed = t.set_expanded(false);
        assert_eq!(resumed.len(), 1);
        assert!(resumed[0].2 <= Duration::from_millis(6000));
        assert!(!t.expire(id, resumed[0].1), "not due yet");
        assert_eq!(t.items.len(), 1);
    }

    #[test]
    fn dismiss_forgets_the_toast() {
        let mut t: Toasts<S> = Toasts::default();
        let a = t.show(Toast::new("a"));
        let b = t.show(Toast::error("b"));
        assert_ne!(a, b);
        t.dismiss(a);
        assert_eq!(t.items.len(), 1);
        assert_eq!(t.items[0].kind, ToastKind::Error);
        assert!(!t.timers.contains_key(&a));
    }
}
