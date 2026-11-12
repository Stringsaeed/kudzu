use super::Rig;

#[test]
fn footer_shows_each_health_state_copy() {
    use kudzu_core::system::TapMode;
    let mut rig = Rig::new(true);
    let mut ready = (*rig.kudzu.expansion.status().get()).clone();
    ready.paused = false;
    ready.secure_input = false;
    ready.tap = TapMode::Active;
    ready.permissions.accessibility = true;
    ready.permissions.input_monitoring = true;
    let mut paused = ready.clone();
    paused.paused = true;
    let mut secure = ready.clone();
    secure.secure_input = true;
    let mut blocked = ready.clone();
    blocked.tap = TapMode::Off;
    let mut limited = ready.clone();
    limited.tap = TapMode::ListenOnly;
    for (status, expected) in [
        (ready, "Status: Expanding. Kudzu is listening for your triggers in every app."),
        (paused, "Status: Paused. Nothing expands until you resume."),
        (secure, "Status: Secure Input on. A password field or app has Secure Input on, so macOS blocks expansion."),
        (blocked, "Status: Needs permission. Kudzu can't see your typing yet. Open Settings → Permissions."),
        (limited, "Status: Limited. Grant Accessibility so Kudzu can type expansions reliably."),
    ] {
        rig.kudzu.expansion.status_changed(status);
        rig.settle();
        assert!(rig.rt.elements().iter().any(|element| element.label.as_deref() == Some(expected)), "missing {expected}");
    }
}

#[test]
fn secure_input_footer_names_the_owner_when_known() {
    let mut rig = Rig::new(true);
    let mut status = (*rig.kudzu.expansion.status().get()).clone();
    status.paused = false;
    status.secure_input = true;
    status.secure_input_owner = Some("Example App".into());
    rig.kudzu.expansion.status_changed(status);
    rig.settle();
    assert!(rig.rt.elements().iter().any(|e| e.label.as_deref() == Some("Status: Secure Input on. Example App has Secure Input on, so macOS blocks expansion.")));
}

#[test]
fn rename_enter_and_escape_leave_the_ime_in_control() {
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
        key: rune::Key::Function(2),
        mods: rune::Mods::default(),
    });
    rig.rt.ime(rune::Ime::Preedit("x".into(), Some((0, 1))));
    rig.settle();
    assert!(rig.rt.state.sidebar.edit.is_composing());
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
    rig.press(super::super::esc());
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
}

#[test]
fn context_menu_keyboard_is_inert_during_ime() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let r = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (x, y) = (r[0] + 20.0, r[1] + 14.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    rig.rt.update(|st, _| {
        st.sidebar
            .edit
            .ime(&rune::Ime::Preedit("x".into(), Some((0, 1))));
    });
    assert!(rig.rt.state.is_composing());
    let focus = rig.focused_key();
    rig.press(rune::KeyEvent {
        key: rune::Key::Down,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.focused_key(), focus);
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods::default(),
    });
    assert!(rig.rt.state.sidebar.rename.is_none());
    assert!(rig.rt.state.sidebar.menu.is_some());
}

#[test]
fn root_drop_target_highlights_during_drag_and_clears_afterward() {
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
    let b = rig.rect("sidebar-groups-root").unwrap();
    let (ax, ay, bx, by) = (a[0] + 20.0, a[1] + 14.0, b[0] + 20.0, b[1] + 8.0);
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.rt.pointer_move(bx, by);
    rig.settle();
    assert!(rig.rect("sidebar-root-drop-indicator").is_some());
    rig.rt.pointer_move(bx + 1.0, by);
    rig.settle();
    assert!(rig.rect("sidebar-root-drop-indicator").is_some());
    rig.rt.pointer_up(bx + 1.0, by);
    rig.settle();
    assert!(rig.rect("sidebar-root-drop-indicator").is_none());
}

#[test]
fn folded_section_keeps_its_rows_mounted_and_inert() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-section-snippets");
    let row = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("sidebar-item-favorites"))
        .expect("row remains mounted");
    assert!(row.inert);
    assert!(!rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Favorites"));
}

#[test]
fn folded_group_keeps_descendants_mounted_and_inert() {
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
    rig.click_key(&format!("sidebar-collapse-{}", parent.id));
    let key = format!("sidebar-group-{}", child.id);
    let row = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&key))
        .expect("child remains mounted");
    assert!(row.inert);
    assert!(!rig.rt.accessibility_json().to_string().contains("Child"));
}

#[test]
fn selection_pill_hides_when_its_parent_group_is_folded() {
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
    rig.click_key(&format!("sidebar-group-{}", child.id));
    super::wait_motion(&mut rig);
    assert!(rig.rect("sidebar-selection-pill").is_some());
    rig.click_key(&format!("sidebar-collapse-{}", parent.id));
    assert!(rig.rect("sidebar-selection-pill").is_none());
}

#[test]
fn folded_clipboard_keeps_folders_mounted_and_inert() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-section-clipboard");
    let row = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("sidebar-item-history"))
        .expect("folder remains mounted");
    assert!(row.inert);
    assert!(!rig.rt.accessibility_json().to_string().contains("History"));
}

#[test]
fn blank_group_rename_keeps_the_existing_name() {
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
    rig.rt.update(|st, _| st.sidebar.edit.replace_text("   "));
    rig.press(rune::KeyEvent {
        key: rune::Key::Enter,
        mods: rune::Mods::default(),
    });
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == group.id)
            .unwrap()
            .name,
        "Original"
    );
    assert!(rig.rt.state.sidebar.rename.is_none());
}

#[test]
fn section_fold_animates_then_comes_to_rest() {
    let mut rig = Rig::new(true);
    super::wait_motion(&mut rig);
    let before = rig.rt.presented("sidebar-region-clipboard").unwrap().0[1];
    rune::motion_gate::Run::tap(&mut rig.rt, "sidebar-section-snippets");
    assert!(rig.rt.frame(1.0 / 120.0), "fold should animate");
    super::wait_motion(&mut rig);
    let after = rig.rt.presented("sidebar-region-clipboard").unwrap().0[1];
    assert!(after < before);
    assert!(!rig.rt.frame(1.0 / 120.0), "fold should come to rest");
}

#[test]
fn selection_pill_tracks_a_group_while_a_previous_group_folds() {
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
    let (target, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", target.id));
    super::wait_motion(&mut rig);
    rune::motion_gate::Run::tap(&mut rig.rt, &format!("sidebar-collapse-{}", parent.id));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let animating = rig.rt.frame(1.0 / 120.0);
        let row = rig
            .rt
            .presented(&format!("sidebar-group-{}", target.id))
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
            "group fold did not finish"
        );
    }
}

#[test]
fn group_fold_motion_state_expires() {
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
    rune::motion_gate::Run::tap(&mut rig.rt, &format!("sidebar-collapse-{}", parent.id));
    assert!(rig.rt.frame(1.0 / 120.0), "group fold should animate");
    super::wait_motion(&mut rig);
    assert!(!rig.rt.frame(1.0 / 120.0), "group fold should come to rest");
}
