//! A settings group and its rows (`Group` and `Row` in `components/app/main/settings.tsx`):
//! a 13 point semibold title with an optional `text-xs` line, over a `surface` card (`rounded-xl`,
//! `px-4`, `shadow-card`) whose rows are divided by hairlines. A row is at least 56 tall
//! (`min-h-14 py-3`), a medium 13 point title over its description, the control on the right;
//! a `dimmed` row fades with the switch it depends on.

use rune::{col, el, row, text, El, TextStyle};

use crate::theme::{motion, radius, Theme};

pub struct SettingsCard {
    title: String,
    description: Option<String>,
    rows: Vec<El>,
}

impl SettingsCard {
    pub fn new(title: impl Into<String>) -> SettingsCard {
        SettingsCard {
            title: title.into(),
            description: None,
            rows: Vec::new(),
        }
    }
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
    pub fn row(mut self, r: El) -> Self {
        self.rows.push(r);
        self
    }

    pub fn build(self, theme: &Theme) -> El {
        let c = &theme.c;
        let mut head = col().key("head").px(4.0).child(
            text(self.title.clone())
                .role("heading")
                .label(self.title.clone())
                .font(TextStyle::new(13.0, 600.0).lh(20.0))
                .color(c.foreground),
        );
        if let Some(d) = self.description {
            head = head.child(text(d).font(xs()).color(c.muted_foreground).wrap());
        }
        let mut card = col()
            .key("card")
            .w_full()
            .px(16.0)
            .radius(radius::XL)
            .fill(c.surface);
        for s in &theme.depth.card {
            card = card.box_shadow(*s);
        }
        let n = self.rows.len();
        for (i, r) in self.rows.into_iter().enumerate() {
            card = card.child(r);
            if i + 1 < n {
                card = card.child(
                    el().key(format!("divider{i}"))
                        .h(1.0)
                        .w_full()
                        .fill(c.hairline)
                        .a11y_hidden(),
                );
            }
        }
        col()
            .role("region")
            .label(self.title)
            .w_full()
            .gap(8.0)
            .child(head)
            .child(card)
    }
}

fn xs() -> TextStyle {
    TextStyle::new(12.0, 400.0).lh(16.0)
}

/// One row: title, description, and the controls on the right.
pub fn setting_row(
    theme: &Theme,
    title: &str,
    description: Option<&str>,
    controls: Vec<El>,
    dimmed: bool,
) -> El {
    let c = &theme.c;
    let mut words = col()
        .key("words")
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .opacity(if dimmed { 0.5 } else { 1.0 })
        .opacity_motion(motion::soft(motion::MODERATE))
        .child(
            text(title.to_owned())
                .font(theme.ty.body_medium)
                .color(c.foreground),
        );
    if let Some(d) = description {
        words = words.child(
            text(d.to_owned())
                .font(xs())
                .color(c.muted_foreground)
                .wrap(),
        );
    }
    row()
        .w_full()
        .min_h(56.0)
        .py(12.0)
        .gap(24.0)
        .center_y()
        .child(words)
        .child(row().key("controls").gap(8.0).center_y().children(controls))
}
