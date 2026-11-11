//! An empty pane (`EditorEmpty` in `snippet-editor.tsx`, and the clipboard and Activity empty
//! views): a 48 point glyph tile (`rounded-2xl`, `foreground/4%`, a hairline ring), a medium
//! title over a `text-xs` muted line, and an optional action, centred, rising in one after
//! another (`stagger`).

use rune::{col, el, icon, text, El, Icon, Shadow, TextStyle};

use crate::theme::{motion, radius, Theme};

pub struct EmptyState {
    glyph: &'static Icon,
    title: String,
    description: Option<String>,
    action: Option<El>,
}

impl EmptyState {
    pub fn new(glyph: &'static Icon, title: impl Into<String>) -> EmptyState {
        EmptyState {
            glyph,
            title: title.into(),
            description: None,
            action: None,
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    /// A button under the text (built by the caller, e.g. "New snippet ⌘N").
    pub fn action(mut self, e: El) -> Self {
        self.action = Some(e);
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        let c = &theme.c;
        let tile = glyph_tile(theme, self.glyph);
        let mut words = col().key("words").gap(4.0).items_center().child(
            text(self.title.clone())
                .key("title")
                .role("heading")
                .label(self.title)
                .font(theme.ty.body_medium)
                .color(c.foreground),
        );
        if let Some(d) = self.description {
            words = words.child(
                text(d)
                    .key("description")
                    .font(TextStyle::new(12.0, 400.0).lh(16.0))
                    .color(c.muted_foreground)
                    .wrap(),
            );
        }
        let mut parts = vec![tile, words];
        parts.extend(self.action);
        col()
            .key("empty")
            .w_full()
            .h_full()
            .center()
            .gap(16.0)
            .padding(32.0)
            .children(parts.into_iter().enumerate().map(|(i, p)| {
                p.transition(crate::motion::rise())
                    .enter_delay(motion::STAGGER[i.min(5)])
            }))
    }
}

/// The 48 point glyph tile (`size-12 rounded-2xl bg-foreground/[0.04] shadow-hairline`, a
/// `size-5` muted glyph).
pub fn glyph_tile(theme: &Theme, glyph: &'static Icon) -> El {
    let c = &theme.c;
    el().key("glyph")
        .size(48.0, 48.0)
        .center()
        .radius(radius::XL2)
        .fill(c.foreground.alpha(0.04))
        .box_shadow(Shadow {
            spread: 1.0,
            color: c.hairline,
            ..Shadow::default()
        })
        .a11y_hidden()
        .child(icon(glyph, 20.0).color(c.muted_foreground))
}
