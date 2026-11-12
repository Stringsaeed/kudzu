//! A link: host as the title, the full URL in a well, and one button to open it. No page is
//! fetched, so there is no thumbnail.

use kudzu_core::clipboard::facts::describe;
use rune::{col, icon, row, text, El, TextStyle};

use super::{well, Body, Hooks};
use crate::theme::{icons, Theme};
use crate::widgets::kit_ext::button::{Button, Size, Variant};

pub fn is_email(url: &str) -> bool {
    url.trim().to_lowercase().starts_with("mailto:")
}

pub fn view<S: 'static>(theme: &Theme, b: &Body, hooks: Hooks<S>) -> El {
    let c = &theme.c;
    let url = b.entry.text.as_deref().unwrap_or("").trim().to_owned();
    let email = is_email(&url);
    let title_size = if b.large { 20.0 } else { 17.0 };
    col()
        .key("clip-link-body")
        .w_full()
        .gap(16.0)
        .child(
            row()
                .gap(12.0)
                .center_y()
                .child(
                    row()
                        .size(44.0, 44.0)
                        .center()
                        .radius(14.0)
                        .fill(c.foreground.alpha(0.05))
                        .border(1.0, c.hairline)
                        .child(icon(&icons::GLOBE, 20.0).color(c.muted_foreground)),
                )
                .child(
                    col()
                        .min_w(0.0)
                        .child(
                            text(describe(b.entry).title)
                                .font(TextStyle::new(title_size, 600.0))
                                .color(c.foreground)
                                .truncate(),
                        )
                        .child(
                            text(if email { "Email address" } else { "Web link" })
                                .font(theme.ty.control_xs)
                                .color(c.muted_foreground),
                        ),
                ),
        )
        .child(
            well(theme).px(14.0).py(12.0).child(
                text(url)
                    .font(TextStyle {
                        line_height: 12.5 * 1.625,
                        ..TextStyle::new(12.5, 400.0).family(theme.ty.mono.family)
                    })
                    .color(c.foreground.alpha(0.85))
                    .wrap_anywhere()
                    .selectable(true)
                    .w_full(),
            ),
        )
        .child(
            row().child(
                Button::new(if email {
                    "Write email"
                } else {
                    "Open in browser"
                })
                .variant(Variant::Secondary)
                .size(Size::Sm)
                .icon(&icons::EXTERNAL_LINK)
                .on_press_tx(hooks.open_link)
                .build(theme),
            ),
        )
}

#[cfg(test)]
mod tests {
    use super::is_email;

    #[test]
    fn mailto_links_are_emails() {
        assert!(is_email(" MAILTO:a@b.c "));
        assert!(!is_email("https://a.b"));
    }
}
