//! Sonner-style centered bottom toast host wrapping `rune_kit::ToastStack` and
//! `crate::widgets::kit_ext::toast` (`PLAN.md` S41, `LIB-7`, `SHL-10`).

use std::rc::Rc;

use rune::{col, El, Tx};
use rune_kit::toast_stack::VISIBLE;
use rune_kit::{Toast, ToastEntry, ToastEvent, ToastPage, ToastStack};

use crate::theme::Theme;
use crate::widgets::kit_ext::toast as kit_toast;

/// Default auto-dismiss duration for transient informational toasts.
pub const DEFAULT_TOAST_SECS: f64 = 4.0;
/// Auto-dismiss duration for Undo delete toasts (`LIB-7`: `toast.tsx` `6000ms`).
pub const UNDO_TOAST_SECS: f64 = 6.0;
/// Auto-dismiss duration for Update-ready toasts (`SHL-10`: `15000ms`).
pub const UPDATE_TOAST_SECS: f64 = 15.0;

/// Shared callback invoked when a toast action button is pressed.
pub type ToastHandler<S> = Rc<dyn Fn(&mut S, &mut Tx)>;

/// Action button specification attached to a [`ToastSpec`].
pub struct ToastAction<S: 'static> {
    pub label: String,
    pub primary: bool,
    pub handler: ToastHandler<S>,
}

impl<S: 'static> Clone for ToastAction<S> {
    fn clone(&self) -> Self {
        Self {
            label: self.label.clone(),
            primary: self.primary,
            handler: Rc::clone(&self.handler),
        }
    }
}

impl<S: 'static> std::fmt::Debug for ToastAction<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastAction")
            .field("label", &self.label)
            .field("primary", &self.primary)
            .finish()
    }
}

impl<S: 'static> ToastAction<S> {
    pub fn new(label: impl Into<String>, primary: bool, f: impl Fn(&mut S) + 'static) -> Self {
        Self {
            label: label.into(),
            primary,
            handler: Rc::new(move |s, _| f(s)),
        }
    }

    pub fn with_tx(
        label: impl Into<String>,
        primary: bool,
        f: impl Fn(&mut S, &mut Tx) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            primary,
            handler: Rc::new(f),
        }
    }
}

/// Content and actions for a single toast entry inside [`Toasts`].
pub struct ToastSpec<S: 'static> {
    pub title: String,
    pub description: Option<String>,
    pub duration_secs: Option<f64>,
    pub action: Option<ToastAction<S>>,
    pub cancel: Option<ToastAction<S>>,
    pub closable: bool,
}

impl<S: 'static> Clone for ToastSpec<S> {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            description: self.description.clone(),
            duration_secs: self.duration_secs,
            action: self.action.clone(),
            cancel: self.cancel.clone(),
            closable: self.closable,
        }
    }
}

impl<S: 'static> std::fmt::Debug for ToastSpec<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastSpec")
            .field("title", &self.title)
            .field("description", &self.description)
            .field("duration_secs", &self.duration_secs)
            .finish()
    }
}

impl<S: 'static> ToastSpec<S> {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            duration_secs: Some(DEFAULT_TOAST_SECS),
            action: None,
            cancel: None,
            closable: false,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn duration_secs(mut self, secs: f64) -> Self {
        self.duration_secs = Some(secs);
        self
    }

    pub fn persistent(mut self) -> Self {
        self.duration_secs = None;
        self
    }

    pub fn action(mut self, action: ToastAction<S>) -> Self {
        self.action = Some(action);
        self
    }

    pub fn cancel(mut self, cancel: ToastAction<S>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

/// Centered bottom Sonner-style toast host (`sonner.tsx`: `position="bottom-center"`, `width: 356`,
/// `radius: 14`).
pub struct Toasts<S: 'static> {
    stack: ToastStack<ToastSpec<S>>,
}

impl<S: 'static> Default for Toasts<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: 'static> Toasts<S> {
    pub fn new() -> Self {
        Self {
            stack: ToastStack::default(),
        }
    }

    /// Shows or updates a toast identified by `key`, returning its assigned numeric toast ID.
    pub fn show(&mut self, key: impl Into<String>, spec: ToastSpec<S>, now_secs: f64) -> u64 {
        let key = key.into();
        let secs = spec.duration_secs;
        let _ = self.stack.step(ToastEvent::Shown {
            key: key.clone(),
            options: spec,
            secs,
            now: now_secs,
        });
        self.stack
            .entries()
            .iter()
            .rev()
            .find(|e| e.key == key)
            .map(|e| e.id)
            .unwrap_or(0)
    }

    /// Dismisses a single toast by ID.
    pub fn dismiss(&mut self, id: u64, now_secs: f64) -> bool {
        let before = self.stack.entries().len();
        let _ = self.stack.step(ToastEvent::Dismissed {
            id: Some(id),
            now: now_secs,
        });
        if self.stack.entries().is_empty() {
            let _ = self.stack.step(ToastEvent::PointerLeft { now: now_secs });
        }
        self.stack.entries().len() != before
    }

    /// Dismisses every active toast.
    pub fn clear(&mut self, now_secs: f64) {
        let _ = self.stack.step(ToastEvent::Dismissed {
            id: None,
            now: now_secs,
        });
        let _ = self.stack.step(ToastEvent::PointerLeft { now: now_secs });
    }

    /// Advances timers up to `now_secs`, closing expired toasts. Returns `true` if any expired.
    pub fn tick(&mut self, now_secs: f64) -> bool {
        let before = self.stack.entries().len();
        let _ = self.stack.step(ToastEvent::Tick { now: now_secs });
        if self.stack.entries().is_empty() {
            let _ = self.stack.step(ToastEvent::PointerLeft { now: now_secs });
        }
        self.stack.entries().len() != before
    }

    /// Updates window visibility so hidden windows pause auto-dismiss timers like Sonner.
    pub fn set_window_visible(&mut self, visible: bool, now_secs: f64) {
        let page = if visible {
            ToastPage::Shown
        } else {
            ToastPage::Hidden
        };
        let _ = self.stack.step(ToastEvent::Page {
            page,
            now: now_secs,
        });
    }

    /// Next clock deadline (in seconds) at which a running toast expires.
    pub fn next_due(&self) -> Option<f64> {
        self.stack.next_due()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.entries().is_empty()
    }

    pub fn len(&self) -> usize {
        self.stack.entries().len()
    }

    pub fn entries(&self) -> &[ToastEntry<ToastSpec<S>>] {
        self.stack.entries()
    }

    /// Invokes the primary action of the frontmost toast (useful in headless tests and keyboard
    /// activation) and dismisses that toast.
    pub fn trigger_front_action(&self, state: &mut S, tx: &mut Tx, now_secs: f64) -> Option<u64> {
        let front = self.stack.front()?;
        let id = front.id;
        let handler = front.options.action.as_ref()?.handler.clone();
        handler(state, tx);
        let _ = now_secs;
        Some(id)
    }

    /// Renders the centered bottom `"Notifications"` region when at least one toast is active.
    pub fn view(
        &self,
        theme: &Theme,
        access: impl Fn(&mut S) -> (&mut Toasts<S>, f64) + Clone + 'static,
    ) -> Option<El> {
        if self.stack.entries().is_empty() {
            return None;
        }
        let kit = theme.kit();
        let mut cards = Vec::new();
        for entry in self.stack.entries().iter().rev().take(VISIBLE) {
            let id = entry.id;
            let spec = &entry.options;
            let mut toast = Toast::new(entry.key.clone(), spec.title.clone());
            if let Some(desc) = &spec.description {
                toast = toast.description(desc.clone());
            }
            if let Some(cancel) = &spec.cancel {
                let handler = cancel.handler.clone();
                let access_cl = access.clone();
                toast = toast.action(kit, &cancel.label, cancel.primary, move |s: &mut S, tx| {
                    handler(s, tx);
                    let (toasts, now) = access_cl(s);
                    toasts.dismiss(id, now);
                });
            }
            if let Some(action) = &spec.action {
                let handler = action.handler.clone();
                let access_cl = access.clone();
                toast = toast.action(kit, &action.label, action.primary, move |s: &mut S, tx| {
                    handler(s, tx);
                    let (toasts, now) = access_cl(s);
                    toasts.dismiss(id, now);
                });
            }
            if spec.closable {
                let access_cl = access.clone();
                toast = toast.on_close(move |s: &mut S| {
                    let (toasts, now) = access_cl(s);
                    toasts.dismiss(id, now);
                });
            }
            cards.push(kit_toast::build(toast, theme));
        }

        let hover_access = access;
        let region = col()
            .key("toast-region")
            .role("region")
            .label("Notifications")
            .gap(rune_kit::toast_stack::GAP)
            .items_center()
            .on_hover_change(move |s: &mut S, on, _| {
                let (toasts, now) = hover_access(s);
                let ev = if on {
                    ToastEvent::PointerOver { now }
                } else {
                    ToastEvent::PointerLeft { now }
                };
                let _ = toasts.stack.step(ev);
            })
            .children(cards);

        Some(
            col()
                .key("toast-overlay")
                .pin(None, Some(0.0), Some(16.0), Some(0.0))
                .items_center()
                .child(region),
        )
    }
}
