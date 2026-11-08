//! `components/app/breadcrumbs.tsx`: a trail of groups. Past four it collapses the middle into a
//! "…" button, which opens a menu of the hidden parents (the screen draws the menu, anchored to
//! [`MORE_KEY`]).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{icon, row, El, Tx};

use crate::ui::icons;
use crate::ui::theme::{radius, Theme};

use super::breadcrumb::{breadcrumb, breadcrumb_item, breadcrumb_page, breadcrumb_separator, BreadcrumbLink};
use super::button::{hover_fast, ink, layer, Look};

/// The key of the "Show parent groups" button, for anchoring its menu.
pub const MORE_KEY: &str = "breadcrumbs-more";

type Select = Box<dyn FnOnce(BreadcrumbLink) -> BreadcrumbLink>;

/// One crumb: a label, and what selecting it does (none for the current page).
pub struct Entry {
    label: String,
    select: Option<Select>,
}

impl Entry {
    pub fn new(label: impl Into<String>) -> Entry {
        Entry { label: label.into(), select: None }
    }
    pub fn on_select<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Entry {
        self.select = Some(Box::new(move |l| l.on_press(f)));
        self
    }
    pub fn on_select_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Entry {
        self.select = Some(Box::new(move |l| l.on_press_tx(f)));
        self
    }
}

/// `<Breadcrumbs items label />`.
pub struct Breadcrumbs {
    items: Vec<Entry>,
    label: String,
    more: Option<Box<dyn FnOnce(El) -> El>>,
    more_open: bool,
}

impl Breadcrumbs {
    pub fn new(items: Vec<Entry>) -> Breadcrumbs {
        Breadcrumbs { items, label: "Breadcrumb".into(), more: None, more_open: false }
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = l.into();
        self
    }
    /// Opens the hidden parents' menu (the labels are [`Breadcrumbs::hidden`]).
    pub fn on_more<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.more = Some(Box::new(move |e| e.on_press(f)));
        self
    }
    /// Whether that menu is open (`aria-expanded`).
    pub fn more_open(mut self, open: bool) -> Self {
        self.more_open = open;
        self
    }
    /// The indexes of the parents collapsed into the "…" menu (`items.slice(1, -2)` past four).
    pub fn hidden(len: usize) -> std::ops::Range<usize> {
        if len > 4 { 1..len - 2 } else { 0..0 }
    }

    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let n = self.items.len();
        let collapsed = n > 4;
        let hidden = Self::hidden(n);
        let visible: Vec<(usize, Entry)> = self.items.into_iter().enumerate().filter(|(i, _)| !hidden.contains(i)).collect();
        let last = visible.len().saturating_sub(1);
        let mut more = self.more;
        let mut parts = Vec::new();
        for (index, (slot, item)) in visible.into_iter().enumerate() {
            if index > 0 {
                parts.push(breadcrumb_separator(th).key(format!("separator-{slot}")));
            }
            if collapsed && index == 1 {
                // `flex size-6 items-center justify-center rounded-md hover:bg-accent hover:text-foreground`.
                let look = Look { live: true, ..Look::default() };
                let mut b = hover_fast(row())
                    .key(MORE_KEY)
                    .role("button")
                    .label("Show parent groups")
                    .size(24.0, 24.0)
                    .center()
                    .radius(radius::MD)
                    .child(layer(0.0).radius(radius::MD).fill(t.accent).group_opacity(0.0, 1.0))
                    .child(ink(icon(&icons::ELLIPSIS, 16.0), t.muted_foreground, Some(t.foreground), look).a11y_hidden())
                    .expanded(self.more_open)
                    .focusable(true)
                    .focus_ring(2.0, 1.0, t.ring);
                if let Some(f) = more.take() {
                    b = f(b);
                }
                parts.push(breadcrumb_item([b]).key("more-slot"));
                parts.push(breadcrumb_separator(th).key("more-separator"));
            }
            let crumb = if index == last {
                breadcrumb_item([breadcrumb_page(th, item.label)]).grow(1.0)
            } else {
                let mut link = BreadcrumbLink::new(item.label);
                if let Some(f) = item.select {
                    link = f(link);
                }
                breadcrumb_item([link.build(th)]).max_w(160.0)
            };
            parts.push(crumb.key(format!("crumb-{slot}")));
        }
        breadcrumb(self.label, parts).min_w(0.0)
    }
}
