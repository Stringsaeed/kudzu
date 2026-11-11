//! RUNE-GAP(#573: rune-kit `Dialog` takes its radius, padding and shadow from `Kit`): delete this
//! module and build `rune_kit::Dialog` once it does.
//!
//! shadcn's `Dialog` and `AlertDialog` content (`rounded-xl`, 14; Kudzu's own dialogs
//! `rounded-2xl`, 18, with `shadow-float` and no ring), built from Rune core on rune-kit's
//! modal layer. Verdict in `PARITY_MATRIX.md`: **wrapper** (D6, after S06): rune-kit's panel is
//! a child of the layer it pushes, with fixed 6 point corners, so no decoration reaches it. The
//! structure is rune-kit's (`dialog.rs` at the pin): a scrim, a centred panel named by its
//! title, a header with the close button, the body; focus, dismissal and inertness are Rune's
//! modal stack.

use rune::{col, el, row, text, El, Shadow, TextStyle};
use rune_kit::{ControlSize, IconButton, IconButtonVariant};

use crate::theme::{radius, Theme};

type Close = Box<dyn FnOnce(El) -> El>;

pub struct Dialog {
    key: String,
    title: String,
    description: Option<String>,
    body: Vec<El>,
    open: bool,
    outside: bool,
    width: f32,
    kudzu: bool,
    sample: Option<String>,
    close: Option<Close>,
    button: Option<Close>,
}

impl Dialog {
    pub fn new(key: impl Into<String>, title: impl Into<String>) -> Dialog {
        Dialog {
            key: key.into(),
            title: title.into(),
            description: None,
            body: Vec::new(),
            open: false,
            outside: true,
            width: 384.0,
            kudzu: false,
            sample: None,
            close: None,
            button: None,
        }
    }
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn child(mut self, e: El) -> Self {
        self.body.push(e);
        self
    }
    /// `max-w-*`: shadcn's default is `sm:max-w-sm` (384).
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    /// Kudzu's own dialogs: `rounded-2xl` (18), `shadow-float`, no ring.
    pub fn kudzu(mut self) -> Self {
        self.kudzu = true;
        self
    }
    pub fn click_outside(mut self, close: bool) -> Self {
        self.outside = close;
        self
    }
    /// Keys the panel for `cargo xtask parity` (galleries).
    pub fn sample(mut self, key: impl Into<String>) -> Self {
        self.sample = Some(key.into());
        self
    }
    pub fn on_close<S: 'static>(mut self, f: impl Fn(&mut S) + Clone + 'static) -> Self {
        let button = f.clone();
        self.button = Some(Box::new(move |e| e.on_press(button)));
        self.close = Some(Box::new(move |e| e.modal(true, f)));
        self
    }

    /// The panel's radius.
    pub fn radius(&self) -> f32 {
        if self.kudzu {
            radius::XL2
        } else {
            radius::XL
        }
    }

    pub fn build(self, theme: &Theme, layer: &mut Vec<El>) {
        let k = theme.kit();
        let c = &theme.c;
        if !self.open {
            return;
        }
        let r = self.radius();
        let Some(close) = self.close else { return };
        let mut dismiss = IconButton::new(k.glyph.close)
            .variant(IconButtonVariant::Ghost)
            .control_size(ControlSize::Regular)
            .label("Close")
            .build(k)
            .key("close");
        if let Some(b) = self.button {
            dismiss = b(dismiss);
        }
        let mut title = col().grow(1.0).gap(4.0).child(
            text(self.title.clone())
                .role("heading")
                .label(self.title.clone())
                .font(TextStyle::new(16.0, 500.0).lh(20.0))
                .color(c.foreground),
        );
        if let Some(d) = &self.description {
            title = title.child(
                text(d.clone())
                    .font(theme.ty.input)
                    .color(c.muted_foreground)
                    .wrap(),
            );
        }
        let mut panel = col()
            .key(
                self.sample
                    .unwrap_or_else(|| format!("{}-dialog", self.key)),
            )
            .role("dialog")
            .label(self.title)
            .description(self.description.unwrap_or_default())
            .w_full()
            .max_w(self.width)
            .padding(16.0)
            .gap(16.0)
            .radius(r)
            .fill(c.popover)
            .child(
                row()
                    .key("header")
                    .w_full()
                    .gap(8.0)
                    .items_start()
                    .child(title)
                    .child(dismiss.size(24.0, 24.0)),
            )
            .children(self.body)
            .token_motion(k.motion.overlay);
        let depth: &[Shadow] = if self.kudzu {
            &theme.depth.float
        } else {
            &theme.depth.panel
        };
        for s in depth {
            panel = panel.box_shadow(*s);
        }
        if !self.kudzu {
            panel = panel.box_shadow(Shadow {
                spread: 1.0,
                color: c.foreground.alpha(0.1),
                ..Shadow::default()
            });
        }
        let panel = close(panel).modal_click_outside(self.outside);
        layer.push(
            el().key(format!("{}-scrim", self.key))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .fill(k.c.scrim)
                .token_motion(k.motion.overlay)
                .a11y_hidden(),
        );
        layer.push(
            el().key(format!("{}-layer", self.key))
                .absolute(0.0, 0.0)
                .w_full()
                .h_full()
                .padding(16.0)
                .center()
                .child(panel),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radii_are_the_tauri_ones() {
        assert_eq!(Dialog::new("d", "T").radius(), 14.0);
        assert_eq!(Dialog::new("d", "T").kudzu().radius(), 18.0);
    }
}
