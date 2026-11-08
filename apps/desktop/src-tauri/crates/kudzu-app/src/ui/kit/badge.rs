//! `components/ui/badge.tsx`: a 20 pt pill label, and the "Dev" badge Kudzu Dev pins to its
//! windows.

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{icon, row, text, Color, Cx, El, Icon, Tx};

use crate::ui::theme::{text as ty, weight, Theme, WeightOf};

use super::button::{hover_fast, ink, layer, link_underline, Acts, Look, State};

/// `variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    #[default]
    Default,
    Secondary,
    Destructive,
    Outline,
    Ghost,
    Link,
}

/// `<Badge>`. With [`Badge::on_press`] it is the `asChild` link (`[a]:hover:*`).
pub struct Badge {
    label: String,
    variant: BadgeVariant,
    lead: Option<(&'static Icon, bool)>,
    trail: Option<(&'static Icon, bool)>,
    state: State,
    acts: Acts,
}

impl Badge {
    pub fn new(label: impl Into<String>) -> Badge {
        Badge { label: label.into(), variant: BadgeVariant::Default, lead: None, trail: None, state: State::Rest, acts: Acts::default() }
    }
    pub fn variant(mut self, v: BadgeVariant) -> Self {
        self.variant = v;
        self
    }
    /// A leading 12 pt icon; `inline` marks it `data-icon="inline-start"` (6 pt start padding).
    pub fn icon(mut self, i: &'static Icon, inline: bool) -> Self {
        self.lead = Some((i, inline));
        self
    }
    /// A trailing 12 pt icon; `inline` marks it `data-icon="inline-end"`.
    pub fn icon_end(mut self, i: &'static Icon, inline: bool) -> Self {
        self.trail = Some((i, inline));
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    /// Makes it a link (`asChild` over an `<a>`), which takes the variant's hover.
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
        let link = !self.acts.is_empty();
        // `[a]:hover:*` applies only to links; ghost and link variants hover anyway.
        let hovers = link || matches!(self.variant, BadgeVariant::Ghost | BadgeVariant::Link);
        let look = Look::of(self.state, false, hovers);
        let (fill, hover_fill, fg, hover_fg, border) = match self.variant {
            BadgeVariant::Default => (t.primary, Theme::tint(t.primary, 0.8), t.primary_foreground, None, Color::TRANSPARENT),
            BadgeVariant::Secondary => (t.secondary, Theme::tint(t.secondary, 0.8), t.secondary_foreground, None, Color::TRANSPARENT),
            BadgeVariant::Destructive => {
                let a = if th.dark { 0.2 } else { 0.1 };
                (Theme::tint(t.destructive, a), Theme::tint(t.destructive, 0.2), t.destructive, None, Color::TRANSPARENT)
            }
            BadgeVariant::Outline => (Color::TRANSPARENT, t.muted, t.foreground, Some(t.muted_foreground), t.border),
            BadgeVariant::Ghost => {
                (Color::TRANSPARENT, if th.dark { Theme::tint(t.muted, 0.5) } else { t.muted }, t.foreground, Some(t.muted_foreground), Color::TRANSPARENT)
            }
            BadgeVariant::Link => (Color::TRANSPARENT, Color::TRANSPARENT, t.primary, None, Color::TRANSPARENT),
        };
        let font = ty::XS.weight_of(weight::MEDIUM);
        let pl = if self.lead.is_some_and(|(_, i)| i) { 6.0 } else { 8.0 };
        let pr = if self.trail.is_some_and(|(_, i)| i) { 6.0 } else { 8.0 };
        // `h-5 rounded-4xl border border-transparent px-2 py-0.5 gap-1` (the border takes no room
        // here, so the padding carries it).
        let r = crate::ui::theme::radius::XXXXL;
        let mut e = row().role(if link { "link" } else { "group" }).h(20.0).pl(pl + 1.0).pr(pr + 1.0).gap(4.0).center().radius(r).clip_rounded(r);
        if hovers {
            e = hover_fast(e);
        }
        let show_hover = hovers && hover_fill != fill;
        e = e.child(layer(0.0).radius(r).fill(if look.hover { hover_fill } else { fill }).key("fill"));
        if show_hover && !look.hover {
            e = e.child(layer(0.0).radius(r).fill(hover_fill).group_opacity(0.0, 1.0).key("fill-hover"));
        }
        e = e.border(1.0, border);
        let hover_fg = if hovers { hover_fg } else { None };
        let glyph = |i: &'static Icon| ink(icon(i, 12.0), fg, hover_fg, look).a11y_hidden();
        if let Some((i, _)) = self.lead {
            e = e.child(glyph(i).key("icon"));
        }
        let mut label = ink(text(self.label.clone()).font(font), fg, hover_fg, look);
        if self.variant == BadgeVariant::Link {
            label = link_underline(label, font, fg, look);
        }
        e = e.child(label.key("label"));
        if let Some((i, _)) = self.trail {
            e = e.child(glyph(i).key("icon-end"));
        }
        if link {
            e = e.focusable(true).focus_ring(3.0, 0.0, Theme::tint(t.ring, 0.5));
        }
        self.acts.apply(e, true)
    }
}

/// The "Dev" badge pinned top-right in Kudzu Dev's onboarding (`fixed top-4 right-4`).
pub fn dev<S: 'static>(cx: &mut Cx<S>) -> El {
    let th = crate::ui::theme_for(cx);
    rune::el().key("dev-badge").pin(None, Some(16.0), Some(16.0), None).child(Badge::new("Dev").build(th))
}
