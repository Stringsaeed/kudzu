//! RUNE-GAP(#573, gap 17: rune-kit `Switch` sizes from `Kit`): delete this module and use
//! `rune_kit::Switch` once its sizes are tokens.
//!
//! shadcn's `Switch` (`components/ui/switch.tsx`), built from Rune core. Verdict in
//! `PARITY_MATRIX.md`: **wrapper**. rune-kit's switch is an `NSSwitch` (36 × 16 and up, a pill
//! knob 21 wide) or the OpenCode web look (24 × 16, 4 point corners), and its track and knob are
//! inner elements a decoration cannot resize. Kudzu's are 32 × 18.4 with a 16 point knob
//! (`default`) and 24 × 14 with a 12 point knob (`sm`), capsule-round, 1 point in from the edge.
//!
//! Not reachable: the knob's live stretch while pressed (`group-active/switch:scale-x-[1.2]`
//! from the leading or trailing edge). Rune draws a pressed overlay (`El::press`) and a uniform
//! press scale about the centre, nothing that moves a child's geometry while its parent is held;
//! the stretch shows only for a forced `State::Pressed` (galleries).

use rune::{el, Color, El, Shadow, Tx};
use rune_kit::State;

use crate::theme::{motion, Theme};

/// shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    #[default]
    Default,
    Sm,
}

impl Size {
    /// Track width and height, knob side (`w-[32px] h-[18.4px]`, `size-4`; `w-[24px] h-[14px]`,
    /// `size-3`).
    pub fn metrics(self) -> (f32, f32, f32) {
        match self {
            Size::Default => (32.0, 18.4, 16.0),
            Size::Sm => (24.0, 14.0, 12.0),
        }
    }
}

/// The transparent border the knob sits inside.
const INSET: f32 = 1.0;
/// `group-active/switch:scale-x-[1.2]`.
const STRETCH: f32 = 1.2;

type Attach = Box<dyn FnOnce(El) -> El>;

/// `Switch::new(on).label("Open at login").on_toggle(|s: &mut S| s.login ^= true).build(theme)`.
pub struct Switch {
    checked: bool,
    size: Size,
    label: Option<String>,
    state: State,
    attach: Vec<Attach>,
}

impl Switch {
    pub fn new(checked: bool) -> Switch {
        Switch {
            checked,
            size: Size::Default,
            label: None,
            state: State::Rest,
            attach: Vec::new(),
        }
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    /// The accessible name; the visible label is the row's own text, as in the Tauri app.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
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
    pub fn on_toggle<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.attach.push(Box::new(move |e| e.on_press(f)));
        self
    }
    pub fn on_toggle_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.attach.push(Box::new(move |e| e.on_press_tx(f)));
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        let c = &theme.c;
        let (w, h, side) = self.size.metrics();
        let disabled = self.state == State::Disabled;
        let track = if self.checked {
            c.iris
        } else if theme.dark {
            c.input.alpha(c.input.a * 0.8)
        } else {
            c.input
        };
        let knob_fill = match (theme.dark, self.checked) {
            (false, _) => c.background,
            (true, true) => Color::WHITE,
            (true, false) => c.foreground.alpha(0.9),
        };
        let knob_w = if self.state == State::Pressed {
            side * STRETCH
        } else {
            side
        };
        // Checked, the knob sits `100% - 2px` along: flush with the far inset.
        let x = if self.checked {
            w - INSET - knob_w
        } else {
            INSET
        };
        let knob = theme.depth.knob.iter().fold(
            el().key("thumb")
                .size(knob_w, side)
                .absolute(x, (h - side) / 2.0)
                .radius(side / 2.0)
                .fill(knob_fill)
                .motion(motion::SOFT),
            |k, s| k.box_shadow(*s),
        );
        let shadows: Vec<Shadow> = if self.checked {
            vec![Shadow {
                y: 1.0,
                blur: 2.0,
                color: Color::BLACK.alpha(0.18),
                inset: true,
                ..Shadow::default()
            }]
        } else {
            theme.depth.well.clone()
        };
        let mut e = el()
            .role("switch")
            .checked(self.checked)
            .size(w, h)
            .radius(h / 2.0)
            .fill(track)
            .token_motion(motion::soft_spring(motion::MODERATE))
            .focusable(!disabled)
            .focus_ring(3.0, 0.0, c.ring.alpha(0.5))
            .disabled(disabled)
            .child(knob);
        for s in shadows {
            e = e.box_shadow(s);
        }
        if let Some(l) = self.label {
            e = e.label(l);
        }
        match self.state {
            State::Focus => e = e.outline(3.0, 0.0, c.ring.alpha(0.5)),
            State::Disabled => e = e.layer_opacity(0.5),
            _ => {}
        }
        if !disabled {
            for f in self.attach {
                e = f(e);
            }
        }
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_the_shadcn_ones() {
        assert_eq!(Size::Default.metrics(), (32.0, 18.4, 16.0));
        assert_eq!(Size::Sm.metrics(), (24.0, 14.0, 12.0));
    }

    #[test]
    fn checked_knob_lands_at_the_far_inset() {
        // translate-x-[calc(100%-2px)] from the 1 point inset: 1 + 16 - 2 = 15, so it ends at 31.
        let (w, _, side) = Size::Default.metrics();
        assert_eq!(w - INSET - side, INSET + side - 2.0);
        let (w, _, side) = Size::Sm.metrics();
        assert_eq!(w - INSET - side, INSET + side - 2.0);
    }
}
