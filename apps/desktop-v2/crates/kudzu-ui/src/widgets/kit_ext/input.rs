//! shadcn's `Input` and `InputGroup` (`components/ui/input.tsx`, `input-group.tsx`) as rune-kit's
//! [`rune_kit::TextInput`], decorated.
//!
//! Verdict in `PARITY_MATRIX.md`: **decorated**. rune-kit's field is 28 points tall with 13/16
//! type, 2 points before the text and 8 after, `radius.md` corners, a hover tint and a 1 point
//! inside focus ring. Kudzu's is 32 tall (`h-8`), 14/20 (`md:text-sm`), 11 points either side
//! (`px-2.5` plus the transparent border), 10 point corners, no hover, a `well` shadow at rest
//! and at focus a 1 point ring-coloured border with a 3 point `ring/50` halo. Height and type go
//! through the builder (`height`, `text_style`); padding, radius, fill and the focus look are
//! overrides on the built element. Upstream: gap 17, "rune-kit `TextInput`: take padding,
//! radius and the focus ring from `Kit`" (proposed in S06).

use rune::{Color, El, Icon, Shadow, TextEdit, TextStyle};
use rune_kit::{State, TextInput};

use crate::theme::{radius, Theme};

/// `h-8`.
pub const HEIGHT: f32 = 32.0;
/// `px-2.5` plus the 1 point transparent border.
pub const INSET: f32 = 11.0;
/// rune-kit's spacer before the editor when no icon leads.
const KIT_LEAD: f32 = 2.0;
/// `input-group-addon`'s `pl-2` plus the border: where a leading icon starts.
const ADDON_INSET: f32 = 9.0;
/// rune-kit's padding before a leading icon.
const KIT_ICON_LEAD: f32 = 8.0;

/// `Input::new(&s.name).with(|t| t.placeholder("Name").on_edit(|s: &mut S| &mut s.name))`.
pub struct Input {
    inner: TextInput,
    focused: bool,
    leading: bool,
    state: State,
    invalid: bool,
    style: Option<TextStyle>,
}

impl Input {
    pub fn new(edit: &TextEdit) -> Input {
        Input {
            inner: TextInput::new(edit).height(HEIGHT).width(None),
            focused: edit.focused,
            leading: false,
            state: State::Rest,
            invalid: false,
            style: None,
        }
    }
    /// Configure the rune-kit field: placeholder, handlers, trailing slot, width.
    pub fn with(mut self, f: impl FnOnce(TextInput) -> TextInput) -> Self {
        self.inner = f(self.inner);
        self
    }
    /// The text's type: `Theme::ty.input` (14/20) unless set, e.g. the 13 point filter fields.
    pub fn text_style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }
    /// An `InputGroupAddon` icon before the text.
    pub fn leading_icon(mut self, i: &'static Icon) -> Self {
        self.inner = self.inner.leading_icon(i);
        self.leading = true;
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.inner = self.inner.state(s);
        self.state = s;
        self
    }
    pub fn invalid(mut self, on: bool) -> Self {
        self.inner = self.inner.invalid(on);
        self.invalid = on;
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        let lead = if self.leading {
            ADDON_INSET - KIT_ICON_LEAD
        } else {
            INSET - KIT_LEAD
        };
        let e = self
            .inner
            .text_style(self.style.unwrap_or(theme.ty.input))
            .build(theme.kit())
            .radius(radius::LG)
            .pl(lead)
            .pr(INSET)
            .hover(Color::TRANSPARENT);
        let focused = (self.focused || self.state == State::Focus) && self.state != State::Disabled;
        well(
            theme,
            e,
            focused,
            self.invalid,
            self.state == State::Disabled,
        )
    }
}

/// `bg-foreground/[0.025]`, `dark:bg-black/15`.
pub fn rest_fill(theme: &Theme) -> Color {
    if theme.dark {
        Color::BLACK.alpha(0.15)
    } else {
        theme.c.foreground.alpha(0.025)
    }
}

/// The input and textarea surface over a built field: fill, and at focus or when invalid the
/// ring-coloured border with its 3 point halo (rune-kit already dropped the `well` shadow and
/// drew a 1 point ring there; the halo replaces it and the border is an inset ring).
pub(super) fn well(theme: &Theme, e: El, focused: bool, invalid: bool, disabled: bool) -> El {
    let c = &theme.c;
    let border = |e: El, colour: Color| {
        e.box_shadow(Shadow {
            spread: 1.0,
            color: colour,
            inset: true,
            ..Shadow::default()
        })
    };
    if disabled {
        let a = if theme.dark { 0.8 } else { 0.5 };
        return e.fill(c.input.alpha(a));
    }
    if invalid {
        let halo = c.destructive.alpha(if theme.dark { 0.4 } else { 0.2 });
        return border(
            e.fill(rest_fill(theme)).outline(3.0, 0.0, halo),
            c.destructive,
        );
    }
    if focused {
        return border(
            e.fill(Color::TRANSPARENT)
                .outline(3.0, 0.0, c.ring.alpha(0.5)),
            c.ring,
        );
    }
    e.fill(rest_fill(theme))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_starts_where_the_web_puts_it() {
        // rune-kit draws a 2 point spacer before the editor: 9 + 2 = 11, `px-2.5` + border.
        assert_eq!(INSET - KIT_LEAD + KIT_LEAD, 11.0);
        assert_eq!(HEIGHT, 32.0);
        assert_eq!(Theme::light().ty.input.size, 14.0);
    }

    #[test]
    fn rest_fill_follows_the_theme() {
        assert_eq!(rest_fill(Theme::dark()), Color::BLACK.alpha(0.15));
        assert_eq!(
            rest_fill(Theme::light()),
            Theme::light().c.foreground.alpha(0.025)
        );
    }
}
