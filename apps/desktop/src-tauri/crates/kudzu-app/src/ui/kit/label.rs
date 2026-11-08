//! `components/ui/label.tsx`: a form label (`text-sm leading-none font-medium`), dimmed beside a
//! disabled control.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{row, text, Color, El, TextStyle};

use crate::ui::theme::{weight, Theme};

/// `text-sm leading-none font-medium`.
pub const FONT: TextStyle = TextStyle::new(14.0, weight::MEDIUM).lh(14.0);

/// `<Label>`: `text`, then any children (a control beside it), `gap-2` apart.
pub struct Label {
    text: String,
    children: Vec<El>,
    disabled: bool,
    color: Option<Color>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Label {
        Label { text: text.into(), children: Vec::new(), disabled: false, color: None }
    }
    pub fn child(mut self, e: El) -> Self {
        self.children.push(e);
        self
    }
    /// `group-data-[disabled=true]` / `peer-disabled`: the control it labels is disabled.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    pub fn build(self, th: &Theme) -> El {
        let e = row().gap(8.0).center_y().child(text(self.text).font(FONT).color(self.color.unwrap_or(th.t.foreground))).children(self.children);
        if self.disabled { e.opacity(0.5) } else { e }
    }
}
