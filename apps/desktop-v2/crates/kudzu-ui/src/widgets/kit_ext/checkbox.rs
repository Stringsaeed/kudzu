//! RUNE-GAP(#573: rune-kit `Checkbox` takes its checked fill, mark and depth from `Kit`): delete
//! this module and use `rune_kit::Checkbox` once it does.
//!
//! shadcn's `Checkbox` (`components/ui/checkbox.tsx`) beside its label, as the clipboard's
//! "Also delete pinned items" row draws it (`label.flex.items-center.gap-2.5.text-[13px]`).
//! Verdict in `PARITY_MATRIX.md`: **wrapper** (D21). rune-kit's box is an inner element of its
//! row, filled from `surface_weak` with no depth, so the checked look (`bg-primary
//! border-primary text-primary-foreground shadow-raised sheen-strong`) cannot be decorated on;
//! this builds the same control from Rune core: a 16 point box with 4 point corners, `border
//! border-input shadow-well` (`dark:bg-input/30`) when off, the primary face with the `raised`
//! depth and the strong sheen and a 14 point check when on, and at focus a ring-coloured border
//! with the 3 point `ring/50` halo. The row is the control: role checkbox, named by its label.

use rune::{el, icon, row, text, Color, El, Shadow, Tx};
use rune_kit::State;

use crate::theme::{icons, motion, palette::fixed, radius, Theme};

/// `size-4`.
pub const SIDE: f32 = 16.0;

type Attach = Box<dyn FnOnce(El) -> El>;

pub struct Checkbox {
    checked: bool,
    label: Option<String>,
    show_label: bool,
    state: State,
    attach: Vec<Attach>,
}

impl Checkbox {
    pub fn new(checked: bool) -> Checkbox {
        Checkbox {
            checked,
            label: None,
            show_label: true,
            state: State::Rest,
            attach: Vec::new(),
        }
    }
    /// The label beside the box, and the control's accessible name.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// Names the control without drawing the label (the row draws its own text).
    pub fn hide_label(mut self) -> Self {
        self.show_label = false;
        self
    }
    /// A forced visual state (galleries).
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
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
        let disabled = self.state == State::Disabled;
        let focused = self.state == State::Focus;
        let on = self.checked;
        let border = Shadow {
            spread: 1.0,
            color: if focused && !on {
                c.ring
            } else if on {
                c.primary
            } else {
                c.input
            },
            inset: true,
            ..Shadow::default()
        };
        let mut b = el()
            .key("box")
            .size(SIDE, SIDE)
            .center()
            .radius(radius::CHECKBOX)
            .token_motion(motion::soft_spring(motion::FAST))
            .a11y_hidden();
        if on {
            let [(_, top), (_, bottom)] = fixed::SHEEN_STRONG;
            b = b.fill(c.primary).gradient(top.color(), bottom.color());
            for s in &theme.depth.raised {
                b = b.box_shadow(*s);
            }
            b = b.child(
                icon(&icons::CHECK, 14.0)
                    .key("mark")
                    .color(c.primary_foreground)
                    .transition(crate::motion::check_in()),
            );
        } else {
            b = b.fill(if theme.dark {
                c.input.alpha(c.input.a * 0.3)
            } else {
                Color::TRANSPARENT
            });
            for s in &theme.depth.well {
                b = b.box_shadow(*s);
            }
        }
        b = b.box_shadow(border);
        if focused {
            b = b.outline(3.0, 0.0, c.ring.alpha(0.5));
        }
        let mut e = row()
            .role("checkbox")
            .checked(on)
            .gap(10.0)
            .center_y()
            .focusable(!disabled)
            .disabled(disabled)
            .focus_ring(3.0, 0.0, c.ring.alpha(0.5))
            .child(b);
        if let Some(l) = self.label {
            if self.show_label {
                e = e.child(
                    text(l.clone())
                        .key("label")
                        .font(theme.ty.body)
                        .color(c.foreground)
                        .a11y_hidden(),
                );
            }
            e = e.label(l);
        }
        if disabled {
            return e.layer_opacity(0.5);
        }
        self.attach.into_iter().fold(e, |e, f| f(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Runtime};

    fn view(on: &bool, _: &mut Cx<bool>) -> El {
        row().child(
            Checkbox::new(*on)
                .label("Also delete pinned items")
                .on_toggle(|s: &mut bool| *s = !*s)
                .build(Theme::light())
                .key("check"),
        )
    }

    #[test]
    fn the_row_is_the_checkbox_and_toggles() {
        let mut rt = Runtime::new(false, view, (300.0, 40.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let t = rt.accessibility_json().to_string();
        assert!(t.contains("Also delete pinned items"), "{t}");
        assert!(
            t.contains("\"check-box\"") && t.contains("unchecked"),
            "{t}"
        );
        rune::motion_gate::Run::tap(&mut rt, "check");
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
        assert!(rt.state);
        let els = rt.elements();
        let b = els
            .iter()
            .find(|e| e.key.as_deref() == Some("box"))
            .unwrap();
        assert_eq!((b.rect[2], b.rect[3]), (SIDE, SIDE));
        assert_eq!(b.fill, Theme::light().c.primary);
    }
}
