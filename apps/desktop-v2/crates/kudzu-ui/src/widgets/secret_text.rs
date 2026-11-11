//! Secrets in clipboard history: hidden until revealed (`components/app/clipboard/secret.tsx`).
//! A concealed row shows dots about as long as the text; a concealed preview shows a cover
//! with the reason. Neither puts the content in the element tree until it is revealed: what
//! is not shown is not built, so no tree dump, accessibility client or screenshot of the cover
//! can read it (CLP-17). Settings ▸ Clipboard ▸ Show secrets turns concealment off (`concealed`
//! false).
//!
//! The preview reveals while the pointer is over it or keyboard focus is on it, and conceals
//! again when both leave, as the Tauri cover does (`group-hover/secret`,
//! `group-focus-visible/secret`). It is one keyed, focusable element in both states, so focus
//! stays put when the content swaps in. Rune reports neither focus nor blur to a view, so the
//! window's poll (which runs at the start of every frame, with the focus current) feeds
//! [`Reveal::poll`]; hover comes from the cover's `on_hover_change`. The cover keeps the hidden
//! content's height (D21), measured by the caller ([`text_height`] for text), so revealing
//! moves nothing.

use rune::{col, el, icon, text, Color, El, TextStyle, Tx};

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

/// `min-h-28`.
pub const MIN_HEIGHT: f32 = 112.0;

/// What reveals a concealed preview: the pointer over it, or keyboard focus on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reveal {
    hovered: bool,
    focused: bool,
}

impl Reveal {
    pub fn shown(&self) -> bool {
        self.hovered || self.focused
    }
    /// From the cover's `on_hover_change`.
    pub fn hover(&mut self, on: bool) {
        self.hovered = on;
    }
    /// From the window's poll: whether focus is on the cover keyed `key`. True when the
    /// reveal changed (the view rebuilds).
    pub fn poll(&mut self, key: &str, tx: &Tx) -> bool {
        let focused = tx
            .focused()
            .is_some_and(|f| f == key || f.ends_with(&format!("/{key}")));
        let changed = focused != self.focused;
        self.focused = focused;
        changed
    }
}

/// The height text set in `style` takes at `width`, for the cover to keep. Measured without
/// building an element, so the text never reaches the tree.
pub fn text_height(content: &str, style: TextStyle, width: f32) -> f32 {
    rune::text::measure(content, &[], style, Some(width)).1
}

/// The preview form (`Concealed`), keyed `key`: `content` while revealed, else a well-depth
/// cover with an eye-off tile, the reason ("Looks like a password") and "Hover to show", as tall
/// as the content (`content_height`, at least 112). `on_hover` feeds [`Reveal::hover`].
pub fn cover<S: 'static>(
    theme: &Theme,
    key: &str,
    reason: &str,
    revealed: bool,
    content_height: f32,
    content: impl FnOnce() -> El,
    on_hover: impl Fn(&mut S, bool) + 'static,
) -> El {
    let c = &theme.c;
    let e = col()
        .key(key.to_owned())
        .label(format!(
            "Hidden: {}. Hover or focus to show.",
            reason.to_lowercase()
        ))
        .focusable(true)
        .focus_ring(2.0, 0.0, c.ring)
        .w_full()
        .min_h(MIN_HEIGHT.max(content_height))
        .radius(radius::XL)
        .on_hover_change(move |s: &mut S, on, _| on_hover(s, on));
    let fade = |d| {
        rune::Transition::opacity().springs(rune::SpringPair::new(
            motion::soft_spring(d),
            motion::soft_spring(d),
        ))
    };
    if revealed {
        return e.child(
            col()
                .key("secret-shown")
                .w_full()
                .transition(fade(motion::MODERATE))
                .child(content()),
        );
    }
    let mut face = col()
        .key("secret-cover")
        .w_full()
        .grow(1.0)
        .center()
        .gap(6.0)
        .radius(radius::XL)
        .fill(c.foreground.alpha(0.025))
        .transition(fade(motion::MODERATE))
        .a11y_hidden()
        .child(
            el().size(36.0, 36.0)
                .center()
                .radius(radius::XL)
                .fill(c.foreground.alpha(0.06))
                .child(icon(&icons::EYE_OFF, 16.0).color(c.muted_foreground)),
        )
        .child(
            text(reason.to_owned())
                .font(theme.ty.body_medium)
                .color(c.foreground),
        )
        .child(
            text("Hover to show")
                .font(TextStyle::new(12.0, 400.0).lh(16.0))
                .color(c.faint),
        );
    for s in &theme.depth.well {
        face = face.box_shadow(*s);
    }
    e.h(MIN_HEIGHT.max(content_height)).child(face)
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
    fn dump<S>(rt: &Runtime<S>) -> String {
        let elements: Vec<String> = rt
            .elements()
            .iter()
            .filter_map(|e| e.text.clone())
            .collect();
        format!("{elements:?} {}", rt.accessibility_json())
    }

    #[test]
    fn a_concealed_row_holds_only_dots() {
        fn view(revealed: &bool, _: &mut Cx<bool>) -> El {
            let theme = Theme::light();
            row_secret(SECRET, true, *revealed, theme.ty.body, theme.c.foreground)
        }
        for revealed in [false, true] {
            let mut rt = Runtime::new(revealed, view, (400.0, 60.0), Env::new(true, 1.0));
            rt.frame(0.0);
            assert_eq!(dump(&rt).contains(SECRET), revealed, "{}", dump(&rt));
        }
    }

    struct Preview {
        reveal: Reveal,
    }

    fn preview(s: &Preview, _: &mut Cx<Preview>) -> El {
        let theme = Theme::light();
        let height = text_height(SECRET, theme.ty.body, 300.0);
        col()
            .w(300.0)
            .child(cover(
                theme,
                "secret",
                "Looks like a password",
                s.reveal.shown(),
                height.max(140.0),
                || text(SECRET).font(theme.ty.body).h(140.0),
                |s: &mut Preview, on| s.reveal.hover(on),
            ))
            .child(rune::el().key("other").size(40.0, 20.0).focusable(true))
    }

    fn poll(s: &mut Preview, _: f64, tx: &mut Tx) -> bool {
        s.reveal.poll("secret", tx)
    }

    fn settle(rt: &mut Runtime<Preview>) {
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
    }

    /// Focus reveals it and blur conceals it again; the cover is the content's height either way,
    /// and the concealed tree never holds the secret.
    #[test]
    fn focus_reveals_and_blur_conceals_without_moving_anything() {
        let mut rt = Runtime::new(
            Preview {
                reveal: Reveal::default(),
            },
            preview,
            (400.0, 300.0),
            Env::new(true, 1.0),
        );
        rt.poll = Some(poll);
        settle(&mut rt);
        let rect = |rt: &Runtime<Preview>| {
            rt.elements()
                .into_iter()
                .find(|e| e.key.as_deref() == Some("secret"))
                .unwrap()
                .target
        };
        let covered = rect(&rt);
        assert!(!dump(&rt).contains(SECRET), "{}", dump(&rt));
        assert!(dump(&rt).contains("Hidden: looks like a password. Hover or focus to show."));
        assert_eq!(covered[3], 140.0, "the cover keeps the content's height");

        rt.update(|_, tx| tx.focus("secret", true));
        settle(&mut rt);
        assert!(rt.state.reveal.shown());
        assert!(dump(&rt).contains(SECRET));
        assert_eq!(rect(&rt), covered, "revealing moves nothing");

        rt.update(|_, tx| tx.focus("other", true));
        settle(&mut rt);
        assert!(!rt.state.reveal.shown());
        assert!(!dump(&rt).contains(SECRET), "blur conceals it again");
    }
}
