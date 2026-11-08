//! `components/ui/input.tsx`: a single-line text field on Rune's own editor, so it keeps native
//! editing (IME, undo, selection, copy and paste, secure text).
//!
//! A text field matches `:focus-visible` whenever it has focus, from the pointer too, so its focus
//! look follows [`TextEdit::focused`].

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{row, text_input, Color, El, KeyEvent, Shadow, TextEdit, TextStyle, Tx};

use crate::ui::theme::{radius, text as ty, Theme};

use super::button::{invalid_colors, tw_spring, State};

/// Edit bindings for the inner editor, attached only when enabled.
#[derive(Default)]
pub(crate) struct EditActs(Vec<Box<dyn FnOnce(El) -> El>>);

impl EditActs {
    pub(crate) fn push(&mut self, f: impl FnOnce(El) -> El + 'static) {
        self.0.push(Box::new(f));
    }
    pub(crate) fn apply(self, mut e: El, enabled: bool) -> El {
        if enabled {
            for f in self.0 {
                e = f(e);
            }
        }
        e
    }
}

/// The field shell's look: fill, inset shadows, border and outer ring, for a state.
pub(crate) struct Field {
    pub fill: Color,
    pub shadows: Vec<Shadow>,
    pub border: Color,
    pub ring: Option<Color>,
}

/// `bg-foreground/[0.025] shadow-well … focus-visible:border-ring focus-visible:bg-transparent
/// focus-visible:shadow-none focus-visible:ring-3 focus-visible:ring-ring/50 disabled:bg-input/50
/// aria-invalid:border-destructive aria-invalid:ring-3 …` (`dark:bg-black/15`, `dark:disabled:bg-input/80`).
/// `clear_on_focus`: whether focus also clears the fill (Input and Textarea; not InputGroup).
pub(crate) fn field(th: &Theme, focused: bool, disabled: bool, invalid: bool, clear_on_focus: bool) -> Field {
    let t = &th.t;
    let rest = if th.dark { Color::BLACK.alpha(0.15) } else { Theme::tint(t.foreground, 0.025) };
    if disabled {
        let fill = Theme::tint(t.input, if th.dark { 0.8 } else { 0.5 });
        return Field { fill, shadows: t.elev_well.clone(), border: Color::TRANSPARENT, ring: None };
    }
    let mut f = Field { fill: rest, shadows: t.elev_well.clone(), border: Color::TRANSPARENT, ring: None };
    if focused {
        if clear_on_focus {
            f.fill = Color::TRANSPARENT;
        }
        f.shadows = Vec::new();
        f.border = t.ring;
        f.ring = Some(Theme::tint(t.ring, 0.5));
    }
    if invalid {
        let (b, r) = invalid_colors(th);
        f.border = b;
        f.ring = Some(r);
    }
    f
}

/// Paints `f` on a field's box (radius `r`), keeping three shadow layers so focus springs
/// between them.
pub(crate) fn shell(e: El, f: &Field, r: f32) -> El {
    let n = f.shadows.len().max(3);
    let e = super::button::shadows(e.radius(r).fill(f.fill).border(1.0, f.border), &f.shadows, n).motion(tw_spring());
    match f.ring {
        Some(c) => e.outline(3.0, 0.0, c),
        None => e,
    }
}

/// `<Input />`.
pub struct Input {
    pub(crate) edit: TextEdit,
    pub(crate) placeholder: String,
    pub(crate) label: Option<String>,
    pub(crate) font: TextStyle,
    pub(crate) color: Option<Color>,
    pub(crate) height: f32,
    pub(crate) width: Option<f32>,
    pub(crate) state: State,
    pub(crate) disabled: bool,
    pub(crate) invalid: bool,
    pub(crate) acts: EditActs,
}

impl Input {
    pub fn new(edit: &TextEdit) -> Input {
        Input {
            edit: edit.clone(),
            placeholder: String::new(),
            label: None,
            font: ty::SM,
            color: None,
            height: 32.0,
            width: None,
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
    /// The accessible name when no visible label says it (`aria-label`); the placeholder otherwise.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// The text style (`text-sm` by default; `font-mono text-[13px]` for a trigger field).
    pub fn font(mut self, f: TextStyle) -> Self {
        self.font = f;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// `h-*` (32 by default).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    /// A fixed width instead of `w-full`.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    /// Password-style text.
    pub fn secure(mut self) -> Self {
        self.edit.set_secure(true);
        self.acts.push(|e| e.secure());
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
    /// `aria-invalid`.
    pub fn invalid(mut self, i: bool) -> Self {
        self.invalid = i;
        self
    }
    /// Binds the field to its `TextEdit` in the app's state.
    pub fn on_edit<S: 'static>(mut self, f: impl for<'a> Fn(&'a mut S) -> &'a mut TextEdit + 'static) -> Self {
        self.acts.push(move |e| e.on_edit(f));
        self
    }
    /// Return.
    pub fn on_submit<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_submit(f));
        self
    }
    /// Keys before the editor sees them (`onKeyDown`); true takes the key.
    pub fn on_key<S: 'static>(mut self, f: impl Fn(&mut S, &KeyEvent, &mut Tx) -> bool + 'static) -> Self {
        self.acts.push(move |e| e.on_input_key(f));
        self
    }
    /// After the text changes (`onChange`).
    pub fn on_change<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_input_change(f));
        self
    }

    /// The bare editor, styled for this field (for InputGroup, which draws the box itself).
    pub(crate) fn editor(self, th: &Theme) -> El {
        let t = &th.t;
        let ink = self.color.unwrap_or(t.foreground);
        let mut e = text_input(&self.edit, self.font)
            .single_line()
            .key("input")
            .grow(1.0)
            .shrink()
            .min_w(0.0)
            .color(ink)
            .placeholder(self.placeholder.clone(), t.muted_foreground)
            .caret(ink, t.iris_soft);
        let name = self.label.clone().or_else(|| Some(self.placeholder.clone()).filter(|p| !p.is_empty()));
        if let Some(n) = name {
            e = e.label(n);
        }
        e = e.invalid(self.invalid).disabled(self.disabled);
        self.acts.apply(e, !self.disabled)
    }

    pub fn build(self, th: &Theme) -> El {
        let focused = (self.edit.focused || self.state == State::Focus) && !self.disabled;
        let f = field(th, focused, self.disabled, self.invalid, true);
        let (h, w, disabled) = (self.height, self.width, self.disabled);
        // `px-2.5` inside the 1 pt border; `py-1` is moot at a fixed height.
        let mut e = row().role("textbox").h(h).px(11.0).center_y().child(self.editor(th));
        e = match w {
            Some(w) => e.w(w),
            None => e.w_full().min_w(0.0),
        };
        e = shell(e, &f, radius::LG);
        if disabled { e.layer_opacity(0.5) } else { e }
    }
}
