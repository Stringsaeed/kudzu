//! `components/ui/breadcrumb.tsx`: a breadcrumb trail's parts.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{icon, row, text, El, Spring, Tx};

use crate::ui::icons;
use crate::ui::theme::{text as ty, weight, Theme, WeightOf};

use super::button::{hover_fast, ink, tween, Acts, Look, State, TW_EASE};

/// `<Breadcrumb><BreadcrumbList>`: the trail, `gap-2`, `text-[13px] text-muted-foreground`.
pub fn breadcrumb(label: impl Into<String>, children: impl IntoIterator<Item = El>) -> El {
    row().role("navigation").label(label.into()).min_w(0.0).center_y().gap(8.0).children(children)
}

/// `BreadcrumbItem`: `inline-flex min-w-0 items-center gap-2`.
pub fn breadcrumb_item(children: impl IntoIterator<Item = El>) -> El {
    row().min_w(0.0).shrink().center_y().gap(8.0).children(children)
}

/// `BreadcrumbPage`: the current page, `truncate font-medium text-foreground`.
pub fn breadcrumb_page(th: &Theme, label: impl Into<String>) -> El {
    let label = label.into();
    text(label.clone()).font(ty::BASE.weight_of(weight::MEDIUM)).color(th.t.foreground).truncate().min_w(0.0).shrink().help(label)
}

/// `BreadcrumbSeparator`: a 14 pt chevron in `--faint`.
pub fn breadcrumb_separator(th: &Theme) -> El {
    icon(&icons::CHEVRON_RIGHT, 14.0).color(th.t.faint).a11y_hidden()
}

/// `BreadcrumbLink` (as a button): truncates, and turns `text-foreground` on hover.
pub struct BreadcrumbLink {
    label: String,
    state: State,
    acts: Acts,
}

impl BreadcrumbLink {
    pub fn new(label: impl Into<String>) -> BreadcrumbLink {
        BreadcrumbLink { label: label.into(), state: State::Rest, acts: Acts::default() }
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn on_press<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.acts.push(move |e| e.on_press(f));
        self
    }
    pub fn on_press_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_press_tx(f));
        self
    }
    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let look = Look::of(self.state, false, true);
        let label = ink(text(self.label.clone()).font(ty::BASE).truncate(), t.muted_foreground, Some(t.foreground), look).motion(Spring::new(0.15, 0.0));
        // `transition-colors` (150 ms) on `rounded-sm`, with the page's default focus outline.
        let tw = tween(0.15, TW_EASE);
        let e = hover_fast(row()).hover_motion(tw, tw).role("button").label(self.label.clone()).help(self.label).min_w(0.0).shrink().radius(crate::ui::theme::radius::SM).child(label);
        let e = e.focusable(true).focus_ring(2.0, 1.0, t.ring);
        self.acts.apply(e, true)
    }
}
