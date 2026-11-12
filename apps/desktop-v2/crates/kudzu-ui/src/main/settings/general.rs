use crate::{shell::MainState, theme::Theme};
use rune::{col, El};
pub fn view(_s: &MainState, _t: &Theme) -> El {
    col()
}
pub fn record_key(_s: &mut MainState, _e: &rune::KeyEvent) -> bool {
    false
}
