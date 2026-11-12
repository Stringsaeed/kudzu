//! The large preview (Space): the same heading and body in a dialog of
//! `min(920, 90% of the window)` by 86% of its height. Space and Escape close it and the
//! arrows move to the neighbouring clip (the screen's key handler does that).

use rune::{col, row, El};

use super::preview::{self, Hooks, Preview};
use crate::theme::Theme;
use crate::widgets::kit_ext::dialog::Dialog;

pub const MAX_W: f32 = 920.0;
pub const WIDTH_FRACTION: f32 = 0.9;
pub const HEIGHT_FRACTION: f32 = 0.86;

/// The dialog's width in a window `window_w` wide.
pub fn width(window_w: f32) -> f32 {
    MAX_W.min(window_w * WIDTH_FRACTION)
}

/// The dialog's height in a window `window_h` high.
pub fn height(window_h: f32) -> f32 {
    window_h * HEIGHT_FRACTION
}

/// Pushes the dialog onto `layer`. `title` is the clip's title, or "Secret"; it and the
/// description are spoken, not drawn (`sr-only`, as Tauri's are). `window` is the window's
/// size in points: the panel is at most `min(920, 90%)` wide and 86% high.
pub fn build<S: 'static>(
    theme: &Theme,
    p: &Preview,
    hooks: Hooks<S>,
    title: &str,
    window: (f32, f32),
    on_close: impl Fn(&mut S) + Clone + 'static,
    layer: &mut Vec<El>,
) {
    let head = row()
        .key("clip-large-heading")
        .w_full()
        .pl(24.0)
        .pr(48.0)
        .py(14.0)
        .child(preview::heading(theme, p, hooks));
    let body = col()
        .key("clip-large-body")
        .w_full()
        .shrink()
        .min_h(0.0)
        .scroll(rune::ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .w_full()
                .padding(24.0)
                .child(preview::content(theme, p, hooks)),
        );
    Dialog::new("clip-large", title)
        .description("Large preview. Press Space or Escape to close.")
        .open(true)
        .kudzu()
        .bare()
        .width(width(window.0))
        .max_height(height(window.1))
        .child(head)
        .child(row().w_full().h(1.0).fill(theme.c.hairline))
        .child(body)
        .on_close(on_close)
        .build(theme, layer);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_is_at_most_920_wide_and_ninety_percent_of_a_narrow_window() {
        assert_eq!(width(1180.0), 920.0);
        assert_eq!(width(900.0), 810.0);
        assert!((height(760.0) - 653.6).abs() < 0.01);
    }
}
