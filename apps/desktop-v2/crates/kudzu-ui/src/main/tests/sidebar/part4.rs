use super::Rig;

#[test]
fn rename_failure_shows_the_domain_error_and_retains_the_draft() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Original".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", group.id));
    rig.press(rune::KeyEvent {
        key: rune::Key::Function(2),
        mods: rune::Mods::default(),
    });
    rig.rt
        .update(|st, _| st.sidebar.edit.replace_text("Changed"));
    rig.kudzu.shutdown();
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
    assert!(rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("The library could not be saved or loaded."));
}

#[test]
fn rejected_group_move_shows_the_domain_error() {
    let mut rig = Rig::new(true);
    let (parent, _) = rig
        .kudzu
        .library
        .create_group("Parent".into(), None)
        .unwrap();
    let (child, _) = rig
        .kudzu
        .library
        .create_group("Child".into(), Some(parent.id))
        .unwrap();
    rig.rt.update(move |st, _| {
        crate::dnd::drop_payload(
            st,
            crate::dnd::DragPayload::Group(parent.id),
            Some(child.id),
        );
    });
    rig.settle();
    assert!(rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("A group can't be moved inside itself."));
}

#[test]
fn failed_group_deletion_shows_the_domain_error() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.kudzu.shutdown();
    rig.rt
        .update(move |st, _| crate::main::sidebar::actions::delete(st, group.id, false));
    rig.settle();
    assert!(rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("The library could not be saved or loaded."));
}

#[test]
fn group_keys_ignore_ime_composition() {
    let mut rig = Rig::new(true);
    let (parent, _) = rig
        .kudzu
        .library
        .create_group("Parent".into(), None)
        .unwrap();
    rig.kudzu
        .library
        .create_group("Child".into(), Some(parent.id))
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", parent.id));
    rig.rt.update(|st, _| {
        st.sidebar
            .edit
            .ime(&rune::Ime::Preedit("x".into(), Some((0, 1))));
    });
    assert!(rig.rt.state.is_composing());
    rig.press(rune::KeyEvent {
        key: rune::Key::Function(2),
        mods: rune::Mods::default(),
    });
    assert!(rig.rt.state.sidebar.rename.is_none());
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods {
            alt: true,
            ..Default::default()
        },
    });
    assert!(rig.rt.state.sidebar.rename.is_none());
    rig.press(rune::KeyEvent {
        key: rune::Key::Left,
        mods: rune::Mods::default(),
    });
    assert!(!rig.rt.state.sidebar.collapsed.contains(&parent.id));
}

#[test]
fn double_click_does_not_start_rename_during_ime() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.rt.update(|st, _| {
        st.sidebar
            .edit
            .ime(&rune::Ime::Preedit("x".into(), Some((0, 1))));
    });
    assert!(rig.rt.state.is_composing());
    rig.click_key(&format!("sidebar-group-{}", group.id));
    rig.click_key(&format!("sidebar-group-{}", group.id));
    assert!(rig.rt.state.sidebar.rename.is_none());
}

#[test]
fn sidebar_navigation_keys_do_not_move_focus_during_ime() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-item-favorites");
    rig.rt.update(|st, _| {
        st.sidebar
            .edit
            .ime(&rune::Ime::Preedit("x".into(), Some((0, 1))));
    });
    assert!(rig.rt.state.is_composing());
    for key in [
        rune::Key::Up,
        rune::Key::Down,
        rune::Key::Home,
        rune::Key::End,
    ] {
        rig.press(rune::KeyEvent {
            key,
            mods: rune::Mods::default(),
        });
        assert_eq!(rig.focused_key().as_deref(), Some("sidebar-item-favorites"));
    }
}

#[test]
fn sidebar_landmarks_match_reference_in_both_themes() {
    for theme in [
        kudzu_app::settings::Theme::Light,
        kudzu_app::settings::Theme::Dark,
    ] {
        let mut rig = Rig::themed(true, theme);
        let history = rig.fixtures.sample.clipboard.clone();
        rig.rt.update(move |st, _| {
            crate::main::library::scenes::seed_sample(st);
            st.svc.clipboard.replace_history(history).unwrap();
            st.navigate_to(crate::main::MainView::Insights);
        });
        rig.settle();
        for (key, expected) in [
            ("main-sidebar", [0.0, 42.0, 224.0, 718.0]),
            ("sidebar-wordmark", [16.0, 48.844, 192.0, 58.313]),
            ("sidebar-nav", [0.0, 114.0, 224.0, 527.0]),
            ("sidebar-region-snippets", [10.0, 114.0, 204.0, 295.0]),
            ("sidebar-section-snippets", [14.0, 116.0, 196.0, 24.0]),
            ("sidebar-item-all-snippets", [10.0, 143.0, 204.0, 28.0]),
            ("sidebar-item-favorites", [10.0, 172.0, 204.0, 28.0]),
            ("sidebar-item-recently-used", [10.0, 201.0, 204.0, 28.0]),
            ("sidebar-item-turned-off", [10.0, 230.0, 204.0, 28.0]),
            ("sidebar-new-group", [190.0, 272.0, 20.0, 20.0]),
            ("sidebar-region-clipboard", [10.0, 421.0, 204.0, 260.0]),
            ("sidebar-section-clipboard", [14.0, 423.0, 196.0, 24.0]),
            ("sidebar-item-history", [10.0, 450.0, 204.0, 28.0]),
            ("sidebar-item-pinned", [10.0, 479.0, 204.0, 28.0]),
            ("sidebar-item-text", [10.0, 508.0, 204.0, 28.0]),
            ("sidebar-item-images", [10.0, 537.0, 204.0, 28.0]),
            ("sidebar-item-links", [10.0, 566.0, 204.0, 28.0]),
            ("sidebar-item-files", [10.0, 595.0, 204.0, 28.0]),
            ("sidebar-item-code", [10.0, 624.0, 204.0, 28.0]),
            ("sidebar-item-colors", [10.0, 653.0, 204.0, 28.0]),
            ("sidebar-item-insights", [10.0, 650.0, 204.0, 28.0]),
            ("sidebar-item-settings", [10.0, 679.0, 204.0, 28.0]),
            ("sidebar-status", [10.0, 726.0, 172.0, 24.0]),
            ("sidebar-pause", [186.0, 724.0, 28.0, 28.0]),
        ] {
            let got = rig.rect(key).unwrap_or_else(|| panic!("missing {key}"));
            for axis in 0..4 {
                assert!(
                    (got[axis] - expected[axis]).abs() <= 2.0,
                    "{theme:?} {key} expected {expected:?}, got {got:?}"
                );
            }
        }
        for (name, expected) in [
            ("Work", [10.0, 295.0, 204.0, 28.0]),
            ("Email", [10.0, 323.0, 204.0, 28.0]),
            ("Developer", [10.0, 352.0, 204.0, 28.0]),
            ("Personal", [10.0, 381.0, 204.0, 28.0]),
        ] {
            let id = rig
                .library()
                .groups
                .iter()
                .find(|g| g.name == name)
                .unwrap()
                .id;
            let key = format!("sidebar-group-{id}");
            let got = rig.rect(&key).unwrap();
            for axis in 0..4 {
                assert!(
                    (got[axis] - expected[axis]).abs() <= 2.0,
                    "{theme:?} {key} expected {expected:?}, got {got:?}"
                );
            }
        }
    }
}

#[test]
fn paused_footer_shows_reference_copy() {
    let mut rig = Rig::new(true);
    let mut status = (*rig.kudzu.expansion.status().get()).clone();
    status.paused = true;
    rig.kudzu.expansion.status_changed(status);
    rig.settle();
    let elements = rig.rt.elements();
    assert!(elements
        .iter()
        .any(|e| e.label.as_deref() == Some("Status: Paused. Nothing expands until you resume.")));
    assert!(elements
        .iter()
        .any(|e| e.label.as_deref() == Some("Resume expansion")));
}

#[test]
fn rename_of_deleted_group_reports_missing_and_keeps_draft() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Original".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", group.id));
    rig.press(rune::KeyEvent {
        key: rune::Key::Function(2),
        mods: rune::Mods::default(),
    });
    rig.rt
        .update(|st, _| st.sidebar.edit.replace_text("Changed"));
    rig.kudzu
        .library
        .delete_group(group.id, kudzu_app::library::GroupDeletion::KeepSnippets)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.rt
        .update(|st, _| crate::main::sidebar::actions::commit_rename(st));
    rig.settle();
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
    assert!(rig.rect("sidebar-group-rename").is_some());
    assert!(rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Group not found"));
}

#[test]
fn empty_sidebar_copy_and_footer_fit_the_minimum_window_in_both_themes() {
    for theme in [
        kudzu_app::settings::Theme::Light,
        kudzu_app::settings::Theme::Dark,
    ] {
        let mut rig = Rig::themed(true, theme);
        rig.rt
            .update(|st, _| crate::main::library::scenes::library_empty(st));
        rig.rt.set_viewport((900.0, 560.0), 1.0);
        rig.settle();
        let sidebar = rig.rect("main-sidebar").unwrap();
        for (got, expected) in sidebar.into_iter().zip([0.0, 42.0, 224.0, 518.0]) {
            assert!((got - expected).abs() <= 2.0);
        }
        let footer = rig.rect("sidebar-pause").unwrap();
        assert!(footer[1] + footer[3] <= 560.0);
        assert!(rig.rt.elements().iter().any(|e| e.text.as_deref()
            == Some("Organize snippets into groups — like Work, Email or Code.")));
    }
}
