//! `components/ui/item.tsx`: a list row (media, content, actions) and its parts. Each part is a
//! function returning the element with shadcn's classes; screens chain their overrides on it, as
//! the JSX passes `className`.
//!
//! ```ignore
//! Item::new().variant(ItemVariant::Outline).size(ItemSize::Sm)
//!     .child(item_media(th, MediaVariant::Icon, ItemSize::Sm, true, icon(&icons::LOCK, 16.0)))
//!     .child(item_content(ItemSize::Sm, [item_title(th, "Accessibility"), item_description(th, ItemSize::Sm, "…")]))
//!     .child(item_actions([status]))
//!     .on_press(|s: &mut S| s.open())
//!     .build(th)
//! ```

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{col, row, text, Color, El, Spring, TextStyle, Tx};

use crate::ui::theme::{radius, weight, Theme};

use super::button::{layer, tween, Acts, State, TW_EASE};

/// `variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ItemVariant {
    /// `border-transparent`.
    #[default]
    Default,
    /// `border-border`.
    Outline,
    /// `bg-muted/50`.
    Muted,
}

/// `size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ItemSize {
    #[default]
    Default,
    Sm,
    Xs,
}

/// `<Item>`.
pub struct Item {
    variant: ItemVariant,
    size: ItemSize,
    children: Vec<El>,
    hover_fill: Option<Color>,
    fill: Option<Color>,
    state: State,
    label: Option<String>,
    acts: Acts,
}

impl Item {
    pub fn new() -> Item {
        Item { variant: ItemVariant::Default, size: ItemSize::Default, children: Vec::new(), hover_fill: None, fill: None, state: State::Rest, label: None, acts: Acts::default() }
    }
    pub fn variant(mut self, v: ItemVariant) -> Self {
        self.variant = v;
        self
    }
    pub fn size(mut self, s: ItemSize) -> Self {
        self.size = s;
        self
    }
    pub fn child(mut self, e: El) -> Self {
        self.children.push(e);
        self
    }
    pub fn children(mut self, es: impl IntoIterator<Item = El>) -> Self {
        self.children.extend(es);
        self
    }
    /// `bg-*` over the variant's.
    pub fn fill(mut self, c: Color) -> Self {
        self.fill = Some(c);
        self
    }
    /// `hover:bg-*` other than `[a]:hover:bg-muted`.
    pub fn hover_fill(mut self, c: Color) -> Self {
        self.hover_fill = Some(c);
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// Makes the row a link or button (`asChild`), which takes `[a]:hover:bg-muted`.
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
        let (gap, px, py) = match self.size {
            ItemSize::Default | ItemSize::Sm => (10.0, 12.0, 10.0),
            ItemSize::Xs => (8.0, 10.0, 8.0),
        };
        let border = if self.variant == ItemVariant::Outline { t.border } else { Color::TRANSPARENT };
        let fill = self.fill.unwrap_or(if self.variant == ItemVariant::Muted { Theme::tint(t.muted, 0.5) } else { Color::TRANSPARENT });
        let r = radius::LG;
        // The border takes no room in Rune, so the padding carries its point.
        let mut e = row().flex_wrap().w_full().center_y().gap(gap).px(px + 1.0).py(py + 1.0).radius(r).fill(fill).motion(Spring::new(0.1, 0.0));
        let hover = self.hover_fill.or(link.then_some(t.muted));
        if let Some(h) = hover {
            if self.state == State::Hover {
                e = e.child(layer(0.0).key("item-hover").radius(r).fill(h));
            } else {
                // `transition-colors duration-100`.
                let tw = tween(0.1, TW_EASE);
                e = e.hover_group().hover_motion(tw, tw).child(layer(0.0).radius(r).fill(h).group_opacity(0.0, 1.0).key("item-hover"));
            }
        } else {
            e = e.child(layer(0.0).key("item-hover").opacity(0.0));
        }
        e = e.children(self.children).border(1.0, border);
        if let Some(l) = self.label {
            e = e.label(l);
        }
        if link {
            e = e.role("button").focusable(true).focus_ring(3.0, 0.0, Theme::tint(t.ring, 0.5)).child(layer(0.0).key("item-focus").radius(r).focus_ring(1.0, -1.0, t.ring));
        } else {
            e = e.role("listitem");
        }
        self.acts.apply(e, true)
    }
}

impl Default for Item {
    fn default() -> Self {
        Item::new()
    }
}

/// `ItemMedia`'s `variant`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MediaVariant {
    #[default]
    Default,
    /// 16 pt icons.
    Icon,
    /// A square image: 40, 32 at `sm`, 24 at `xs`, `rounded-sm`, cropped.
    Image,
}

/// `ItemMedia`. Beside a description it sits at the top, nudged down 2 pt.
pub fn item_media(_th: &Theme, variant: MediaVariant, size: ItemSize, has_description: bool, content: El) -> El {
    let mut e = row().gap(8.0).center().child(content);
    if variant == MediaVariant::Image {
        let s = match size {
            ItemSize::Default => 40.0,
            ItemSize::Sm => 32.0,
            ItemSize::Xs => 24.0,
        };
        e = e.size(s, s).radius(radius::SM).clip_rounded(radius::SM);
    }
    if has_description { e.self_start().offset(0.0, 2.0) } else { e }
}

/// `ItemContent`: `flex flex-1 flex-col gap-1` (`gap-0` at `xs`).
pub fn item_content(size: ItemSize, children: impl IntoIterator<Item = El>) -> El {
    col().grow(1.0).min_w(0.0).gap(if size == ItemSize::Xs { 0.0 } else { 4.0 }).children(children)
}

/// `ItemTitle`'s font: `text-sm leading-snug font-medium`.
pub const TITLE: TextStyle = TextStyle::new(14.0, weight::MEDIUM).lh(19.25);

/// `ItemTitle`: one line (`line-clamp-1`).
pub fn item_title(th: &Theme, s: impl Into<String>) -> El {
    text(s.into()).font(TITLE).color(th.t.foreground).max_lines(1)
}

/// `ItemDescription`: muted, two lines at most; `text-xs` at `xs` (`leading-normal` keeps 1.5).
pub fn item_description(th: &Theme, size: ItemSize, s: impl Into<String>) -> El {
    let font = if size == ItemSize::Xs { TextStyle::new(12.0, weight::NORMAL).lh(18.0) } else { TextStyle::new(14.0, weight::NORMAL).lh(21.0) };
    text(s.into()).font(font).color(th.t.muted_foreground).wrap().max_lines(2)
}

/// `ItemActions`: `flex items-center gap-2`.
pub fn item_actions(children: impl IntoIterator<Item = El>) -> El {
    row().center_y().gap(8.0).children(children)
}

/// `ItemHeader` / `ItemFooter`: a full-width row, ends apart.
pub fn item_header(children: impl IntoIterator<Item = El>) -> El {
    row().w_full().center_y().justify_between().gap(8.0).children(children)
}

/// `ItemFooter`.
pub fn item_footer(children: impl IntoIterator<Item = El>) -> El {
    item_header(children)
}

/// `ItemGroup`: a list of items, 16 pt apart (10 at `sm`, 8 at `xs`).
pub fn item_group(size: ItemSize, children: impl IntoIterator<Item = El>) -> El {
    let gap = match size {
        ItemSize::Default => 16.0,
        ItemSize::Sm => 10.0,
        ItemSize::Xs => 8.0,
    };
    col().role("list").w_full().gap(gap).children(children)
}

/// `ItemSeparator`: a rule with 8 pt above and below.
pub fn item_separator(th: &Theme) -> El {
    super::separator::horizontal(th).my(8.0)
}
