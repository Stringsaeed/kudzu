//! `components/ui/radio-group.tsx`: a group of radio items. Arrow keys move through the group and
//! select as they go (Radix's roving focus).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{col, El, Tx};

use crate::ui::theme::{radius, Theme};

use super::button::{site_id, State};
use super::checkbox::{bind, bind_tx, boxed, Bind, Boxed};

/// `<RadioGroup>`: `grid w-full gap-2` around its items, one Tab stop.
pub fn radio_group(label: impl Into<String>, items: impl IntoIterator<Item = El>) -> El {
    col().role("radiogroup").label(label.into()).w_full().gap(8.0).focus_group().children(items)
}

/// `<RadioGroupItem>`: a 16 pt round box; checked, a raised primary key with an 8 pt dot that lands
/// with `animate-check`.
pub struct RadioItem {
    checked: bool,
    state: State,
    disabled: bool,
    invalid: bool,
    label: Option<String>,
    id: String,
    bind: Option<Bind>,
}

impl RadioItem {
    #[track_caller]
    pub fn new(checked: bool) -> RadioItem {
        RadioItem { checked, state: State::Rest, disabled: false, invalid: false, label: None, id: site_id(std::panic::Location::caller(), "radio"), bind: None }
    }
    pub fn state(mut self, s: State) -> Self {
        self.state = s;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn invalid(mut self, i: bool) -> Self {
        self.invalid = i;
        self
    }
    /// The accessible name (its `<Label>`'s text).
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// The press id, unique in the window (items built in a loop: pass the value).
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }
    /// Selects this item.
    pub fn on_select<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.bind = Some(bind(f));
        self
    }
    pub fn on_select_tx<S: 'static>(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.bind = Some(bind_tx(f));
        self
    }
    pub fn build(self, th: &Theme) -> El {
        let dot = rune::el().size(8.0, 8.0).radius(radius::FULL).fill(th.t.primary_foreground).a11y_hidden();
        let checked = self.checked;
        boxed(
            th,
            Boxed {
                role: "radio",
                round: true,
                checked,
                indicator: dot,
                state: self.state,
                disabled: self.disabled,
                invalid: self.invalid,
                label: self.label,
                id: self.id,
                bind: self.bind,
                transition: false,
            },
        )
        .focus_selected(checked)
    }
}
