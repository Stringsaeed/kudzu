//! RUNE-GAP(gap 17, issue proposed in S06: "rune-kit `TextArea`: take the text style from
//! `Kit`"): delete this module and use `rune_kit::TextArea` once it takes a text style and
//! padding, as `TextInput::text_style` does.
//!
//! shadcn's `Textarea` (`components/ui/textarea.tsx`), built from Rune core's `text_input`.
//! Verdict in `PARITY_MATRIX.md`: **wrapper**. rune-kit's multi-line field sets its text in
//! `ty.ui` on a 1.35 line (13/17.5 with Kudzu's kit) inside 8 points of padding and is at least
//! 80 points tall; its editor is an inner element a decoration cannot restyle. Kudzu's is 14/20
//! (`md:text-sm`), `px-2.5 py-2` inside the transparent border, at least 64 tall (`min-h-16`),
//! growing with its content (`field-sizing-content`). The surface is the input's (`well`).

use rune::{col, text_input, El, TextEdit, TextStyle, Tx};
use rune_kit::State;

use super::input::well;
use crate::theme::{radius, Theme};

/// `min-h-16`.
pub const MIN_HEIGHT: f32 = 64.0;
/// `px-2.5` and `py-2`, each plus the 1 point border.
pub const PADDING: (f32, f32) = (11.0, 9.0);

type Attach = Box<dyn FnOnce(El) -> El>;

/// `Textarea::new(&s.note).placeholder("Notes").on_edit(|s: &mut S| &mut s.note).build(theme)`.
pub struct Textarea {
    edit: TextEdit,
    placeholder: String,
    label: Option<String>,
    state: State,
    invalid: bool,
    min_height: f32,
    style: Option<TextStyle>,
    attach: Vec<Attach>,
}

impl Textarea {
    pub fn new(edit: &TextEdit) -> Textarea {
        Textarea {
            edit: edit.clone(),
            placeholder: String::new(),
            label: None,
            state: State::Rest,
            invalid: false,
            min_height: MIN_HEIGHT,
            style: None,
            attach: Vec::new(),
        }
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// The accessible name, when no visible label says it; else the placeholder names it.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// A taller floor (`min-h-20`, `min-h-24` on the chip forms).
    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = h;
        self
    }
    /// The text's type: `Theme::ty.input` (14/20) unless set (`font-mono` on the chip forms).
    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn invalid(mut self, on: bool) -> Self {
        self.invalid = on;
        self
    }
    pub fn on_edit<S: 'static>(
        mut self,
        f: impl for<'a> Fn(&'a mut S) -> &'a mut TextEdit + 'static,
    ) -> Self {
        self.attach.push(Box::new(move |e| e.on_edit(f)));
        self
    }
    pub fn on_submit<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.attach.push(Box::new(move |e| e.on_submit(f)));
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        let c = &theme.c;
        let disabled = self.state == State::Disabled;
        let name = self
            .label
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| self.placeholder.clone());
        let mut field = text_input(&self.edit, self.style.unwrap_or(theme.ty.input))
            .key("input")
            .w_full()
            .color(c.foreground)
            .placeholder(self.placeholder, c.muted_foreground)
            .caret(c.foreground, theme.kit().selection())
            .disabled(disabled);
        if !name.is_empty() {
            field = field.label(name);
        }
        if !disabled {
            for f in self.attach {
                field = f(field);
            }
        }
        // The box carries no name: the editor inside does, as rune-kit's fields do.
        let e = col()
            .role("textbox")
            .w_full()
            .min_h(self.min_height)
            .px(PADDING.0)
            .py(PADDING.1)
            .radius(radius::LG)
            .child(field);
        let focused = (self.edit.focused || self.state == State::Focus) && !disabled;
        let e = if focused || self.invalid || disabled {
            e
        } else {
            theme.depth.well.iter().fold(e, |e, s| e.box_shadow(*s))
        };
        let e = well(theme, e, focused, self.invalid, disabled);
        if disabled {
            e.layer_opacity(0.5)
        } else {
            e
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_are_the_shadcn_classes() {
        assert_eq!(MIN_HEIGHT, 64.0);
        assert_eq!(PADDING, (11.0, 9.0));
        let ty = Theme::light().ty.input;
        assert_eq!((ty.size, ty.line_height), (14.0, 20.0));
    }
}
