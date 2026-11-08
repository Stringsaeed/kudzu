//! `components/app/clipboard/secret.tsx`: secrets in clipboard history (passwords, keys,
//! tokens) are hidden until the pointer rests on them, unless Settings ▸ Clipboard ▸ Show
//! secrets is on. Detection runs in Rust when an item is copied; the user can correct it per item.
//!
//! CSS hides with `group-hover:hidden` / `invisible`; here both looks are drawn and crossfade by
//! their hover group's state (`group_opacity`), so hovering never re-runs the view.

use kudzu_core::client::clipboard::{is_secret, secret_reason};
use kudzu_core::clipboard::ClipEntry;
use rune::{col, el, icon, row, text, El};
use uuid::Uuid;

use super::ClipHost;
use crate::ui::icons;
use crate::ui::kit::toast::Toast;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, WeightOf};

/// `useConcealed`: it's a secret and "Show secrets" is off.
pub fn concealed<S: ClipHost>(s: &S, e: &ClipEntry) -> bool {
    is_secret(e) && !s.show_secrets()
}

/// Dots standing in for hidden text, about as long as the text (capped).
pub fn dots(length: usize) -> String {
    "•".repeat(length.clamp(8, 18))
}

/// `RowSecret`: `stand` until the pointer is over the enclosing row (a hover group), then the
/// real text. `real` keeps the layout; `stand` sits over it.
pub fn row_secret(concealed: bool, stand: El, real: El) -> El {
    if !concealed {
        return real;
    }
    el().w_full()
        .min_w(0.0)
        .child(real.group_opacity(0.0, 1.0))
        .child(el().absolute(0.0, 0.0).w_full().child(stand.group_opacity(1.0, 0.0)).a11y_hidden())
}

/// `Concealed`: covers a preview until it's hovered or focused. The content keeps its layout (so
/// nothing jumps on reveal) but isn't seen while covered.
pub fn concealed_box<S: ClipHost>(s: &S, th: &Theme, e: &ClipEntry, content: El) -> El {
    if !concealed(s, e) {
        return content;
    }
    let t = &th.t;
    let reason = secret_reason(e);
    let cover = col()
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .center()
        .gap(6.0)
        .radius(radius::XL)
        .fill(Theme::tint(t.foreground, 0.025))
        .elevate_well(th)
        .child(el().size(36.0, 36.0).center().radius(radius::XL).fill(Theme::tint(t.foreground, 0.06)).child(icon(&icons::EYE_OFF, 16.0).color(t.muted_foreground)))
        .child(text(reason.clone()).font(ty::BASE.weight_of(weight::MEDIUM)).color(t.foreground))
        .child(text("Hover to show").font(ty::XS).color(t.faint))
        .group_opacity(1.0, 0.0)
        .a11y_hidden()
        .inert();
    el().key("clip-secret")
        .w_full()
        .min_h(112.0)
        .radius(radius::XL)
        .hover_group_focus()
        .focusable(true)
        .focus_ring(2.0, 0.0, t.ring)
        .label(format!("Hidden: {}. Hover or focus to show.", js_lower(&reason)))
        .child(el().w_full().child(content).group_opacity(0.0, 1.0))
        .child(cover)
}

fn js_lower(s: &str) -> String {
    kudzu_core::client::js::lower(s)
}

trait Well {
    fn elevate_well(self, th: &Theme) -> Self;
}

impl Well for El {
    fn elevate_well(self, th: &Theme) -> El {
        use crate::ui::theme::Utilities;
        self.elevate(&th.t.elev_well)
    }
}

/// `setSecretWithFeedback`: marks an entry as a secret or not, with an Undo toast that says what
/// changed.
pub fn set_with_feedback(s: &mut MainUi, id: Uuid, secret: bool) {
    let Some(e) = s.clip.get(id) else { return };
    let before = e.secret_override;
    let next = super::secret_correction(e, secret);
    s.clip.set_secret(id, next);
    let toast = Toast::success(if secret { "Hidden as a secret" } else { "Marked as not a secret" })
        .description(if secret {
            "It stays hidden until you hover over it, and AI agents can't see it."
        } else {
            "It's shown normally, and AI agents can read it if you share clipboard history with them."
        })
        .action("Undo", move |s: &mut MainUi, _| s.clip.set_secret(id, before))
        .duration_ms(6000);
    s.toasts.push(toast);
}

/// `SecretBadge`: the heading's "Secret" chip with a one-click "Not a secret?" correction.
pub fn badge<S: ClipHost>(th: &Theme, e: &ClipEntry) -> Option<El> {
    if !is_secret(e) {
        return None;
    }
    let t = &th.t;
    let chip = row()
        .gap(4.0)
        .center_y()
        .radius(radius::MD)
        .px(6.0)
        .py(2.0)
        .fill(Theme::tint(t.warning, 0.12))
        .child(icon(&icons::KEY_ROUND, 12.0).color(t.warning))
        .child(text("Secret").font(super::preview::chip_font(weight::MEDIUM)).color(t.warning));
    let id = e.id;
    let fix = el()
        .key("not-a-secret")
        .role("button")
        .label("Not a secret?")
        .radius(4.0)
        .px(4.0)
        .min_w(0.0)
        .shrink()
        .hover_group()
        .focusable(true)
        .focus_ring(2.0, 0.0, t.ring)
        .on_press(move |s: &mut S| s.set_secret_with_feedback(id, false))
        .child(underlined(text("Not a secret?").font(super::preview::chip_font(weight::NORMAL)).truncate(), t.faint, t.muted_foreground));
    Some(row().gap(6.0).center_y().min_w(0.0).child(chip).child(fix))
}

/// `underline-offset-2 hover:underline` with a hover colour, inside a hover group.
pub fn underlined(label: El, rest: rune::Color, hover: rune::Color) -> El {
    el().child(label.color(rest).group_color(hover)).child(
        el().pin(Some(0.0), None, Some(0.0), Some(1.0)).h(1.0).fill(hover).group_opacity(0.0, 1.0).a11y_hidden(),
    )
}

