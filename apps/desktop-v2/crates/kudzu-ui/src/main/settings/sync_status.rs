//! iCloud status slot owned by S63. This module keeps its work independent of General.
use crate::{shell::MainState, theme::Theme};
use rune::{col, El};
pub fn view(_s: &MainState, _t: &Theme) -> El {
    col().key("settings-sync-status").w_full()
}
