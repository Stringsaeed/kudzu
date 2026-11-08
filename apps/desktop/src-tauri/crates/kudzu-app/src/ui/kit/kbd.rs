//! `components/ui/kbd.tsx`: a keycap (`Kbd`) and a row of them (`KbdGroup`).
//!
//! CSS restyles a Kbd by where it sits (inside a tooltip, inside a default-variant Button); here
//! the caller says so with [`KbdContext`].

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{icon, row, text, Color, El, Icon, Shadow, TextStyle};

use crate::ui::theme::{weight, Theme};

/// Where the keycap sits, for the contextual looks Kbd's CSS selects with `in-data-[slot=…]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KbdContext {
    /// A regular surface.
    #[default]
    Surface,
    /// Inside a tooltip (inverted surface): `bg-background/20 text-background shadow-none`.
    Tooltip,
    /// Inside a default-variant (filled) Button: `bg-black/15 text-current` and an inset shadow.
    FilledButton,
}

/// `text-[11px] leading-none font-medium tabular`.
pub const FONT: TextStyle = TextStyle::new(11.0, weight::MEDIUM).lh(11.0).features("'tnum' 1");

/// `shadow-[inset_0_1px_1px_oklch(0_0_0/0.15)]`, the keycap on an accent fill.
pub fn on_accent_shadow() -> Shadow {
    Shadow { y: 1.0, blur: 1.0, color: Color::BLACK.alpha(0.15), inset: true, ..Shadow::default() }
}

/// `↵` as Chromium draws it in a keycap (from the system's symbol font): a hook down and left with
/// an arrowhead, about 0.9 em wide. The fallback face Rune finds for U+21B5 draws it much smaller.
static RETURN: Icon = Icon {
    name: "kudzu-kbd-return",
    view: 12.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round"><path d="M10.5 2.5v4.5H2"/><path d="M4.5 4.5 2 7l2.5 2.5"/></g>"#,
};

/// `⇥` likewise (U+21E5, a rightward arrow to a bar): Rune's fallback face draws it small too.
static TAB: Icon = Icon {
    name: "kudzu-kbd-tab",
    view: 12.0,
    body: r#"<g fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M1.5 6h8"/><path d="M7 3.5 9.5 6 7 8.5"/><path d="M10.5 3v6"/></g>"#,
};

/// One keycap.
pub struct Kbd {
    label: String,
    context: KbdContext,
    height: f32,
    font: TextStyle,
    fill: Option<Color>,
    color: Option<Color>,
    shadows: Option<Vec<Shadow>>,
    current: Option<Color>,
}

impl Kbd {
    pub fn new(label: impl Into<String>) -> Kbd {
        Kbd { label: label.into(), context: KbdContext::Surface, height: 20.0, font: FONT, fill: None, color: None, shadows: None, current: None }
    }
    pub fn context(mut self, c: KbdContext) -> Self {
        self.context = c;
        self
    }
    /// `currentColor` where the context says `text-current` (the filled button's text colour).
    pub fn current(mut self, c: Color) -> Self {
        self.current = Some(c);
        self
    }
    /// `h-* min-w-*` (20 by default).
    pub fn height(mut self, h: f32) -> Self {
        self.height = h;
        self
    }
    pub fn font(mut self, f: TextStyle) -> Self {
        self.font = f;
        self
    }
    /// `bg-*` override.
    pub fn fill(mut self, c: Color) -> Self {
        self.fill = Some(c);
        self
    }
    /// `text-*` override.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// `shadow-*` override (empty for `shadow-none`).
    pub fn shadows(mut self, s: Vec<Shadow>) -> Self {
        self.shadows = Some(s);
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let (fill, ink, shadows) = match self.context {
            KbdContext::Surface => (Theme::tint(t.foreground, 0.06), t.muted_foreground, t.elev_key.clone()),
            KbdContext::Tooltip => (Theme::tint(t.background, if th.dark { 0.1 } else { 0.2 }), t.background, Vec::new()),
            KbdContext::FilledButton => (Color::BLACK.alpha(0.15), self.current.unwrap_or(t.primary_foreground), vec![on_accent_shadow()]),
        };
        let fill = self.fill.unwrap_or(fill);
        let ink = self.color.unwrap_or(ink);
        let shadows = self.shadows.unwrap_or(shadows);
        let h = self.height;
        let mut e = row().h(h).min_w(h).px(4.0).gap(4.0).center().radius(5.0).fill(fill);
        for s in shadows {
            e = e.box_shadow(s);
        }
        let content = if self.label == "↵" {
            icon(&RETURN, (self.font.size * 1.05).round()).color(ink)
        } else if self.label == "⇥" {
            icon(&TAB, (self.font.size * 0.9).round()).color(ink)
        } else {
            text(self.label).font(self.font).color(ink)
        };
        e.child(content).a11y_hidden()
    }
}

/// `KbdGroup`: keycaps in a row, `gap` apart (4 by default).
pub fn group(caps: impl IntoIterator<Item = El>, gap: f32) -> El {
    row().gap(gap).center_y().children(caps).a11y_hidden()
}
