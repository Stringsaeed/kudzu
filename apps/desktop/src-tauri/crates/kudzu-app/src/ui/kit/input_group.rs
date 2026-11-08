//! `components/ui/input-group.tsx`: a field box holding an Input or Textarea with addons (icons,
//! text, keycaps, buttons) inline at either end or as blocks above and below.
//!
//! ```ignore
//! InputGroup::input(Input::new(&s.query).placeholder("Search…").on_edit(|s: &mut S| &mut s.query))
//!     .start(addon_icon(th, &icons::SEARCH))
//!     .end(Shortcut::new("mod+k").build(th))
//!     .build(th)
//! ```

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{col, icon, row, text, El, Icon};

use crate::ui::theme::{text as ty, weight, Theme, WeightOf};

use super::button::{Button, Size, State, Variant};
use super::input::{field, shell, Input};
use super::textarea::Textarea;

enum Control {
    Input(Input),
    Textarea(Textarea),
}

/// `InputGroupAddon`'s `align`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    InlineStart,
    InlineEnd,
    BlockStart,
    BlockEnd,
}

/// What an addon holds, for its `has-[>button]` / `has-[>kbd]` margins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AddonKind {
    #[default]
    Plain,
    Button,
    Kbd,
}

/// `<InputGroup>`.
pub struct InputGroup {
    control: Control,
    addons: Vec<(Align, AddonKind, El)>,
    state: State,
    width: Option<f32>,
}

impl InputGroup {
    pub fn input(i: Input) -> InputGroup {
        InputGroup { control: Control::Input(i), addons: Vec::new(), state: State::Rest, width: None }
    }
    pub fn textarea(t: Textarea) -> InputGroup {
        InputGroup { control: Control::Textarea(t), addons: Vec::new(), state: State::Rest, width: None }
    }
    /// An `InputGroupAddon` holding `content`.
    pub fn addon(mut self, align: Align, kind: AddonKind, content: El) -> Self {
        self.addons.push((align, kind, content));
        self
    }
    /// An inline-start addon.
    pub fn start(self, content: El) -> Self {
        self.addon(Align::InlineStart, AddonKind::Plain, content)
    }
    /// An inline-end addon.
    pub fn end(self, content: El) -> Self {
        self.addon(Align::InlineEnd, AddonKind::Plain, content)
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let (focused, disabled, invalid, textarea) = match &self.control {
            Control::Input(i) => (i.edit.focused || i.state == State::Focus || self.state == State::Focus, i.disabled, i.invalid, false),
            Control::Textarea(a) => (a.edit.focused || a.state == State::Focus || self.state == State::Focus, a.disabled, a.invalid, true),
        };
        let focused = focused && !disabled;
        let f = field(th, focused, disabled, invalid, false);
        let has = |a: Align| self.addons.iter().any(|(x, _, _)| *x == a);
        let block = has(Align::BlockStart) || has(Align::BlockEnd);
        let addon = |align: Align, kind: AddonKind, content: El| -> El {
            // `flex items-center justify-center gap-2 py-1.5 text-sm font-medium text-muted-foreground`.
            let mut a = row().gap(8.0).center_y().py(6.0).child(content);
            a = match align {
                Align::InlineStart => a.pl(8.0).ml(match kind {
                    AddonKind::Button => -4.8,
                    AddonKind::Kbd => -2.4,
                    AddonKind::Plain => 0.0,
                }),
                Align::InlineEnd => a.pr(8.0).mr(match kind {
                    AddonKind::Button => -4.8,
                    AddonKind::Kbd => -2.4,
                    AddonKind::Plain => 0.0,
                }),
                Align::BlockStart => a.w_full().justify_start().px(10.0).pt(8.0),
                Align::BlockEnd => a.w_full().justify_start().px(10.0).pb(8.0),
            };
            if disabled { a.opacity(0.5) } else { a }
        };
        // `has-[>[data-align=inline-start]]:[&>input]:pl-1.5`, and the block paddings.
        let pl = if has(Align::InlineStart) { 6.0 } else { 10.0 };
        let pr = if has(Align::InlineEnd) { 6.0 } else { 10.0 };
        let control = match self.control {
            Control::Input(i) => {
                let mut c = row().grow(1.0).min_w(0.0).h(30.0).pl(pl).pr(pr).center_y().child(i.editor(th));
                if block {
                    c = c.w_full();
                    if has(Align::BlockEnd) {
                        c = c.pt(12.0);
                    }
                    if has(Align::BlockStart) {
                        c = c.pb(12.0);
                    }
                }
                c
            }
            Control::Textarea(a) => col().grow(1.0).w_full().min_h(62.0).px(10.0).py(8.0).child(a.editor(th)),
        };
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        for (align, kind, content) in self.addons {
            match align {
                Align::InlineStart | Align::BlockStart => starts.push(addon(align, kind, content)),
                Align::InlineEnd | Align::BlockEnd => ends.push(addon(align, kind, content)),
            }
        }
        let mut e = if block || textarea { col() } else { row().center_y() };
        e = e.role("group").padding(1.0).children(starts).child(control).children(ends);
        if !(block || textarea) {
            e = e.h(32.0);
        }
        e = match self.width {
            Some(w) => e.w(w),
            None => e.w_full().min_w(0.0),
        };
        let e = shell(e, &f, crate::ui::theme::radius::LG);
        if disabled { e.opacity(0.5) } else { e }
    }
}

/// `InputGroupText`: muted text, with an icon if given.
pub fn group_text(th: &Theme, label: impl Into<String>, glyph: Option<&'static Icon>) -> El {
    let c = th.t.muted_foreground;
    let mut e = row().gap(8.0).center_y();
    if let Some(i) = glyph {
        e = e.child(icon(i, 16.0).color(c));
    }
    e.child(text(label.into()).font(ty::SM).color(c))
}

/// An addon's icon: `size-4`, muted.
pub fn addon_icon(th: &Theme, i: &'static Icon) -> El {
    icon(i, 16.0).color(th.t.muted_foreground).a11y_hidden()
}

/// An addon's text: `text-sm font-medium text-muted-foreground`.
pub fn addon_text(th: &Theme, s: impl Into<String>) -> El {
    text(s.into()).font(ty::SM.weight_of(weight::MEDIUM)).color(th.t.muted_foreground)
}

/// `InputGroupButton`'s `size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GroupButtonSize {
    /// `h-6 gap-1 rounded-[calc(var(--radius)-3px)] px-1.5`, 14 pt icons.
    #[default]
    Xs,
    /// The Button's own `sm`.
    Sm,
    /// `size-6`, no padding.
    IconXs,
    /// `size-8`, no padding.
    IconSm,
}

/// `InputGroupButton`: a ghost button sized for an addon (`text-sm shadow-none`).
#[track_caller]
pub fn group_button(label: impl Into<String>, size: GroupButtonSize) -> Button {
    let b = Button::new(label).variant(Variant::Ghost);
    group_button_size(b, size)
}

/// `InputGroupButton` with only an icon.
#[track_caller]
pub fn group_icon_button(i: &'static Icon, label: impl Into<String>, size: GroupButtonSize) -> Button {
    group_button_size(Button::icon_only(i, label).variant(Variant::Ghost), size)
}

fn group_button_size(b: Button, size: GroupButtonSize) -> Button {
    let md = ty::SM.weight_of(weight::MEDIUM);
    let r = crate::ui::theme::RADIUS - 3.0;
    match size {
        GroupButtonSize::Xs => b.size(Size::Xs).font(md).padding_x(6.0).radius(r).icon_size(14.0),
        GroupButtonSize::Sm => b.size(Size::Sm).font(md),
        GroupButtonSize::IconXs => b.size(Size::IconXs).radius(r),
        GroupButtonSize::IconSm => b.size(Size::IconSm).height(32.0),
    }
}
