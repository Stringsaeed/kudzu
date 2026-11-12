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
    let cloud = rig.kudzu.settings.watch().get().icloud_sync;
    rig.click_key("settings-icloud");
    assert_eq!(rig.kudzu.settings.watch().get().icloud_sync, !cloud);
    let old = rig.kudzu.settings.watch().get().launch_at_login;
    rig.rt.wheel(800.0, 400.0, -1000.0);
    rig.settle();
    rig.click_key("settings-login");
    assert_eq!(rig.kudzu.settings.watch().get().launch_at_login, !old);
    rig.click_key("settings-dock");
    assert!(!rig.kudzu.settings.watch().get().show_in_dock);
    rig.click_key("settings-theme-light");
    assert_eq!(rig.kudzu.settings.watch().get().theme, Theme::Light);
    assert_eq!(
        rig.rt.state.model.applied_appearance,
        Some(crate::theme::Appearance::Light)
    );
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

#[test]
fn apps_adds_paste_and_removes_and_sorts_names() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::Apps));
    rig.rt
        .update(|s, _| crate::main::settings::apps::choose(s, "com.example.typed".into()));
    rig.settle();
    assert_eq!(
        rig.kudzu.settings.watch().get().app_insertions["com.example.typed"],
        kudzu_core::library::model::InsertionMethod::Paste
    );
    let installed = rig.kudzu.apps.installed().get();
    let mut names: Vec<_> = rig
        .kudzu
        .settings
        .watch()
        .get()
        .app_insertions
        .keys()
        .map(|id| {
            (
                installed
                    .iter()
                    .find(|a| a.bundle_id == *id)
                    .map(|a| a.name.to_lowercase())
                    .unwrap_or_else(|| id.to_lowercase()),
                id.clone(),
            )
        })
        .collect();
    names.sort();
    for pair in names.windows(2) {
        let first = rig
            .rect(&format!("settings-app-remove-{}", pair[0].1))
            .unwrap();
        let second = rig
            .rect(&format!("settings-app-remove-{}", pair[1].1))
            .unwrap();
        assert!(first[1] < second[1]);
    }
    rig.click_key("settings-app-remove-com.example.typed");
    assert!(!rig
        .kudzu
        .settings
        .watch()
        .get()
        .app_insertions
        .contains_key("com.example.typed"));
}

#[test]
fn settings_regions_fit_both_themes_and_minimum_viewport() {
    for theme in [Theme::Dark, Theme::Light] {
        for size in [(1180.0, 760.0), (900.0, 560.0)] {
            for pane in [
                SettingsPane::General,
                SettingsPane::Expansion,
                SettingsPane::Apps,
            ] {
                let mut rig = Rig::themed(true, theme);
                rig.rt.viewport = size;
                let mut prefs: kudzu_app::settings::Settings =
                    serde_json::from_value(rig.fixtures.sample.raw["settings"].clone()).unwrap();
                prefs.theme = theme;
                rig.kudzu.settings.update(|p| *p = prefs).unwrap();
                rig.open_route(Route::Settings(pane));
                let expected = [
                    ("settings-nav", [224.0, 50.0, 192.0, size.1 - 58.0]),
                    ("settings-heading", [416.0, 50.0, size.0 - 424.0, 48.0]),
                ];
                for (key, rect) in expected {
                    let got = rig.rect(key).unwrap();
                    for i in 0..4 {
                        assert!(
                            (got[i] - rect[i]).abs() <= 2.0,
                            "{pane:?} {key} {got:?} != {rect:?}"
                        );
                    }
                }
                let body = rig.rect("settings-body").unwrap();
                assert!(body[2] <= 680.0);
                if size.0 == 1180.0 {
                    let expected: &[(&str, [f32; 4])] = match pane {
                        SettingsPane::General => &[
                            ("settings-group-Permissions", [486.0, 130.0, 616.0, 180.0]),
                            ("settings-group-Quick search", [486.0, 342.0, 616.0, 90.0]),
                        ],
                        SettingsPane::Expansion => &[
                            ("settings-group-Expanding", [486.0, 130.0, 616.0, 228.0]),
                            ("settings-group-Good to know", [486.0, 390.0, 616.0, 165.5]),
                        ],
                        SettingsPane::Apps => &[
                            (
                                "settings-group-Never expand in",
                                [486.0, 130.0, 616.0, 96.0],
                            ),
                            (
                                "settings-group-How to insert, per app",
                                [486.0, 258.0, 616.0, 194.0],
                            ),
                        ],
                        _ => unreachable!(),
                    };
                    for (key, want) in expected {
                        let got = rig.rect(key).unwrap();
                        for i in 0..4 {
                            assert!((got[i] - want[i]).abs() <= 2.0, "{key} {got:?} != {want:?}");
                        }
                    }
                }
                assert!(body[0] >= 416.0);
                assert!(body[0] + body[2] <= size.0 - 8.0 + 2.0);
            }
        }
    }
}

#[test]
fn shortcut_capture_expires_and_login_reaction_completes() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::General));
    rig.rt
        .update(|s, _| crate::main::settings::general::set_shortcut(s, "Control+Alt+K"));
    rig.settle();
    assert!(rig.rt.state.settings.captured_until.is_some());
    rig.fixtures.clock.advance(Duration::from_secs(1));
    rig.settle();
    assert!(rig.rt.state.settings.captured_until.is_none());
    rig.click_key("settings-login");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !*rig.fixtures.login_item.enabled.lock().unwrap() && std::time::Instant::now() < deadline
    {
        std::thread::yield_now();
    }
    assert!(*rig.fixtures.login_item.enabled.lock().unwrap());
    assert_eq!(
        rig.rt.state.model.applied_appearance,
        Some(crate::theme::Appearance::Dark)
    );
}

#[test]
fn shortcut_taken_rolls_back_registration_and_shows_exact_error() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::General));
    let old = rig.kudzu.settings.watch().get().palette_hotkey.clone();
    rig.fake_hotkeys
        .rejected
        .lock()
        .unwrap()
        .insert("Control+Alt+K".into());
    rig.rt
        .update(|s, _| crate::main::settings::general::set_shortcut(s, "Control+Alt+K"));
    rig.settle();
    assert_eq!(rig.kudzu.settings.watch().get().palette_hotkey, old);
    assert!(
        rig.rt
            .elements()
            .iter()
            .any(|e| e.text.as_deref()
                == Some("Couldn't use that shortcut — another app may own it."))
    );
}

#[test]
fn prefix_and_recorder_ignore_ime_composition() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::Expansion));
    rig.click_key("settings-prefix");
    let before = rig
        .kudzu
        .settings
        .watch()
        .get()
        .default_trigger_prefix
        .clone();
    rig.rt.ime(rune::Ime::Preedit("あ".into(), None));
    rig.settle();
    assert!(rig.rt.state.settings.is_composing());
    rig.rt.update(|s, _| {
        s.settings.recording = true;
        assert!(!crate::main::settings::general::record_key(s, &cmd('k')));
    });
    rig.press(KeyEvent {
        key: Key::Down,
        mods: Mods::default(),
    });
    assert_eq!(
        rig.rt.state.frame.view,
        crate::main::MainView::Settings(SettingsPane::Expansion)
    );
    assert_eq!(
        rig.kudzu.settings.watch().get().default_trigger_prefix,
        before
    );
}

#[test]
fn general_permission_copy_covers_missing_ready_and_blocked_states() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::General));
    for (granted, can_expand, summary, title, subtitle, button) in [
        (
            false,
            false,
            "blocked",
            "Enable typing in other apps",
            "A guided setup, with every step beside you.",
            "Set up permissions",
        ),
        (
            true,
            true,
            "ready",
            "Permissions are set",
            "Kudzu is ready to expand your snippets.",
            "Review permissions",
        ),
        (
            true,
            false,
            "Secure Input is on",
            "Permissions are set",
            "Secure Input is on",
            "Review permissions",
        ),
    ] {
        let mut status = (*rig.kudzu.expansion.status().get()).clone();
        status.permissions.accessibility = granted;
        status.permissions.input_monitoring = granted;
        status.can_post_events = granted;
        status.can_expand = can_expand;
        status.summary = summary.into();
        rig.kudzu.expansion.status_changed(status);
        rig.settle();
        for expected in [title, subtitle, button] {
            assert!(
                rig.rt
                    .elements()
                    .iter()
                    .any(|e| e.text.as_deref() == Some(expected)),
                "{expected}"
            );
        }
    }
}

#[test]
fn settings_reactions_reach_engine_and_login_errors_reach_visible_copy() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::Expansion));
    rig.click_key("settings-insertion-paste");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while rig
        .fixtures
        .engine
        .state
        .lock()
        .unwrap()
        .library
        .config
        .default_insertion
        != kudzu_core::library::model::InsertionMethod::Paste
        && std::time::Instant::now() < deadline
    {
        std::thread::yield_now();
    }
    assert_eq!(
        rig.fixtures
            .engine
            .state
            .lock()
            .unwrap()
            .library
            .config
            .default_insertion,
        kudzu_core::library::model::InsertionMethod::Paste
    );
    *rig.fixtures.login_item.failure.lock().unwrap() =
        Some(kudzu_app::error::PortError::Unavailable);
    rig.open_route(Route::Settings(SettingsPane::General));
    rig.rt.wheel(800.0, 400.0, -1000.0);
    rig.settle();
    rig.click_key("settings-login");
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while rig.kudzu.login_notice.get().is_none() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        rig.kudzu.login_notice.get().as_deref(),
        Some("login item unavailable")
    );
    rig.settle();
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some("login item unavailable")));
}

#[test]
fn prefix_field_commits_and_bounds_typed_input() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::Expansion));
    rig.click_key("settings-prefix");
    rig.press(KeyEvent {
        key: Key::Text("abcdef".into()),
        mods: Mods::default(),
    });
    assert!(
        rig.kudzu
            .settings
            .watch()
            .get()
            .default_trigger_prefix
            .encode_utf16()
            .count()
            <= 4
    );
    assert_eq!(
        rig.kudzu.settings.watch().get().default_trigger_prefix,
        rig.rt.state.settings.prefix.text
    );
    rig.press(KeyEvent {
        key: Key::Backspace,
        mods: Mods::default(),
    });
    assert_eq!(
        rig.kudzu
            .settings
            .watch()
            .get()
            .default_trigger_prefix
            .encode_utf16()
            .count(),
        3
    );
}

#[test]
fn dock_reaction_sends_the_committed_setting_to_its_output_port() {
    let mut rig = Rig::new(true);
    let output = Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = output.clone();
    rig.rt.update(move |s, _| {
        crate::main::settings::general::toggle_dock(s, |visible| {
            sink.lock().unwrap().push(visible)
        });
        crate::main::settings::general::toggle_dock(s, |visible| {
            sink.lock().unwrap().push(visible)
        });
    });
    assert_eq!(*output.lock().unwrap(), [false, true]);
    assert!(rig.kudzu.settings.watch().get().show_in_dock);
}

#[test]
fn never_expand_picker_callback_flips_the_latest_settings() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::Apps));
    rig.rt.update(|s, _| {
        s.settings.picker_deny = true;
        crate::main::settings::apps::choose(s, "com.example.excluded".into());
    });
    assert!(rig
        .kudzu
        .settings
        .watch()
        .get()
        .denied_bundle_ids
        .contains(&"com.example.excluded".into()));
    rig.rt
        .update(|s, _| crate::main::settings::apps::choose(s, "com.example.excluded".into()));
    assert!(!rig
        .kudzu
        .settings
        .watch()
        .get()
        .denied_bundle_ids
        .contains(&"com.example.excluded".into()));
}

#[test]
fn recorder_shows_its_prompt_and_escape_preserves_the_shortcut() {
    let mut rig = Rig::new(true);
    rig.open_route(Route::Settings(SettingsPane::General));
    let before = rig.kudzu.settings.watch().get().palette_hotkey.clone();
    rig.click_key("settings-shortcut");
    assert!(rig.rt.state.settings.recording);
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some("Press shortcut…")));
    rig.press(esc());
    assert!(!rig.rt.state.settings.recording);
    assert_eq!(rig.kudzu.settings.watch().get().palette_hotkey, before);
}
