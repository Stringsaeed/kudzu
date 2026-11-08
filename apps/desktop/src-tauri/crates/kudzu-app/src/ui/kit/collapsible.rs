//! `components/ui/collapsible.tsx` (Radix Collapsible): a trigger that shows and hides its
//! content. shadcn gives it no styling or motion: the content mounts and unmounts at once, and
//! the trigger reports `aria-expanded`. Enter and Space on a focused trigger toggle it.
//!
//! ```ignore
//! Collapsible::new("advanced")
//!     .open(s.advanced_open)
//!     .on_open_change(|s: &mut MainUi, open, _| s.advanced_open = open)
//!     .trigger(Button::new("Advanced").variant(Variant::Ghost).build(th))
//!     .content(advanced_fields)
//!     .build()
//! ```

use std::rc::Rc;

use rune::{El, Transition, Tx, col, el};

use super::popover::OpenChange;

pub struct Collapsible<S> {
    key: String,
    open: bool,
    disabled: bool,
    change: Option<OpenChange<S>>,
    trigger: Option<El>,
    content: Option<El>,
}

impl<S: 'static> Collapsible<S> {
    pub fn new(key: impl Into<String>) -> Self {
        Collapsible { key: key.into(), open: false, disabled: false, change: None, trigger: None, content: None }
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    pub fn on_open_change(mut self, f: impl Fn(&mut S, bool, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }
    /// `CollapsibleTrigger asChild`. Its own `on_press` is replaced by the toggle.
    pub fn trigger(mut self, t: El) -> Self {
        self.trigger = Some(t);
        self
    }
    /// `CollapsibleContent`.
    pub fn content(mut self, c: El) -> Self {
        self.content = Some(c);
        self
    }

    pub fn build(self) -> El {
        let open = self.open;
        let mut trigger = self.trigger.unwrap_or_else(el).expanded(open);
        if let (Some(f), false) = (self.change, self.disabled) {
            trigger = trigger.on_press_tx(move |s: &mut S, tx| f(s, !open, tx));
        }
        let mut root = col().key(self.key.clone()).child(trigger);
        if open && let Some(c) = self.content {
            root = root.child(c.key(format!("{}-content", self.key)).transition(Transition::none()));
        }
        root
    }
}
