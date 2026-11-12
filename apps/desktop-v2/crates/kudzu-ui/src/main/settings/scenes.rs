//! Whole-window parity is deferred until S42 lands; region tests cover these panes.
use crate::shell::{MainState, Route, SettingsPane};
pub fn general(s: &mut MainState) {
    s.open_route(Route::Settings(SettingsPane::General));
}
pub fn expansion(s: &mut MainState) {
    s.open_route(Route::Settings(SettingsPane::Expansion));
}
pub fn apps(s: &mut MainState) {
    s.open_route(Route::Settings(SettingsPane::Apps));
}
