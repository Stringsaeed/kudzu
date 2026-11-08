//! `components/ui/toggle-group.tsx`: toggles in a row (or column), `spacing` × 4 pt apart; at
//! spacing 0 they join, square inside and rounded at the ends. The group is one Tab stop; arrow keys
//! move between its items.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{col, row, El};

use crate::ui::theme::{radius, Theme};

use super::toggle::{Toggle, ToggleSize, ToggleVariant};

/// `orientation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// `<ToggleGroup type="single" | "multiple">`.
pub struct ToggleGroup {
    label: String,
    items: Vec<Toggle>,
    variant: Option<ToggleVariant>,
    size: Option<ToggleSize>,
    spacing: f32,
    orientation: Orientation,
}

impl ToggleGroup {
    /// A single-choice group (`type="single"`), named `label`.
    pub fn single(label: impl Into<String>) -> ToggleGroup {
        ToggleGroup { label: label.into(), items: Vec::new(), variant: None, size: None, spacing: 2.0, orientation: Orientation::Horizontal }
    }
    /// A multiple-choice group (`type="multiple"`). Which items are on is the caller's: both
    /// kinds draw alike.
    pub fn multiple(label: impl Into<String>) -> ToggleGroup {
        ToggleGroup::single(label)
    }
    /// A `ToggleGroupItem`: a toggle, restyled by the group's variant and size.
    pub fn item(mut self, t: Toggle) -> Self {
        self.items.push(t);
        self
    }
    pub fn variant(mut self, v: ToggleVariant) -> Self {
        self.variant = Some(v);
        self
    }
    pub fn size(mut self, s: ToggleSize) -> Self {
        self.size = Some(s);
        self
    }
    /// `spacing` (in Tailwind units of 4 pt; 2 by default).
    pub fn spacing(mut self, s: f32) -> Self {
        self.spacing = s;
        self
    }
    pub fn orientation(mut self, o: Orientation) -> Self {
        self.orientation = o;
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let n = self.items.len();
        let joined = self.spacing == 0.0;
        let horizontal = self.orientation == Orientation::Horizontal;
        let r = radius::LG;
        let items: Vec<El> = self
            .items
            .into_iter()
            .enumerate()
            .map(|(i, mut t)| {
                if let Some(v) = self.variant {
                    t = t.variant(v);
                }
                if let Some(s) = self.size {
                    t = t.size(s);
                }
                if joined {
                    let (first, last) = (i == 0, i + 1 == n);
                    let c = if horizontal {
                        [if first { r } else { 0.0 }, if last { r } else { 0.0 }, if last { r } else { 0.0 }, if first { r } else { 0.0 }]
                    } else {
                        [if first { r } else { 0.0 }, if first { r } else { 0.0 }, if last { r } else { 0.0 }, if last { r } else { 0.0 }]
                    };
                    t = t.corners(c).padding_x(8.0);
                }
                t.build_as(th, "button")
            })
            .collect();
        let gap = self.spacing * 4.0;
        let e = if horizontal { row().center_y() } else { col().items_stretch() };
        // Radix's roving focus: one Tab stop, arrows move between items (they don't press).
        e.gap(gap).role("group").label(self.label).children(items).focus_group()
    }
}
