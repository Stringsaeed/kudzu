use super::{LibraryFilter, MainView, Rig};

#[test]
fn footer_reports_status_and_can_pause() {
    let rig = Rig::new(true);
    assert!(rig.rt.elements().iter().any(|e| e
        .label
        .as_deref()
        .is_some_and(|label| label.starts_with("Status: "))));
}

#[test]
fn arrows_and_home_end_move_focus_across_sidebar_rows() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-item-favorites");
    rig.press(rune::KeyEvent {
        key: rune::Key::Down,
        mods: rune::Mods::default(),
    });
    assert_eq!(
        rig.focused_key().as_deref(),
        Some("sidebar-item-recently-used")
    );
    rig.press(rune::KeyEvent {
        key: rune::Key::Home,
        mods: rune::Mods::default(),
    });
    assert_eq!(
        rig.focused_key().as_deref(),
        Some("sidebar-section-snippets")
    );
    rig.press(rune::KeyEvent {
        key: rune::Key::End,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.focused_key().as_deref(), Some("sidebar-pause"));
}

#[test]
fn left_and_right_fold_and_unfold_sections() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-section-snippets");
    rig.press(rune::KeyEvent {
        key: rune::Key::Right,
        mods: rune::Mods::default(),
    });
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.label.as_deref() == Some("Favorites")));
    rig.press(rune::KeyEvent {
        key: rune::Key::Left,
        mods: rune::Mods::default(),
    });
    assert!(!rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Favorites"));
}

#[test]
fn new_subgroup_expands_parent_and_starts_rename() {
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
    rig.click_key(&format!("sidebar-collapse-{}", parent.id));
    let a = rig.rect(&format!("sidebar-group-{}", parent.id)).unwrap();
    let (x, y) = (a[0] + 20.0, a[1] + a[3] / 2.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    rig.click_text("New subgroup");
    assert!(!rig.rt.state.sidebar.collapsed.contains(&parent.id));
    let id = rig.rt.state.sidebar.rename.unwrap();
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Group(id))
    );
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == id)
            .unwrap()
            .parent_id,
        Some(parent.id)
    );
}

#[test]
fn clipboard_folders_refresh_after_a_new_copy() {
    let mut rig = Rig::new(true);
    assert!(!rig
        .rt
        .elements()
        .iter()
        .any(|e| e.role == "button" && e.label.as_deref() == Some("Text")));
    rig.fixtures.pasteboard.external_copy(vec![vec![(
        "public.utf8-plain-text".into(),
        b"fixture".to_vec(),
    )]]);
    rig.kudzu.clipboard.poll().unwrap();
    rig.settle();
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.role == "button" && e.label.as_deref() == Some("Text")));
}

#[test]
fn group_menu_rect_and_sidebar_landmarks_in_both_themes() {
    for theme in [
        kudzu_app::settings::Theme::Light,
        kudzu_app::settings::Theme::Dark,
    ] {
        let mut rig = Rig::themed(true, theme);
        let (group, _) = rig
            .kudzu
            .library
            .create_group("Example".into(), None)
            .unwrap();
        rig.rt.update(|st, _| st.refresh_library_snapshot());
        rig.settle();
        let sidebar = rig.rect("main-sidebar").unwrap();
        for (got, expected) in sidebar.into_iter().zip([0.0, 42.0, 224.0, 718.0]) {
            assert!((got - expected).abs() <= 2.0);
        }
        let a = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
        let (x, y) = (a[0] + 20.0, a[1] + a[3] / 2.0);
        rig.rt.secondary_down(x, y);
        rig.rt.secondary_up(x, y);
        rig.settle();
        let menu = rig.rect("sidebar-group-menu").unwrap();
        for (got, expected) in menu.into_iter().zip([x, y, 208.0, 168.0]) {
            assert!((got - expected).abs() <= 2.0);
        }
    }
}

#[test]
fn footer_stays_at_bottom_when_groups_are_many() {
    let mut rig = Rig::new(true);
    for index in 0..30 {
        rig.kudzu
            .library
            .create_group(format!("Group {index}"), None)
            .unwrap();
    }
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let footer = rig.rect("sidebar-pause").unwrap();
    assert!(footer[1] >= 710.0 && footer[1] + footer[3] <= 760.0);
}

#[test]
fn group_menu_delete_with_snippets_removes_descendants() {
    use kudzu_app::library::SnippetDraft;
    for theme in [
        kudzu_app::settings::Theme::Light,
        kudzu_app::settings::Theme::Dark,
    ] {
        let mut rig = Rig::themed(true, theme);
        let (parent, _) = rig
            .kudzu
            .library
            .create_group("Delete tree".into(), None)
            .unwrap();
        let (child, _) = rig
            .kudzu
            .library
            .create_group("Child".into(), Some(parent.id))
            .unwrap();
        let (snippet, _) = rig
            .kudzu
            .library
            .create_snippet(SnippetDraft {
                group_id: Some(child.id),
                ..Default::default()
            })
            .unwrap();
        rig.rt.update(|st, _| st.refresh_library_snapshot());
        rig.settle();
        let a = rig.rect(&format!("sidebar-group-{}", parent.id)).unwrap();
        let (x, y) = (a[0] + 20.0, a[1] + a[3] / 2.0);
        rig.rt.secondary_down(x, y);
        rig.rt.secondary_up(x, y);
        rig.settle();
        rig.click_text("Delete group and snippets");
        let lib = rig.library();
        assert!(!lib
            .groups
            .iter()
            .any(|g| g.id == parent.id || g.id == child.id));
        assert!(!lib.snippets.iter().any(|s| s.id == snippet.id));
    }
}

#[test]
fn context_menu_is_clickable_beyond_the_sidebar_edge() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (x, y) = (a[0] + a[2] - 20.0, a[1] + a[3] / 2.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    let menu = rig.rect("sidebar-group-menu").unwrap();
    let picked = rig.rt.element_at(menu[0] + 100.0, menu[1] + 14.0).unwrap();
    assert!(picked.path().contains("sidebar-group-menu"));
}

#[test]
fn clipboard_folder_displays_the_entry_count() {
    let mut rig = Rig::new(true);
    rig.fixtures.pasteboard.external_copy(vec![vec![(
        "public.utf8-plain-text".into(),
        b"fixture".to_vec(),
    )]]);
    rig.kudzu.clipboard.poll().unwrap();
    rig.settle();
    let e = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("sidebar-clip-count-text"))
        .unwrap();
    assert_eq!(e.text.as_deref(), Some("1"));
}

#[test]
fn clipboard_section_indicates_when_recording_is_paused() {
    let mut rig = Rig::new(true);
    rig.kudzu
        .settings
        .update(|s| s.clipboard_enabled = false)
        .unwrap();
    rig.settle();
    assert!(rig.rect("sidebar-clipboard-paused").is_some());
}

#[test]
fn context_menu_can_be_used_from_the_keyboard() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (x, y) = (a[0] + 20.0, a[1] + 14.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
}

#[test]
fn context_menu_arrow_down_selects_the_next_action() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (x, y) = (a[0] + 20.0, a[1] + 14.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    rig.press(rune::KeyEvent {
        key: rune::Key::Down,
        mods: rune::Mods::default(),
    });
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods::default(),
    });
    let id = rig.rt.state.sidebar.rename.expect("new subgroup rename");
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == id)
            .unwrap()
            .parent_id,
        Some(group.id)
    );
}

#[test]
fn option_return_starts_group_rename() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", group.id));
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods {
            alt: true,
            ..Default::default()
        },
    });
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
}

#[test]
fn blocked_status_opens_general_settings() {
    let mut rig = Rig::new(true);
    let mut status = (*rig.kudzu.expansion.status().get()).clone();
    status.paused = false;
    status.tap = kudzu_core::system::TapMode::Off;
    rig.kudzu.expansion.status_changed(status);
    rig.rt.update(|_, _| {});
    rig.settle();
    rig.click_key("sidebar-status");
    assert!(matches!(
        rig.rt.state.frame.view,
        MainView::Settings(crate::shell::SettingsPane::General)
    ));
}

#[test]
fn status_footer_refreshes_when_engine_status_changes() {
    let mut rig = Rig::new(true);
    let mut status = (*rig.kudzu.expansion.status().get()).clone();
    status.paused = false;
    status.tap = kudzu_core::system::TapMode::Off;
    rig.kudzu.expansion.status_changed(status);
    rig.settle();
    assert!(rig.rt.elements().iter().any(|e| e
        .label
        .as_deref()
        .is_some_and(|label| label.starts_with("Status: Needs permission."))));
}

#[test]
fn context_menu_moves_a_child_to_top_level() {
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
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", child.id)).unwrap();
    let (x, y) = (a[0] + 20.0, a[1] + 14.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    rig.click_text("Move to top level");
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == child.id)
            .unwrap()
            .parent_id,
        None
    );
}

#[test]
fn selection_pill_tracks_its_group_when_sidebar_scrolls() {
    let mut rig = Rig::new(true);
    let mut selected = None;
    for index in 0..30 {
        let (group, _) = rig
            .kudzu
            .library
            .create_group(format!("Group {index:02}"), None)
            .unwrap();
        if index == 4 {
            selected = Some(group.id);
        }
    }
    let selected = selected.unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{selected}"));
    super::wait_motion(&mut rig);
    let before = rig
        .rt
        .presented(&format!("sidebar-group-{selected}"))
        .unwrap()
        .0[1];
    rig.rt.wheel(100.0, 300.0, -80.0);
    super::wait_motion(&mut rig);
    let row = rig
        .rt
        .presented(&format!("sidebar-group-{selected}"))
        .unwrap()
        .0;
    let pill = rig.rt.presented("sidebar-selection-pill").unwrap().0;
    assert!((row[1] - before).abs() > 2.0);
    assert!((pill[1] - row[1]).abs() < 2.0);
}

#[test]
fn scrolled_out_selected_group_does_not_paint_over_the_footer() {
    let mut rig = Rig::new(true);
    let mut selected = None;
    for index in 0..30 {
        let (group, _) = rig
            .kudzu
            .library
            .create_group(format!("Group {index:02}"), None)
            .unwrap();
        if index == 1 {
            selected = Some(group.id);
        }
    }
    let selected = selected.unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{selected}"));
    super::wait_motion(&mut rig);
    rig.rt.wheel(100.0, 300.0, -900.0);
    super::wait_motion(&mut rig);
    let row = rig
        .rt
        .presented(&format!("sidebar-group-{selected}"))
        .unwrap()
        .0;
    let nav = rig.rt.presented("sidebar-nav").unwrap().0;
    assert!(row[1] + row[3] < nav[1]);
    let pill = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("sidebar-selection-pill"))
        .expect("pill remains mounted in the scroll viewport");
    assert!(pill.visible[2] == 0.0 || pill.visible[3] == 0.0);
}

#[test]
fn selection_pill_stays_with_row_during_scroll_motion() {
    let mut rig = Rig::new(true);
    let mut selected = None;
    for index in 0..30 {
        let (group, _) = rig
            .kudzu
            .library
            .create_group(format!("Group {index:02}"), None)
            .unwrap();
        if index == 7 {
            selected = Some(group.id);
        }
    }
    let selected = selected.unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{selected}"));
    super::wait_motion(&mut rig);
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Group(selected))
    );
    rig.rt.wheel(100.0, 300.0, -80.0);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let animating = rig.rt.frame(1.0 / 120.0);
        let row = rig
            .rt
            .presented(&format!("sidebar-group-{selected}"))
            .unwrap()
            .0;
        let pill = rig.rt.presented("sidebar-selection-pill").unwrap().0;
        assert!(
            (pill[1] - row[1]).abs() < 2.0,
            "pill {}, row {}",
            pill[1],
            row[1]
        );
        if !animating {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "scroll motion did not finish"
        );
    }
}
