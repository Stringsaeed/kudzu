//! Secrets in clipboard history (passwords, keys, tokens): hidden until the pointer rests on
//! them or keyboard focus reaches them, unless Settings ▸ Clipboard ▸ Show secrets is on
//! (`secret.tsx`).
//!
//! Hiding is paint only, so it costs no commit and scrolling a list never replays it. The
//! covered content keeps its layout (nothing jumps on reveal) but is never selectable and is
//! hidden from assistive technology: an assistive client reads only "Hidden: <reason>".

use kudzu_core::clipboard::facts::{concealed, secret_reason};
use kudzu_core::clipboard::ClipEntry;
use rune::{col, icon, row, text, El, Tx};

use crate::theme::{icons, Theme};

/// Dots standing in for hidden text, about as long as the text (8 to 18).
pub fn dots(length: usize) -> String {
    "•".repeat(length.clamp(8, 18))
}

/// Whether to hide `e`'s content: it is a secret and "Show secrets" is off.
pub fn is_hidden(e: &ClipEntry, show_secrets: bool) -> bool {
    concealed(e, show_secrets)
}

/// What marking an entry as a secret (or not) does, and what Undo puts back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub title: &'static str,
    pub description: &'static str,
    /// The override to store: `None` clears it, because Kudzu's own detection already agrees.
    pub next: Option<bool>,
    /// The override before, for Undo.
    pub before: Option<bool>,
}

pub fn change(e: &ClipEntry, secret: bool) -> Change {
    let detected = e.secret.is_some();
    let (title, description) = if secret {
        (
            "Hidden as a secret",
            "It stays hidden until you hover over it, and AI agents can't see it.",
        )
    } else {
        (
            "Marked as not a secret",
            "It's shown normally, and AI agents can read it if you share clipboard history with them.",
        )
    };
    Change {
        title,
        description,
        next: (secret != detected).then_some(secret),
        before: e.secret_override,
    }
}

/// The accessible name of covered content.
pub fn cover_label(e: &ClipEntry) -> String {
    format!(
        "Hidden: {}. Hover or focus to show.",
        secret_reason(e).to_lowercase()
    )
}

/// Covers `content` (a preview body) while it is a secret: the content is laid out and
/// invisible, the cover shows the reason, and a hover or focus swaps them.
pub fn cover(theme: &Theme, e: &ClipEntry, content: El) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    let overlay = col()
        .key("clip-concealed-cover")
        .absolute(0.0, 0.0)
        .w_full()
        .h_full()
        .center()
        .gap(6.0)
        .radius(14.0)
        .fill(c.foreground.alpha(0.025))
        .group_opacity(1.0, 0.0)
        .a11y_hidden()
        .child(
            row()
                .size(36.0, 36.0)
                .center()
                .radius(14.0)
                .fill(c.foreground.alpha(0.06))
                .child(icon(&icons::EYE_OFF, 16.0).color(c.muted_foreground)),
        )
        .child(
            text(secret_reason(e).to_owned())
                .font(ty.body_medium)
                .color(c.foreground),
        )
        .child(text("Hover to show").font(ty.control_xs).color(c.faint));
    col()
        .key("clip-concealed")
        .role("group")
        .label(cover_label(e))
        .focusable(true)
        .hover_group_focus()
        .w_full()
        .min_h(112.0)
        .radius(14.0)
        .child(
            col()
                .w_full()
                .group_opacity(0.0, 1.0)
                .a11y_hidden()
                .inert()
                .child(content),
        )
        .child(overlay)
}

/// The heading's "Secret" pill with its one-click "Not a secret?" correction.
pub fn badge<S: 'static>(theme: &Theme, not_a_secret: fn(&mut S, &mut Tx)) -> El {
    let c = &theme.c;
    let ty = &theme.ty;
    row()
        .gap(6.0)
        .center_y()
        .min_w(0.0)
        .child(
            row()
                .h(18.0)
                .gap(4.0)
                .center_y()
                .px(6.0)
                .radius(6.0)
                .fill(c.warning.alpha(0.12))
                .child(icon(&icons::KEY_ROUND, 12.0).color(c.warning))
                .child(text("Secret").font(ty.meta_11).color(c.warning)),
        )
        .child(
            row()
                .key("clip-not-a-secret")
                .role("button")
                .label("Not a secret?")
                .focusable(true)
                .px(4.0)
                .radius(4.0)
                .child(text("Not a secret?").font(ty.meta_11).color(c.faint))
                .on_press_tx(not_a_secret),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::clip;
    use kudzu_core::secrets::SecretKind;

    #[test]
    fn dots_are_as_long_as_the_text_between_eight_and_eighteen() {
        assert_eq!(dots(3).chars().count(), 8);
        assert_eq!(dots(12).chars().count(), 12);
        assert_eq!(dots(500).chars().count(), 18);
    }

    #[test]
    fn hiding_a_clip_detection_missed_stores_an_override_and_undo_clears_it() {
        let e = clip("plain", 1, false);
        let c = change(&e, true);
        assert_eq!(
            (c.title, c.next, c.before),
            ("Hidden as a secret", Some(true), None)
        );
    }

    #[test]
    fn agreeing_with_detection_clears_the_override_instead_of_storing_a_redundant_one() {
        let mut e = clip("hunter2", 1, false);
        e.secret = Some(SecretKind::Credential);
        e.secret_override = Some(false);
        let c = change(&e, true);
        assert_eq!((c.next, c.before), (None, Some(false)));
        let c = change(&e, false);
        assert_eq!(c.next, Some(false));
        assert_eq!(c.title, "Marked as not a secret");
    }

    #[test]
    fn show_secrets_uncovers_and_the_label_gives_the_reason_but_never_the_content() {
        let mut e = clip("hunter2-very-secret", 1, false);
        e.secret = Some(SecretKind::Credential);
        assert!(is_hidden(&e, false));
        assert!(!is_hidden(&e, true));
        let label = cover_label(&e);
        assert!(label.starts_with("Hidden: ") && !label.contains("hunter2"));
    }
}
