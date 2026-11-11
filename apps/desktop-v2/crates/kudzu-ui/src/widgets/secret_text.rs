//! Secrets in clipboard history: hidden until revealed (`components/app/clipboard/secret.tsx`).
//! A concealed row shows dots about as long as the text; a concealed preview shows a cover
//! with the reason. Neither puts the content in the element tree until it is revealed: what
//! is not shown is not built, so no tree dump, accessibility client or screenshot of the cover
//! can read it (CLP-17). The window reveals on hover or focus and sets `revealed`; Settings ▸
//! Clipboard ▸ Show secrets turns concealment off (`concealed` false).

use rune::{col, el, icon, text, Color, El, TextStyle};

use crate::theme::{icons, motion, radius, Theme};

/// Dots standing in for hidden text, about as long as the text (8 to 18).
pub fn dots(chars: usize) -> String {
    "•".repeat(chars.clamp(8, 18))
}

/// The row form (`RowSecret`): the content in `style`, or dots while concealed and not
/// revealed. The content fades in on reveal (`animate-fade`).
pub fn row_secret(
    content: &str,
    concealed: bool,
    revealed: bool,
    style: TextStyle,
    ink: Color,
) -> El {
    if concealed && !revealed {
        return text(dots(content.chars().count()))
            .key("concealed")
            .font(style)
            .color(ink)
            .truncate()
            .a11y_hidden();
    }
    text(content.to_owned())
        .key("revealed")
        .font(style)
        .color(ink)
        .truncate()
        .transition(rune::Transition::opacity().springs(rune::SpringPair::new(
            motion::soft_spring(motion::FAST),
            motion::soft_spring(motion::FAST),
        )))
}

/// The preview form (`Concealed`): `content` while revealed, else a 112 point (`min-h-28`)
/// well-depth cover with an eye-off tile, the reason ("Looks like a password") and "Hover to
/// show". Focusable, so the keyboard reveals it too (`on_reveal`).
pub fn cover<S: 'static>(
    theme: &Theme,
    reason: &str,
    revealed: bool,
    content: impl FnOnce() -> El,
    on_reveal: impl Fn(&mut S, bool) + Clone + 'static,
) -> El {
    if revealed {
        return col()
            .key("secret-shown")
            .w_full()
            .transition(rune::Transition::opacity().springs(rune::SpringPair::new(
                motion::soft_spring(motion::MODERATE),
                motion::soft_spring(motion::MODERATE),
            )))
            .on_hover_change({
                let f = on_reveal.clone();
                move |s: &mut S, on, _| f(s, on)
            })
            .child(content());
    }
    let c = &theme.c;
    let (hover, press) = (on_reveal.clone(), on_reveal);
    let mut e = col()
        .key("secret-cover")
        .role("button")
        .label(format!(
            "Hidden: {}. Hover or focus to show.",
            reason.to_lowercase()
        ))
        .focusable(true)
        .focus_ring(2.0, 0.0, c.ring)
        .w_full()
        .min_h(112.0)
        .center()
        .gap(6.0)
        .radius(radius::XL)
        .fill(c.foreground.alpha(0.025))
        .on_hover_change(move |s: &mut S, on, _| hover(s, on))
        .on_press(move |s: &mut S| press(s, true))
        .child(
            el().size(36.0, 36.0)
                .center()
                .radius(radius::XL)
                .fill(c.foreground.alpha(0.06))
                .a11y_hidden()
                .child(icon(&icons::EYE_OFF, 16.0).color(c.muted_foreground)),
        )
        .child(
            text(reason.to_owned())
                .font(theme.ty.body_medium)
                .color(c.foreground)
                .a11y_hidden(),
        )
        .child(
            text("Hover to show")
                .font(TextStyle::new(12.0, 400.0).lh(16.0))
                .color(c.faint)
                .a11y_hidden(),
        );
    for s in &theme.depth.well {
        e = e.box_shadow(*s);
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Runtime};

    const SECRET: &str = "hunter2-correct-horse";

    #[test]
    fn dots_are_about_as_long_as_the_text() {
        assert_eq!(dots(3).chars().count(), 8);
        assert_eq!(dots(12).chars().count(), 12);
        assert_eq!(dots(400).chars().count(), 18);
    }

    /// Everything the tree holds: elements and the accessibility tree.
    fn dump(revealed: bool, row: bool) -> String {
        fn view(s: &(bool, bool), _: &mut Cx<(bool, bool)>) -> El {
            let theme = Theme::light();
            let (revealed, row) = *s;
            if row {
                return row_secret(SECRET, true, revealed, theme.ty.body, theme.c.foreground);
            }
            cover(
                theme,
                "Looks like a password",
                revealed,
                || text(SECRET),
                |_: &mut (bool, bool), _| {},
            )
        }
        let mut rt = Runtime::new((revealed, row), view, (400.0, 200.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let elements: Vec<String> = rt
            .elements()
            .iter()
            .filter_map(|e| e.text.clone())
            .collect();
        format!("{elements:?} {}", rt.accessibility_json())
    }

    #[test]
    fn a_secret_is_not_in_the_tree_until_revealed() {
        for row in [true, false] {
            assert!(
                !dump(false, row).contains(SECRET),
                "row {row}: {}",
                dump(false, row)
            );
            assert!(dump(true, row).contains(SECRET), "row {row}");
        }
        assert!(dump(false, false).contains("Hidden: looks like a password"));
    }
}
