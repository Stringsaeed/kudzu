//! `components/ui/textarea.tsx`: a multi-line field on Rune's editor that grows with its text
//! (`field-sizing-content`), at least 64 pt tall.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{col, icon, text_input, Color, Drag, El, Icon, KeyEvent, Phase, TextEdit, TextStyle, Tx};

use crate::ui::theme::{radius, text as ty, Theme};

use super::button::State;
use super::input::{field, shell, EditActs};

/// The resize grip a `<textarea>` draws in its corner (Chromium's two diagonal strokes).
static GRIP: Icon = Icon {
    name: "kudzu-textarea-grip",
    view: 9.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round"><path d="M8.5 1.5 1.5 8.5"/><path d="M8.5 5 5 8.5"/></g>"#,
};

/// Attaches the grip's drag: (grip, minimum height, height at the press).
type Resize = Box<dyn FnOnce(El, f32, f32) -> El>;

/// `<Textarea />`. The corner grip resizes it vertically when the caller owns its height
/// ([`Textarea::height`] and [`Textarea::on_resize`]).
pub struct Textarea {
    pub(crate) edit: TextEdit,
    pub(crate) placeholder: String,
    pub(crate) label: Option<String>,
    pub(crate) font: TextStyle,
    pub(crate) color: Option<Color>,
    pub(crate) min_height: f32,
    pub(crate) max_height: Option<f32>,
    pub(crate) height: Option<f32>,
    pub(crate) resize: Option<Resize>,
    pub(crate) grip: bool,
    pub(crate) state: State,
    pub(crate) disabled: bool,
    pub(crate) invalid: bool,
    pub(crate) acts: EditActs,
}

impl Textarea {
    pub fn new(edit: &TextEdit) -> Textarea {
        Textarea {
            edit: edit.clone(),
            placeholder: String::new(),
            label: None,
            font: ty::SM,
            color: None,
            min_height: 64.0,
            max_height: None,
            height: None,
            resize: None,
            grip: true,
            state: State::Rest,
            disabled: false,
            invalid: false,
            acts: EditActs::default(),
        }
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    pub fn font(mut self, f: TextStyle) -> Self {
        self.font = f;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// `min-h-*` (64 by default).
    pub fn min_height(mut self, h: f32) -> Self {
        self.min_height = h;
        self
    }
    /// `max-h-*`: past it the text scrolls.
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_height = Some(h);
        self
    }
    /// A height the user set by dragging the grip (`None`: it follows its text).
    pub fn height(mut self, h: Option<f32>) -> Self {
        self.height = h;
        self
    }
    /// The grip was dragged: the new height, at least the minimum.
    pub fn on_resize<S: 'static>(mut self, f: impl Fn(&mut S, f32) + 'static) -> Self {
        self.resize = Some(Box::new(move |e: El, min: f32, start: f32| {
            e.on_drag(move |s: &mut S, d: &mut Drag, _tx: &mut Tx| match d.phase {
                Phase::Down => d.data = start,
                _ if d.cancelled() => {}
                _ => f(s, (d.data + d.dy).max(min)),
            })
        }));
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
    pub fn on_edit<S: 'static>(mut self, f: impl for<'a> Fn(&'a mut S) -> &'a mut TextEdit + 'static) -> Self {
        self.acts.push(move |e| e.on_edit(f));
        self
    }
    /// ⌘Return (Return types a newline).
    pub fn on_submit<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_submit(f));
        self
    }
    pub fn on_key<S: 'static>(mut self, f: impl Fn(&mut S, &KeyEvent, &mut Tx) -> bool + 'static) -> Self {
        self.acts.push(move |e| e.on_input_key(f));
        self
    }
    pub fn on_change<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_input_change(f));
        self
    }

    pub(crate) fn editor(self, th: &Theme) -> El {
        let t = &th.t;
        let ink = self.color.unwrap_or(t.foreground);
        let mut e = text_input(&self.edit, self.font)
            .key("input")
            .w_full()
            .color(ink)
            .placeholder(self.placeholder.clone(), t.muted_foreground)
            .caret(ink, t.iris_soft);
        if let Some(n) = self.label.clone().or_else(|| Some(self.placeholder.clone()).filter(|p| !p.is_empty())) {
            e = e.label(n);
        }
        e = e.invalid(self.invalid).disabled(self.disabled);
        let e = self.acts.apply(e, !self.disabled);
        match self.max_height {
            Some(h) => col().key("scroll").w_full().shrink().max_h(h - 18.0).scroll(rune::ScrollSpec { stick_bottom: false }).child(e.animate_edit_layout()),
            None => e,
        }
    }

    pub fn build(mut self, th: &Theme) -> El {
        let focused = (self.edit.focused || self.state == State::Focus) && !self.disabled;
        let f = field(th, focused, self.disabled, self.invalid, true);
        let (min_h, disabled, height, grip) = (self.min_height, self.disabled, self.height, self.grip);
        let resize = self.resize.take();
        // `px-2.5 py-2` inside the 1 pt border.
        let mut e = col().role("textbox").w_full().min_h(min_h).px(11.0).py(9.0).child(self.editor(th));
        if let Some(h) = height {
            e = e.h(h.max(min_h)).clip_rounded(radius::LG);
        }
        let mut e = shell(e, &f, radius::LG);
        if grip {
            let mut g = rune::el().key("grip").pin(None, None, Some(2.0), Some(2.0)).size(9.0, 9.0).child(icon(&GRIP, 9.0).color(th.t.muted_foreground.alpha(0.6))).a11y_hidden();
            if let (Some(r), false) = (resize, disabled) {
                // The gesture keeps its starting height in `Drag::data`.
                let start = height.unwrap_or(min_h);
                g = r(g, min_h, start).cursor(rune::Cursor::ResizeRow);
            }
            e = e.child(g);
        }
        if disabled { e.layer_opacity(0.5) } else { e }
    }
}
