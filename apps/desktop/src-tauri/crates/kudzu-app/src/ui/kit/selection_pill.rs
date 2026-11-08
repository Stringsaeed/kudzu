//! `components/app/selection-pill.tsx`: one background for a group's selected option, gliding
//! from option to option on `--spring-soft` over `--motion-slow`. It reads where the group and the
//! selected option were laid out and sits behind the options; with nothing selected it fades out,
//! and it fades in where it belongs (it doesn't slide in from elsewhere).
//!
//! Put it first among the group's children, give the group and each option a key, and leave the
//! background off the options:
//!
//! ```ignore
//! row().key("theme-seg").child(SelectionPill::new("theme-seg", selected_key).fill(t.surface_raised).build(cx))
//! ```

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{Color, Cx, El, Shadow, SpringPair, Transition};

use crate::ui::theme::{motion, radius, Utilities};

use super::button::fast_spring;

/// `<SelectionPill className />`.
pub struct SelectionPill {
    group: String,
    selected: Option<String>,
    fill: Color,
    radius: f32,
    shadows: Vec<Shadow>,
    sheen: Option<bool>,
}

impl SelectionPill {
    /// The pill for the group keyed `group`, under the option keyed `selected`.
    pub fn new(group: impl Into<String>, selected: Option<String>) -> SelectionPill {
        SelectionPill { group: group.into(), selected, fill: Color::TRANSPARENT, radius: radius::MD, shadows: Vec::new(), sheen: None }
    }
    /// `bg-*`.
    pub fn fill(mut self, c: Color) -> Self {
        self.fill = c;
        self
    }
    /// `rounded-*` (`rounded-md` by default).
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    /// `shadow-*`.
    pub fn shadows(mut self, s: &[Shadow]) -> Self {
        self.shadows = s.to_vec();
        self
    }
    /// `sheen` / `sheen-strong`.
    pub fn sheen(mut self, strong: bool) -> Self {
        self.sheen = Some(strong);
        self
    }

    pub fn build<S>(self, cx: &Cx<S>) -> El {
        let group = cx.layout(&self.group);
        let on = self.selected.as_deref().and_then(|k| cx.layout(k));
        let (Some(g), Some(o)) = (group, on) else { return rune::el().key("selection-pill-none") };
        if o[2] <= 0.0 {
            return rune::el().key("selection-pill-none");
        }
        // `absolute top-0 left-0`, then translated: offsets are within the group's padding box,
        // which Rune's absolute positions use too.
        let mut e = rune::el()
            .key("selection-pill")
            .absolute(o[0] - g[0], o[1] - g[1])
            .size(o[2], o[3])
            .radius(self.radius)
            .fill(self.fill)
            .elevate(&self.shadows)
            .motion(motion::soft())
            .transition(Transition::opacity().springs(SpringPair::new(fast_spring(), fast_spring())))
            .a11y_hidden();
        if let Some(strong) = self.sheen {
            e = e.sheen(strong);
        }
        e
    }
}
