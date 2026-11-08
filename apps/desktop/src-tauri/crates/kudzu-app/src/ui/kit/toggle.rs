//! `components/ui/toggle.tsx`: a two-state button. On, it lifts onto a raised surface
//! (`aria-pressed:bg-surface-raised aria-pressed:shadow-soft`).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{icon, row, text, Color, El, Icon, TextStyle, Tx};

use crate::ui::theme::{radius, text as ty, weight, Theme, WeightOf};

use super::button::{ink, paint, site_id, tween, Acts, Look, Skin, State, TW_EASE};

/// `variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToggleVariant {
    /// `bg-transparent`.
    #[default]
    Default,
    /// `bg-surface-raised shadow-raised sheen`.
    Outline,
}

/// `size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToggleSize {
    #[default]
    Default,
    Sm,
    Lg,
}

impl ToggleSize {
    /// Height (and min width), radius, icon size, font, inline-icon padding.
    fn metrics(self) -> (f32, f32, f32, TextStyle, f32) {
        let md = ty::SM.weight_of(weight::MEDIUM);
        match self {
            ToggleSize::Default => (32.0, radius::LG, 16.0, md, 8.0),
            ToggleSize::Sm => (28.0, radius::MD.min(12.0), 14.0, TextStyle { size: 12.8, ..md }, 6.0),
            ToggleSize::Lg => (36.0, radius::LG, 16.0, md, 8.0),
        }
    }
}

/// The skin of a toggle in `variant`, on or off.
pub(crate) fn toggle_skin(th: &Theme, variant: ToggleVariant, on: bool, r: f32) -> Skin {
    let t = &th.t;
    let mut s = Skin { focus_ring: Some(Theme::tint(t.ring, 0.5)), ..Skin::flat(r, Color::TRANSPARENT) };
    // `aria-pressed` outranks `hover:` in Tailwind's variant order: an on toggle keeps its look.
    if variant == ToggleVariant::Outline {
        s.fill = t.surface_raised;
        s.shadows = t.elev_raised.clone();
        s.sheen = Some(false);
        if !on {
            s.hover_fill = Some(t.muted);
            s.hover_shadows = Some(t.elev_raised_hover.clone());
        }
    } else if !on {
        s.hover_fill = Some(t.muted);
    }
    if on {
        s.fill = t.surface_raised;
        s.shadows = t.elev_soft.clone();
    }
    s
}

/// `<Toggle pressed onPressedChange>`.
pub struct Toggle {
    label: String,
    show_label: bool,
    icon: Option<(&'static Icon, bool)>,
    on: bool,
    variant: ToggleVariant,
    size: ToggleSize,
    state: State,
    disabled: bool,
    color: Option<Color>,
    corners: Option<[f32; 4]>,
    pad_x: Option<f32>,
    help: Option<String>,
    id: String,
    acts: Acts,
}

impl Toggle {
    #[track_caller]
    pub fn new(label: impl Into<String>, on: bool) -> Toggle {
        let label = label.into();
        let id = site_id(std::panic::Location::caller(), &label);
        Toggle {
            label,
            show_label: true,
            icon: None,
            on,
            variant: ToggleVariant::Default,
            size: ToggleSize::Default,
            state: State::Rest,
            disabled: false,
            color: None,
            corners: None,
            pad_x: None,
            help: None,
            id,
            acts: Acts::default(),
        }
    }
    /// An icon-only toggle; `label` names it (`aria-label`).
    #[track_caller]
    pub fn icon_only(i: &'static Icon, label: impl Into<String>, on: bool) -> Toggle {
        let mut t = Toggle::new(label, on);
        t.icon = Some((i, false));
        t.show_label = false;
        t
    }
    /// A leading icon (`data-icon="inline-start"` when `inline`, which tightens the padding).
    pub fn icon(mut self, i: &'static Icon, inline: bool) -> Self {
        self.icon = Some((i, inline));
        self
    }
    pub fn variant(mut self, v: ToggleVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: ToggleSize) -> Self {
        self.size = s;
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
    /// The text colour it inherits at rest.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    pub fn help(mut self, h: impl Into<String>) -> Self {
        self.help = Some(h.into());
        self
    }
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    pub(crate) fn corners(mut self, c: [f32; 4]) -> Self {
        self.corners = Some(c);
        self
    }
    pub(crate) fn padding_x(mut self, p: f32) -> Self {
        self.pad_x = Some(p);
        self
    }
    /// `onPressedChange`: the new value is `!on`.
    pub fn on_toggle<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.acts.push(move |e| e.on_press(f));
        self
    }
    pub fn on_toggle_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.acts.push(move |e| e.on_press_tx(f));
        self
    }

    pub fn build(self, th: &Theme) -> El {
        self.build_as(th, "button")
    }

    pub(crate) fn build_as(self, th: &Theme, role: &'static str) -> El {
        let (h, r, icon_sz, font, inline) = self.size.metrics();
        let enabled = !self.disabled;
        let look = Look::of(self.state, false, enabled);
        let mut skin = toggle_skin(th, self.variant, self.on, r);
        skin.corners = self.corners;
        let rest_ink = self.color.unwrap_or(th.t.foreground);
        let hover_ink = Some(th.t.foreground).filter(|&c| c != rest_ink);
        // `transition-all` (150 ms), not the `press` utility: no sink.
        let tw = tween(0.15, TW_EASE);
        let mut e = row().hover_group().hover_motion(tw, tw).role(role).label(self.label.clone()).h(h).min_w(h).gap(4.0).center();
        e = skin.round(e, 0.0);
        let px = self.pad_x.unwrap_or(10.0);
        e = e.pl(if self.icon.is_some_and(|(_, i)| i) { inline } else { px }).pr(px);
        e = paint(e, &skin, Look { pressed: false, ..look });
        if let Some((i, _)) = self.icon {
            e = e.child(ink(icon(i, icon_sz), rest_ink, hover_ink, look).key("icon").a11y_hidden());
        }
        if self.show_label {
            e = e.child(ink(text(self.label.clone()).font(font), rest_ink, hover_ink, look).key("label").a11y_hidden());
        }
        if let Some(h) = &self.help {
            e = e.help(h.clone());
        }
        e = e.checked(self.on);
        if !enabled {
            return e.disabled(true).layer_opacity(0.5);
        }
        e = e.focusable(true);
        self.acts.apply(e, true)
    }
}
