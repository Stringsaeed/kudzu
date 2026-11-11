//! A whole-number field that keeps its value inside `min..=max` (`Stepper` in
//! `components/app/editor/tag-tools.tsx`: `<Input type="number" className="h-8 tabular">`),
//! for the key count, the pause length and the clipboard history depth. It is the decorated
//! input ([`Input`]) with tabular figures, a `spinbutton` value for assistive technology, and
//! the number field's keys: Up and Down move one step. Typing keeps what parses and clamps it;
//! anything else leaves the value as it was, as the browser's number input does.

use rune::{El, Key, TextEdit, TextStyle};

use crate::theme::Theme;
use crate::widgets::kit_ext::input::Input;

/// What the typed text means: a whole number clamped to `min..=max`, or nothing.
pub fn parse(text: &str, min: i64, max: i64) -> Option<i64> {
    let n: f64 = text.trim().parse().ok()?;
    n.is_finite().then(|| (n.floor() as i64).clamp(min, max))
}

/// `value` moved `d` steps, clamped.
pub fn nudge(value: i64, d: i64, step: i64, min: i64, max: i64) -> i64 {
    (value + d * step).clamp(min, max)
}

type Lens<S> = fn(&mut S) -> &mut TextEdit;

pub struct Stepper<S> {
    edit_text: String,
    focused: bool,
    lens: Lens<S>,
    label: String,
    value: i64,
    min: i64,
    max: i64,
    step: i64,
    width: f32,
}

impl<S: 'static> Stepper<S> {
    /// `lens` finds the field's `TextEdit` in the state; `value` is the current number.
    pub fn new(edit: &TextEdit, lens: Lens<S>, value: i64, min: i64, max: i64) -> Self {
        Stepper {
            edit_text: edit.text.clone(),
            focused: edit.focused,
            lens,
            label: String::new(),
            value,
            min,
            max,
            step: 1,
            width: 96.0,
        }
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }
    pub fn step(mut self, s: i64) -> Self {
        self.step = s.max(1);
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }

    /// `on_change` receives each new value: typed (clamped) or stepped.
    pub fn build(self, theme: &Theme, on_change: impl Fn(&mut S, i64) + Clone + 'static) -> El {
        let Stepper {
            lens,
            min,
            max,
            step,
            value,
            ..
        } = self;
        let (typed, stepped) = (on_change.clone(), on_change);
        let mut edit = TextEdit::new(self.edit_text);
        edit.focused = self.focused;
        let style = TextStyle {
            features: "'tnum' 1",
            ..theme.ty.input
        };
        Input::new(&edit)
            .text_style(style)
            .with(move |t| {
                t.label(self.label)
                    .width(Some(self.width))
                    .on_edit(lens)
                    .on_change(move |s: &mut S, _| {
                        if let Some(n) = parse(&lens(s).text, min, max) {
                            typed(s, n);
                        }
                    })
                    .on_key(move |s: &mut S, e, tx| {
                        let d = match e.key {
                            Key::Up => 1,
                            Key::Down => -1,
                            _ => return false,
                        };
                        let current = parse(&lens(s).text, min, max).unwrap_or(value);
                        let n = nudge(current, d, step, min, max);
                        let text = n.to_string();
                        let field = lens(s);
                        field.replace_text(&text);
                        stepped(s, n);
                        tx.instant();
                        true
                    })
            })
            .build(theme)
            .role("spinbutton")
            .range_value(value as f64, min as f64, max as f64)
            .range_step(step as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_keeps_what_parses_and_clamps_it() {
        assert_eq!(parse("12", 1, 50), Some(12));
        assert_eq!(parse(" 7.9 ", 1, 50), Some(7));
        assert_eq!(parse("120", 1, 50), Some(50));
        assert_eq!(parse("0", 1, 50), Some(1));
        assert_eq!(parse("abc", 1, 50), None);
        assert_eq!(parse("", 1, 50), None);
    }

    #[test]
    fn steps_clamp_to_the_range() {
        assert_eq!(nudge(300, 1, 50, 1, 5000), 350);
        assert_eq!(nudge(4990, 1, 50, 1, 5000), 5000);
        assert_eq!(nudge(1, -1, 1, 1, 50), 1);
    }
}
