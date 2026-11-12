//! Activity pane. Owned by S63; replace this placeholder in that step.
use crate::{shell::MainState, theme::Theme};
use rune::{text, El};
pub fn view(_s: &MainState, t: &Theme) -> El {
    text("Activity").font(t.ty.body).color(t.c.foreground)
}
