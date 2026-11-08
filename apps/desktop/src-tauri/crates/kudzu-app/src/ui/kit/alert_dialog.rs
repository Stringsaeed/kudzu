//! `components/ui/alert-dialog.tsx` (Radix AlertDialog): the dialog's scrim and motion, a
//! header (optional media tile, title, description) and a footer with Cancel and the action.
//! Unlike a dialog, a press outside doesn't close it; Escape and Cancel do. Focus starts on
//! Cancel and returns to the opener when it closes. Either button closes it first, then runs
//! its handler.
//!
//! ```ignore
//! AlertDialog::new("clear-history")
//!     .open(s.settings_page.confirm)
//!     .on_open_change(|s: &mut MainUi, open, _| s.settings_page.confirm = open)
//!     .title("Clear clipboard history?")
//!     .description("This deletes every item that isn't pinned. It can't be undone.")
//!     .cancel("Cancel", |_, _| {})
//!     .action("Clear history", Variant::Destructive, |s: &mut MainUi, tx| ..)
//!     .build(cx, &mut layer);
//! ```

use std::rc::Rc;

use rune::{Cx, El, Tx, col, el, row, text};

use super::button::{Button, Variant};
use super::dialog::{self, Placement};
use super::popover::{self, Handler, OpenChange};
use crate::ui::theme::{radius, text as ty, weight, Utilities, WeightOf};
use crate::ui::theme_for;

/// `size`: `default` (`sm:max-w-sm`, start-aligned) or `sm` (`max-w-xs`, centred, two-column footer).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AlertSize {
    #[default]
    Default,
    Sm,
}

pub struct AlertDialog<S> {
    key: String,
    open: bool,
    change: Option<OpenChange<S>>,
    size: AlertSize,
    media: Option<El>,
    title: String,
    description: Option<String>,
    cancel: Option<(String, Handler<S>)>,
    action: Option<(String, Variant, Handler<S>)>,
}

impl<S: 'static> AlertDialog<S> {
    pub fn new(key: impl Into<String>) -> Self {
        AlertDialog { key: key.into(), open: false, change: None, size: AlertSize::Default, media: None, title: String::new(), description: None, cancel: None, action: None }
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn on_open_change(mut self, f: impl Fn(&mut S, bool, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }
    pub fn size(mut self, s: AlertSize) -> Self {
        self.size = s;
        self
    }
    /// `AlertDialogMedia`: a `size-10 rounded-md bg-muted` tile (put a 24 pt icon in it).
    pub fn media(mut self, e: El) -> Self {
        self.media = Some(e);
        self
    }
    pub fn title(mut self, t: impl Into<String>) -> Self {
        self.title = t.into();
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    /// `AlertDialogCancel` (`variant="outline"`).
    pub fn cancel(mut self, label: impl Into<String>, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.cancel = Some((label.into(), Rc::new(f)));
        self
    }
    /// `AlertDialogAction` (`variant="default"`, or `destructive`).
    pub fn action(mut self, label: impl Into<String>, variant: Variant, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.action = Some((label.into(), variant, Rc::new(f)));
        self
    }

    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) {
        if !self.open {
            return;
        }
        let th = theme_for(cx);
        let t = &th.t;
        let sm = self.size == AlertSize::Sm;
        let title = text(self.title.clone()).font(ty::LG.weight_of(weight::MEDIUM)).color(t.popover_foreground).wrap();
        // `text-balance`: lines of even length. Measure the description on one line, then wrap it
        // at that width over the line count the panel's width forces.
        let measure_key = format!("{}-desc-measure", self.key);
        let one_line = cx.layout_size(&measure_key).map(|s| s[0]);
        let inner = if sm { 320.0 } else { 384.0 } - 32.0 - 2.0;
        let desc = self.description.clone().map(|d| {
            let mut e = text(d.clone()).font(ty::SM).color(t.muted_foreground).wrap();
            if let Some(w) = one_line.filter(|w| *w > inner) {
                let lines = (w / inner).ceil();
                e = e.max_w((w / lines).ceil() + 8.0);
            }
            col().child(e).child(
                text(d).key(measure_key.clone()).font(ty::SM).absolute(0.0, 0.0).opacity(0.0).inert().a11y_hidden(),
            )
        });
        let mut words = col().gap(6.0).child(if sm { row().w_full().justify_center().child(title) } else { title });
        if let Some(d) = desc {
            words = words.child(if sm { row().w_full().justify_center().child(d) } else { d });
        }
        let media = self.media.map(|m| el().size(40.0, 40.0).radius(radius::MD).fill(t.muted).center().mb(8.0).child(m));
        let header = match (media, sm) {
            (Some(m), false) => row().key("header").gap(16.0).items_start().child(m).child(words.grow(1.0).shrink()),
            (Some(m), true) => col().key("header").gap(6.0).items_center().child(m).child(words),
            (None, true) => col().key("header").items_center().child(words),
            (None, false) => col().key("header").child(words),
        };
        let close = self.change.clone();
        let button = |label: String, variant: Variant, f: Handler<S>, id: String| {
            let close = close.clone();
            let mut b = Button::new(label).variant(variant).id(id).on_press_tx(move |s: &mut S, tx| {
                if let Some(c) = &close {
                    c(s, false, tx);
                }
                f(s, tx);
            });
            if sm {
                b = b.full_width();
            }
            b.build(th)
        };
        let mut buttons = Vec::new();
        if let Some((l, f)) = self.cancel {
            buttons.push(button(l, Variant::Outline, f, format!("{}-cancel", self.key)));
        }
        if let Some((l, v, f)) = self.action {
            buttons.push(button(l, v, f, format!("{}-action", self.key)));
        }
        let footer = if sm {
            dialog::footer(cx, 16.0, radius::XL, buttons.into_iter().map(|b| el().grow(1.0).child(b.w_full())))
        } else {
            dialog::footer(cx, 16.0, radius::XL, buttons)
        };
        let panel = col()
            .role("alertdialog")
            .label(self.title.clone())
            .description(self.description.clone().unwrap_or_default())
            .radius(radius::XL)
            .fill(t.popover)
            .padding(16.0)
            .gap(16.0)
            .box_shadow(popover::ring(th))
            .elevate(&t.elev_panel)
            .child(header)
            .child(footer);
        let close = self.change.map(|f| Rc::new(move |s: &mut S| f(s, false, &mut Tx::default())) as Rc<dyn Fn(&mut S)>);
        let width = if sm { 320.0 } else { 384.0 };
        dialog::mount(cx, layer, &self.key, panel, width, Placement::Center, false, close);
    }
}
