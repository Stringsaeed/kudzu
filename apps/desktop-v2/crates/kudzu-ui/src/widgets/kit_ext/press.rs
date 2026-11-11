//! RUNE-GAP(#360: hover and press states for shadows and child geometry): delete this module
//! once an element can take a shadow (and a child's frame) per hover and press state.
//!
//! The raised controls change their depth under the pointer (`hover:shadow-raised-hover`,
//! `active:shadow-pressed`, `active:bg-none`), and the switch's knob stretches 1.2× from one edge
//! while the switch is held. Rune swaps a fill under hover and press (`El::hover`,
//! `El::press`) but not a shadow or a child's frame, and a view knows neither state. So:
//!
//! - **hover** is a pair of layers inside the control, crossfaded by `group_opacity` with the
//!   control as the hover group. Each layer is the control's face: its fill (rest or hovered),
//!   its shadows and, at rest, its sheen. Rune draws an element's outer shadow under its whole
//!   box and lets the element's own fill hide the inside (CSS clips it instead), so a layer
//!   must be filled or its drop shadow shows through the face;
//! - **press** is a [`Live`] value the control's drag handler sets on Down and clears on Up
//!   (`Tx::live_only`, so the view does not rebuild), bound to the pressed layer's opacity, or
//!   to the knob's frame with `live_rect`. The control keeps its `on_press`: Rune delivers the
//!   drag's Up and then the press.
//! - **keyboard press** (D21): Space on a focused control presses it on key-down, and the
//!   press commits, so the view rebuilds while the key is still down. The control's `on_key`
//!   records it as held by the keyboard ([`keyboard_held`]), a rebuilt control starts pressed,
//!   and its `on_key_up` (Rune routes a release to the element that had focus when the key went
//!   down) lets go. Return passes a button by in Rune, as in AppKit, so only Space presses.
//!
//! A pointer press that sees a rebuild while held (another event) starts a new value at rest,
//! so the look lets go early rather than sticking.

use std::cell::RefCell;

use rune::{el, Color, El, Key, KeyEvent, Live, Phase, Shadow};

use crate::theme::motion;

thread_local! {
    /// The control Space is holding down, by its press id.
    static KEY_HELD: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Whether the control `id` is held down by the keyboard.
pub fn keyboard_held(id: &str) -> bool {
    KEY_HELD.with(|k| k.borrow().as_deref() == Some(id))
}

fn set_keyboard_held(id: Option<String>) {
    KEY_HELD.with(|k| *k.borrow_mut() = id);
}

/// Space without Shift, Command, Option or Control (Rune presses nothing on Shift-Space).
fn is_space(e: &KeyEvent) -> bool {
    let m = &e.mods;
    e.key == Key::Text(" ".into()) && !(m.shift || m.cmd || m.alt || m.ctrl)
}

/// Space held on the element `id`: `down` runs on the press (before Rune presses the control,
/// which the handler leaves to it), `up` on its release.
pub fn on_space<S: 'static>(
    e: El,
    id: String,
    down: impl Fn() + 'static,
    up: impl Fn() + 'static,
) -> El {
    let held = id.clone();
    e.on_key(move |_: &mut S, ev: &KeyEvent, _| {
        if is_space(ev) {
            set_keyboard_held(Some(id.clone()));
            down();
        }
        false
    })
    .on_key_up(move |_: &mut S, ev: &KeyEvent, tx| {
        if ev.key == Key::Text(" ".into()) && keyboard_held(&held) {
            set_keyboard_held(None);
            up();
            tx.live_only();
        }
        false
    })
}

/// A raised control's press: 1 while held (pointer or Space), its complement for the raised
/// pair. A control rebuilt while Space holds it starts pressed.
#[derive(Clone)]
pub struct Press {
    id: String,
    held: Live<f32>,
    unheld: Live<f32>,
}

impl Press {
    /// `id` names the control across rebuilds (its kind and accessible name).
    pub fn new(id: impl Into<String>) -> Press {
        let id = id.into();
        let on = if keyboard_held(&id) { 1.0 } else { 0.0 };
        Press {
            id,
            held: Live::new(on),
            unheld: Live::new(1.0 - on),
        }
    }
    pub fn set(&self, on: bool) {
        self.held.set(if on { 1.0 } else { 0.0 });
        self.unheld.set(if on { 0.0 } else { 1.0 });
    }
    /// Space presses and releases the look until its key-up.
    pub fn keyboard<S: 'static>(&self, e: El) -> El {
        let (down, up) = (self.clone(), self.clone());
        on_space::<S>(
            e,
            self.id.clone(),
            move || down.set(true),
            move || up.set(false),
        )
    }
}

/// The press spring: the soft curve at `FAST` (the `press` utility sinks in `FAST`).
pub fn press_spring() -> rune::Spring {
    motion::soft_spring(motion::FAST)
}

/// A layer filling its parent, drawn over the parent's fill and under its content.
pub fn layer(key: &str, radius: f32, fill: Color, shadows: &[Shadow]) -> El {
    shadows.iter().fold(
        el().key(key.to_owned())
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .radius(radius)
            .fill(fill)
            .paint_order(-1)
            .a11y_hidden(),
        |e, s| e.box_shadow(*s),
    )
}

/// Which look a control shows, forced (galleries) or live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Live,
    Rest,
    Hover,
    Pressed,
}

/// The faces a raised control shows: its fill at rest and hovered (pressed keeps the hovered
/// fill, as `:active` implies `:hover`), and its sheen at rest (`active:bg-none` drops it).
#[derive(Clone, Copy, Debug)]
pub struct Faces {
    pub rest: Color,
    pub hover: Color,
    pub sheen: Option<(Color, Color)>,
}

/// A raised control's depth: `rest` (with its sheen, if any) and `hover` crossfaded by hover,
/// both fading out for `pressed` while held. `control` must be the hover group; `press` drives
/// the live look.
#[allow(clippy::too_many_arguments)]
pub fn raised_layers(
    control: El,
    radius: f32,
    faces: Faces,
    rest: &[Shadow],
    hover: &[Shadow],
    pressed: &[Shadow],
    look: Look,
    press: &Press,
) -> El {
    let mut rest_l = layer("depth-rest", radius, faces.rest, rest);
    if let Some((top, bottom)) = faces.sheen {
        rest_l = rest_l.gradient(top, bottom);
    }
    let hover_l = layer("depth-hover", radius, faces.hover, hover);
    let pressed_l = layer("depth-pressed", radius, faces.hover, pressed);
    let (rest_o, hover_o) = match look {
        Look::Hover => ((0.0, 0.0), (1.0, 1.0)),
        Look::Pressed => ((0.0, 0.0), (0.0, 0.0)),
        _ => ((1.0, 0.0), (0.0, 1.0)),
    };
    let stack = |e: El| {
        e.child(
            rest_l
                .group_opacity(rest_o.0, rest_o.1)
                .opacity_motion(motion::press::PAINT),
        )
        .child(
            hover_l
                .group_opacity(hover_o.0, hover_o.1)
                .opacity_motion(motion::press::PAINT),
        )
    };
    match look {
        Look::Live => {
            let p = press.clone();
            // One drag handler per element: the pressed layer and the raised pair read the
            // same press, the pair through its complement.
            let control = control.hover_group().on_drag_any(move |d, tx| {
                p.set(matches!(d.phase, Phase::Down) || (d.phase == Phase::Move && !d.cancelled()));
                tx.live_only();
            });
            let raised = stack(layer("depth", radius, Color::TRANSPARENT, &[]))
                .live_opacity(press.unheld.smooth(press_spring()));
            control
                .child(raised)
                .child(pressed_l.live_opacity(press.held.smooth(press_spring())))
        }
        Look::Pressed => stack(control.hover_group()).child(pressed_l),
        _ => stack(control.hover_group()).child(pressed_l.opacity(0.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use crate::widgets::kit_ext::{button, switch};
    use rune::{Cx, Env, Mods, Runtime};

    #[derive(Default)]
    struct Pane {
        presses: u32,
        on: bool,
    }

    fn pane(s: &Pane, _: &mut Cx<Pane>) -> El {
        let theme = Theme::light();
        rune::row()
            .gap(12.0)
            .padding(12.0)
            .child(
                button::Button::new(format!("Save {}", s.presses))
                    .name("Save")
                    .variant(button::Variant::Outline)
                    .on_press(|s: &mut Pane| s.presses += 1)
                    .build(theme)
                    .key("save"),
            )
            .child(
                switch::Switch::new(s.on)
                    .label("Open at login")
                    .on_toggle(|s: &mut Pane| s.on = !s.on)
                    .build(theme)
                    .key("login"),
            )
    }

    fn space() -> KeyEvent {
        KeyEvent {
            key: Key::Text(" ".into()),
            mods: Mods::default(),
        }
    }

    fn settle(rt: &mut Runtime<Pane>) {
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
    }

    fn pressed_opacity(rt: &Runtime<Pane>) -> f32 {
        rt.presented("depth-pressed").expect("a pressed layer").1
    }

    fn knob_width(rt: &Runtime<Pane>) -> f32 {
        rt.presented("face").expect("a knob face").0[2]
    }

    /// Space presses the focused button on key-down; the press rebuilds the view (its label
    /// changes), and the pressed depth stays until the key comes up.
    #[test]
    fn space_holds_a_buttons_pressed_depth_until_key_up() {
        let mut rt = Runtime::new(Pane::default(), pane, (400.0, 80.0), Env::new(true, 1.0));
        settle(&mut rt);
        assert!(pressed_opacity(&rt) < 0.01);
        rt.update(|_, tx| tx.focus("save", true));
        settle(&mut rt);
        rt.key(space());
        settle(&mut rt);
        assert_eq!(rt.state.presses, 1, "Space pressed it once");
        assert!(pressed_opacity(&rt) > 0.99, "{}", pressed_opacity(&rt));
        rt.key_up(space());
        settle(&mut rt);
        assert!(pressed_opacity(&rt) < 0.01, "{}", pressed_opacity(&rt));
    }

    /// Space toggles the focused switch on key-down; the knob stays stretched 1.2× from the edge
    /// facing its travel until the key comes up.
    #[test]
    fn space_holds_the_switch_knob_stretch_until_key_up() {
        let mut rt = Runtime::new(Pane::default(), pane, (400.0, 80.0), Env::new(true, 1.0));
        settle(&mut rt);
        let side = knob_width(&rt);
        rt.update(|_, tx| tx.focus("login", true));
        settle(&mut rt);
        rt.key(space());
        settle(&mut rt);
        assert!(rt.state.on, "Space toggled it");
        assert!(
            (knob_width(&rt) - side * 1.2).abs() < 0.05,
            "{}",
            knob_width(&rt)
        );
        rt.key_up(space());
        settle(&mut rt);
        assert!((knob_width(&rt) - side).abs() < 0.05, "{}", knob_width(&rt));
    }
}
