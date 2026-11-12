//! Whole-window parity is deferred until S42 lands; region tests cover these panes.
use crate::shell::{MainState, Route, SettingsPane};
pub fn general(s: &mut MainState) {
    seed(s, SettingsPane::General);
}
pub fn expansion(s: &mut MainState) {
    seed(s, SettingsPane::Expansion);
}
pub fn apps(s: &mut MainState) {
    seed(s, SettingsPane::Apps);
}

fn seed(s: &mut MainState, pane: SettingsPane) {
    let sample = kudzu_app::fixtures::SampleState::load().expect("settings fixture");
    let mut prefs: kudzu_app::settings::Settings =
        serde_json::from_value(sample.raw["settings"].clone()).expect("settings fixture schema");
    prefs.theme = s.svc.settings.watch().get().theme;
    s.svc
        .settings
        .update(|p| *p = prefs)
        .expect("settings fixture accepted");
    s.svc.expansion.status_changed(sample.status);
    s.open_route(Route::Settings(pane));
}
