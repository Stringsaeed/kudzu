//! `components/ui/switch.tsx`: an on/off switch. The thumb travels on `--spring-soft` and
//! stretches while held (`group-active:scale-x-[1.2]`), and the click target reaches 12 pt past the
//! track's sides and 8 pt past its top and bottom (`after:-inset-x-3 after:-inset-y-2`).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::rc::Rc;

use rune::{row, Color, El, Shadow, Tx};

use crate::ui::theme::{motion, radius, Theme, Utilities};

use super::button::{invalid_colors, is_pressed, layer, site_id, track_press, tw_spring, State};

/// `size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SwitchSize {
    /// 32 × 18.4 with a 16 pt thumb.
    #[default]
    Default,
    /// 24 × 14 with a 12 pt thumb.
    Sm,
}

type Bind = Rc<dyn Fn(El) -> El>;

/// `<Switch checked onCheckedChange />`.
pub struct Switch {
    checked: bool,
    size: SwitchSize,
    state: State,
    disabled: bool,
    invalid: bool,
    label: Option<String>,
    id: String,
    toggle: Option<Bind>,
}

impl Switch {
    #[track_caller]
    pub fn new(checked: bool) -> Switch {
        Switch {
            checked,
            size: SwitchSize::Default,
            state: State::Rest,
            disabled: false,
            invalid: false,
            label: None,
            id: site_id(std::panic::Location::caller(), "switch"),
            toggle: None,
        }
    }
    pub fn size(mut self, s: SwitchSize) -> Self {
        self.size = s;
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
    pub fn invalid(mut self, i: bool) -> Self {
        self.invalid = i;
        self
    }
    /// The accessible name (`aria-label`, or the `<label>` it sits in).
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// The press id, unique in the window, when several switches share a call site.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    /// `onCheckedChange`: the new value is `!checked`.
    pub fn on_toggle<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        let f = Rc::new(f);
        self.toggle = Some(Rc::new(move |e: El| {
            let f = f.clone();
            e.on_press(move |s: &mut S| f(s))
        }));
        self
    }
    pub fn on_toggle_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        let f = Rc::new(f);
        self.toggle = Some(Rc::new(move |e: El| {
            let f = f.clone();
            e.on_press_tx(move |s: &mut S, tx: &mut Tx| f(s, tx))
        }));
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let (w, h, knob) = match self.size {
            SwitchSize::Default => (32.0, 18.4, 16.0),
            SwitchSize::Sm => (24.0, 14.0, 12.0),
        };
        let on = self.checked;
        let enabled = !self.disabled;
        let pressed = enabled && (is_pressed(&self.id) || self.state == State::Pressed);
        let fill = if on { t.iris } else if th.dark { Theme::tint(t.input, 0.8) } else { t.input };
        let checked_shadow = [Shadow { y: 1.0, blur: 2.0, color: Color::BLACK.alpha(0.18), inset: true, ..Shadow::default() }];
        let shadows: &[Shadow] = if on { &checked_shadow } else { &t.elev_well };
        let thumb_fill = match (th.dark, on) {
            (true, true) => Color::WHITE,
            (true, false) => Theme::tint(t.foreground, 0.9),
            _ => t.background,
        };
        // `translate-x-[calc(100%-2px)]` when on; held, the thumb stretches 1.2× from its
        // outer side (`origin-left`, or `origin-right` when on).
        let travel = knob - 2.0;
        let width = if pressed { knob * 1.2 } else { knob };
        let x = if on { travel - (width - knob) } else { 0.0 };
        let soft = motion::soft();
        let thumb = rune::el()
            .key("thumb")
            .size(width, knob)
            .radius(radius::FULL)
            .fill(thumb_fill)
            .elevate(&t.elev_knob)
            .offset(x, 0.0)
            .offset_motion(soft)
            .motion(soft)
            .a11y_hidden();
        let mut e = row()
            .role("switch")
            .size(w, h)
            .padding(1.0)
            .center_y()
            .radius(radius::FULL)
            .fill(fill)
            .border(1.0, Color::TRANSPARENT)
            .motion(tw_spring())
            .checked(on);
        e = super::button::shadows(e, shadows, 3);
        if let Some(l) = &self.label {
            e = e.label(l.clone());
        }
        e = e.child(thumb);
        if self.invalid && enabled {
            let (b, r) = invalid_colors(th);
            e = e.border(1.0, b).outline(3.0, 0.0, r).no_focus_ring();
        } else if enabled && self.state == State::Focus {
            e = e.border(1.0, t.ring).outline(3.0, 0.0, Theme::tint(t.ring, 0.5));
        } else if enabled {
            e = e.focus_ring(3.0, 0.0, Theme::tint(t.ring, 0.5)).child(layer(0.0).radius(radius::FULL).focus_ring(1.0, -1.0, t.ring).key("focus-border"));
        }
        if !enabled {
            return e.disabled(true).layer_opacity(0.5);
        }
        e = e.focusable(true);
        let Some(bind) = self.toggle else { return e };
        // The wider click target: an invisible box over the track and around it.
        let hit = rune::el().key("hit").pin(Some(-12.0), Some(-8.0), Some(-12.0), Some(-8.0)).a11y_hidden();
        let hit = track_press(bind(hit), self.id.clone());
        let e = e.child(hit);
        track_press(bind(e), self.id)
    }
}
