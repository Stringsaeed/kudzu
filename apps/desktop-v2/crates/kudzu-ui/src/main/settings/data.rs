//! Data pane. Owned by S62; replace this placeholder in that step.
use crate::{shell::MainState, theme::Theme};
use rune::{text, El};
pub fn view(_s: &MainState, t: &Theme) -> El {
    text("Data").font(t.ty.body).color(t.c.foreground)
}
