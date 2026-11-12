use super::*;
use crate::shell::{Route, SettingsPane};

#[test]
fn settings_frame_has_eight_panes_and_cycles() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::General));
    assert_eq!(rig.rect("settings-nav").unwrap()[2], 192.0);
    assert_eq!(rig.rect("settings-heading").unwrap()[3], 48.0);
    for pane in crate::main::settings::nav::PANES {
        assert!(rig.rect(&format!("settings-nav-{pane:?}")).is_some());
    }
    rig.press(KeyEvent {
        key: Key::Up,
        mods: Mods::default(),
    });
    assert_eq!(
        rig.rt.state.frame.view,
        crate::main::MainView::Settings(SettingsPane::About)
    );
}

#[test]
fn general_controls_commit_settings_and_hotkey_collision_rolls_back() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::General));
    let old = rig.kudzu.settings.watch().get().launch_at_login;
    rig.click_key("settings-login");
    assert_eq!(rig.kudzu.settings.watch().get().launch_at_login, !old);
    rig.click_key("settings-dock");
    assert!(!rig.kudzu.settings.watch().get().show_in_dock);
    rig.click_key("settings-theme-light");
    assert_eq!(rig.kudzu.settings.watch().get().theme, Theme::Light);
    let before = rig.kudzu.settings.watch().get().palette_hotkey.clone();
    let collision = rig.kudzu.settings.watch().get().clipboard_hotkey.clone();
    rig.rt
        .update(move |s, _| crate::main::settings::general::set_shortcut(s, &collision));
    rig.settle();
    assert_eq!(rig.kudzu.settings.watch().get().palette_hotkey, before);
    assert!(
        rig.rt
            .elements()
            .iter()
            .any(|e| e.text.as_deref()
                == Some("Kudzu already uses that shortcut for something else."))
    );
}

#[test]
fn expansion_pause_reads_settings_and_prefix_limit_is_utf16() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::Expansion));
    let paused = rig.kudzu.settings.watch().get().paused;
    rig.click_key("settings-pause");
    rig.click_key("settings-pause");
    assert_eq!(rig.kudzu.settings.watch().get().paused, paused);
    assert_eq!(
        crate::main::settings::expansion::limited_prefix("😀😀a"),
        "😀😀"
    );
    rig.click_key("settings-insertion-paste");
    assert_eq!(
        rig.kudzu.settings.watch().get().default_insertion,
        kudzu_core::library::model::InsertionMethod::Paste
    );
    for copy in ["Undo an expansion", "Clipboard is safe", "Secure Input"] {
        assert!(rig
            .rt
            .elements()
            .iter()
            .any(|e| e.text.as_deref().is_some_and(|t| t.starts_with(copy))));
    }
}
