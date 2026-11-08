//! `components/app/segmented.tsx`: a radio group drawn as a segmented control. The selected
//! option sits on a raised [`SelectionPill`] that glides between options; arrow keys move the
//! choice (one Tab stop for the group).
//!
//! ```ignore
//! Segmented::new("theme", "Appearance").option("dark", "Dark").option("light", "Light")
//!     .selected(s.theme.as_str()).on_change(|s: &mut MainUi, v: &str| s.set_theme(v)).build(th, cx)
//! ```

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::rc::Rc;

use rune::{row, text, Cx, El, TextStyle, Tx};

use crate::ui::theme::{radius, weight, Theme, Utilities};

use super::button::{hover_fast, ink, Look, State};
use super::selection_pill::SelectionPill;

/// `size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SegmentedSize {
    /// `h-6 text-[11px]`.
    Sm,
    /// `h-7 text-xs`.
    #[default]
    Md,
}

struct Opt {
    value: String,
    label: String,
    description: Option<String>,
}

type Change = Rc<dyn Fn(El, String) -> El>;

/// `<Segmented value options onChange label size />`.
pub struct Segmented {
    id: String,
    label: String,
    options: Vec<Opt>,
    selected: Option<String>,
    size: SegmentedSize,
    hover: Option<String>,
    change: Option<Change>,
}

impl Segmented {
    /// `id` keys the group (unique in the window); `label` names the radio group.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Segmented {
        Segmented { id: id.into(), label: label.into(), options: Vec::new(), selected: None, size: SegmentedSize::Md, hover: None, change: None }
    }
    pub fn option(mut self, value: impl Into<String>, label: impl Into<String>) -> Self {
        self.options.push(Opt { value: value.into(), label: label.into(), description: None });
        self
    }
    /// An option with a `description` (its `title` tooltip).
    pub fn option_with(mut self, value: impl Into<String>, label: impl Into<String>, description: impl Into<String>) -> Self {
        self.options.push(Opt { value: value.into(), label: label.into(), description: Some(description.into()) });
        self
    }
    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.selected = Some(value.into());
        self
    }
    pub fn size(mut self, s: SegmentedSize) -> Self {
        self.size = s;
        self
    }
    /// Forces the hover look on one option (galleries).
    pub fn hover(mut self, value: impl Into<String>) -> Self {
        self.hover = Some(value.into());
        self
    }
    /// `onChange(value)`.
    pub fn on_change<S: 'static>(mut self, f: impl Fn(&mut S, &str) + 'static) -> Self {
        let f = Rc::new(f);
        self.change = Some(Rc::new(move |e: El, v: String| {
            let f = f.clone();
            e.on_press(move |s: &mut S| f(s, &v))
        }));
        self
    }
    pub fn on_change_tx<S: 'static>(mut self, f: impl Fn(&mut S, &str, &mut Tx) + 'static) -> Self {
        let f = Rc::new(f);
        self.change = Some(Rc::new(move |e: El, v: String| {
            let f = f.clone();
            e.on_press_tx(move |s: &mut S, tx: &mut Tx| f(s, &v, tx))
        }));
        self
    }

    /// The key of the option `value` (for anchoring or tests).
    pub fn option_key(id: &str, value: &str) -> String {
        format!("{id}:{value}")
    }

    pub fn build<S>(self, th: &Theme, cx: &Cx<S>) -> El {
        let t = &th.t;
        let (h, font) = match self.size {
            SegmentedSize::Md => (28.0, TextStyle::new(12.0, weight::MEDIUM).lh(16.0)),
            SegmentedSize::Sm => (24.0, TextStyle::new(11.0, weight::MEDIUM).lh(16.0)),
        };
        let selected_key = self.selected.as_ref().filter(|v| self.options.iter().any(|o| &o.value == *v)).map(|v| Self::option_key(&self.id, v));
        let pill = SelectionPill::new(self.id.clone(), selected_key.clone()).radius(radius::MD).fill(t.surface_raised).shadows(&t.elev_soft).sheen(false).build(cx);
        let mut e = row()
            .key(self.id.clone())
            .role("radiogroup")
            .label(self.label.clone())
            .padding(2.0)
            .radius(radius::LG)
            .fill(Theme::tint(t.foreground, 0.04))
            .elevate(&t.elev_well)
            .focus_group()
            .child(pill);
        for o in self.options {
            let on = self.selected.as_deref() == Some(o.value.as_str());
            let forced = self.hover.as_deref() == Some(o.value.as_str());
            let look = Look::of(if forced { State::Hover } else { State::Rest }, false, true);
            let rest = if on { t.foreground } else { t.muted_foreground };
            let mut b = hover_fast(row())
                .key(Self::option_key(&self.id, &o.value))
                .role("radio")
                .label(o.label.clone())
                .checked(on)
                .focus_selected(on)
                .h(h)
                .px(10.0)
                .center()
                .radius(radius::MD)
                .child(ink(text(o.label).font(font), rest, (!on).then_some(t.foreground), look).a11y_hidden())
                .focus_ring(2.0, 1.0, t.ring)
                .press_scale(0.97);
            if let Some(d) = o.description {
                b = b.help(d);
            }
            if let Some(c) = &self.change {
                b = c(b, o.value);
            }
            e = e.child(b);
        }
        e
    }
}
