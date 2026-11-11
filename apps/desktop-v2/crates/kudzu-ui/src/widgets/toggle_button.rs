//! shadcn's `Toggle` (`components/ui/toggle.tsx`): a ghost button that stays down while on,
//! `aria-pressed:bg-surface-raised shadow-soft`. Built on the decorated button
//! ([`kit_ext::button`](super::kit_ext::button)), so its sizes are the button's (`sm` is 28,
//! `icon-sm` the formatting toolbar's 28 square); the on state adds the raised surface and the
//! `soft` depth, and the tree reads it as pressed.

use rune::{El, Icon};
use rune_kit::State;

use crate::theme::Theme;
use crate::widgets::kit_ext::button::{Button, Size, Variant};

pub struct ToggleButton {
    label: String,
    on: bool,
    icon: Option<&'static Icon>,
    size: Size,
    state: State,
    shortcut: Option<String>,
}

impl ToggleButton {
    pub fn new(label: impl Into<String>, on: bool) -> ToggleButton {
        ToggleButton {
            label: label.into(),
            on,
            icon: None,
            size: Size::Sm,
            state: State::Rest,
            shortcut: None,
        }
    }
    /// An icon-only toggle (`size` should be an icon size); the label names it.
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    pub fn build<S: 'static>(self, theme: &Theme, on_toggle: impl Fn(&mut S) + 'static) -> El {
        let mut b = Button::new(self.label)
            .variant(Variant::Ghost)
            .size(self.size)
            .state(self.state)
            .on_press(on_toggle);
        if let Some(i) = self.icon {
            b = b.icon(i);
        }
        if let Some(s) = self.shortcut {
            b = b.shortcut(s);
        }
        let mut e = b.build(theme).checked(self.on);
        if self.on {
            e = e.fill(theme.c.surface_raised);
            for s in &theme.depth.soft {
                e = e.box_shadow(*s);
            }
        }
        e
    }
}
