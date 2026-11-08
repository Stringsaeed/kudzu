//! `components/ui/checkbox.tsx`: a 16 pt checkbox. Checked, it turns into a raised primary key
//! with the strong sheen, and its check lands with `animate-check` (from 0.4× and clear, on
//! `--spring-pop`). The click target reaches past the box as the web's `after:` inset does.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::rc::Rc;

use rune::{icon, row, Beat, Color, El, Sequence, Spring, Tx};

use crate::ui::icons;
use crate::ui::theme::{radius, Theme, Utilities};

use super::button::{invalid_colors, layer, site_id, track_press, tw_spring, State, SPRING_POP};

/// `animate-check`: `check-in var(--motion-slow) var(--spring-pop)`.
pub(crate) const CHECK_IN: &[Beat] = &[Beat::new(0.0).opacity(0.0).scale(0.4), Beat::new(0.24).easing(SPRING_POP)];

pub(crate) type Bind = Rc<dyn Fn(El) -> El>;

pub(crate) fn bind<S: 'static>(f: impl Fn(&mut S) + 'static) -> Bind {
    let f = Rc::new(f);
    Rc::new(move |e: El| {
        let f = f.clone();
        e.on_press(move |s: &mut S| f(s))
    })
}

pub(crate) fn bind_tx<S: 'static>(f: impl Fn(&mut S, &mut Tx) + 'static) -> Bind {
    let f = Rc::new(f);
    Rc::new(move |e: El| {
        let f = f.clone();
        e.on_press_tx(move |s: &mut S, tx: &mut Tx| f(s, tx))
    })
}

/// The box a checkbox or radio item draws: 16 pt, `border border-input shadow-well`
/// (`dark:bg-input/30`), turning `border-primary bg-primary shadow-raised sheen-strong` when on.
pub(crate) struct Boxed {
    pub role: &'static str,
    pub round: bool,
    pub checked: bool,
    pub indicator: El,
    pub state: State,
    pub disabled: bool,
    pub invalid: bool,
    pub label: Option<String>,
    pub id: String,
    pub bind: Option<Bind>,
    /// `transition-colors` (the checkbox has it, the radio item doesn't).
    pub transition: bool,
}

pub(crate) fn boxed(th: &Theme, b: Boxed) -> El {
    let t = &th.t;
    let r = if b.round { radius::FULL } else { 4.0 };
    let enabled = !b.disabled;
    let on = b.checked;
    let fill = if on { t.primary } else if th.dark { Theme::tint(t.input, 0.3) } else { Color::TRANSPARENT };
    let mut border = if on { t.primary } else { t.input };
    let shadows = if on { &t.elev_raised } else { &t.elev_well };
    let mut e = row().role(b.role).size(16.0, 16.0).center().radius(r).fill(fill).checked(on);
    e = super::button::shadows(e, shadows, 3);
    if b.transition {
        e = e.motion(tw_spring());
    } else {
        e = e.motion(Spring::NONE);
    }
    if on {
        e = e.child(layer(0.0).radius(r).sheen(true).key("sheen"));
    }
    if let Some(l) = &b.label {
        e = e.label(l.clone());
    }
    let invalid = b.invalid && enabled;
    let ring = Theme::tint(t.ring, 0.5);
    if invalid {
        let (ib, ir) = invalid_colors(th);
        if !on {
            border = ib;
        }
        e = e.outline(3.0, 0.0, ir).no_focus_ring();
    } else if enabled && b.state == State::Focus {
        if !on {
            border = t.ring;
        }
        e = e.outline(3.0, 0.0, ring);
    } else if enabled {
        e = e.focus_ring(3.0, 0.0, ring);
        if !on {
            e = e.child(layer(0.0).radius(r).focus_ring(1.0, -1.0, t.ring).key("focus-border"));
        }
    }
    e = e.border(1.0, border);
    if on {
        e = e.child(b.indicator.key("indicator").sequence(Sequence::new(CHECK_IN)));
    }
    if !enabled {
        return e.disabled(true).layer_opacity(0.5);
    }
    e = e.focusable(true);
    let Some(bind) = b.bind else { return e };
    let hit = rune::el().key("hit").pin(Some(-12.0), Some(-8.0), Some(-12.0), Some(-8.0)).a11y_hidden();
    let e = e.child(track_press(bind(hit), b.id.clone()));
    track_press(bind(e), b.id)
}

/// `<Checkbox checked onCheckedChange />`.
pub struct Checkbox {
    checked: bool,
    state: State,
    disabled: bool,
    invalid: bool,
    label: Option<String>,
    id: String,
    bind: Option<Bind>,
}

impl Checkbox {
    #[track_caller]
    pub fn new(checked: bool) -> Checkbox {
        Checkbox { checked, state: State::Rest, disabled: false, invalid: false, label: None, id: site_id(std::panic::Location::caller(), "checkbox"), bind: None }
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
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    /// `onCheckedChange`: the new value is `!checked`.
    pub fn on_toggle<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.bind = Some(bind(f));
        self
    }
    pub fn on_toggle_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.bind = Some(bind_tx(f));
        self
    }
    pub fn build(self, th: &Theme) -> El {
        let indicator = icon(&icons::CHECK, 14.0).color(th.t.primary_foreground).a11y_hidden();
        boxed(
            th,
            Boxed {
                role: "checkbox",
                round: false,
                checked: self.checked,
                indicator,
                state: self.state,
                disabled: self.disabled,
                invalid: self.invalid,
                label: self.label,
                id: self.id,
                bind: self.bind,
                transition: true,
            },
        )
    }
}
