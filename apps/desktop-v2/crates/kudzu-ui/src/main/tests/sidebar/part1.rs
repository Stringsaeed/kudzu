use super::Rig;

#[test]
fn dropping_snippets_on_a_group_moves_them() {
    use kudzu_app::library::SnippetDraft;
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Destination".into(), None)
        .unwrap();
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft::default())
        .unwrap();
    rig.rt.update(move |st, _| {
        crate::dnd::drop_payload(
            st,
            crate::dnd::DragPayload::Snippets(vec![snippet.id]),
            Some(group.id),
        );
    });
    assert_eq!(
        rig.library()
            .snippets
            .iter()
            .find(|s| s.id == snippet.id)
            .unwrap()
            .group_id,
        Some(group.id)
    );
}

#[test]
fn f2_starts_rename_of_focused_group() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Rename".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", group.id));
    rig.press(rune::KeyEvent {
        key: rune::Key::Function(2),
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.rt.state.sidebar.rename, Some(group.id));
}

#[test]
fn rename_enter_trims_and_saves_the_group_name() {
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
    rig.rt.update(|st, _| {
        st.sidebar.edit.replace_text("  Renamed  ");
    });
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
        "Renamed"
    );
}

#[test]
fn dragging_a_group_onto_another_group_reparents_it() {
    let mut rig = Rig::new(true);
    let (source, _) = rig
        .kudzu
        .library
        .create_group("Source".into(), None)
        .unwrap();
    let (target, _) = rig
        .kudzu
        .library
        .create_group("Destination".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", source.id)).unwrap();
    let b = rig.rect(&format!("sidebar-group-{}", target.id)).unwrap();
    let (ax, ay, bx, by) = (
        a[0] + 20.0,
        a[1] + a[3] / 2.0,
        b[0] + 20.0,
        b[1] + b[3] / 2.0,
    );
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.rt.pointer_move(bx, by);
    rig.rt.pointer_up(bx, by);
    rig.settle();
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == source.id)
            .unwrap()
            .parent_id,
        Some(target.id)
    );
}

#[test]
fn group_drag_shows_a_named_ghost() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Source".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (x, y) = (a[0] + 20.0, a[1] + a[3] / 2.0);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_move(x + 15.0, y + 15.0);
    rig.settle();
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.path.contains("sidebar-drag-ghost") && e.text.as_deref() == Some("Source")));
    rig.rt.pointer_up(x + 15.0, y + 15.0);
    rig.settle();
    assert!(rig.rt.state.sidebar.drag.is_none());
    assert!(rig.rt.state.sidebar.drop_target.is_none());
    assert!(rig.rect("sidebar-drag-ghost").is_none());
}

#[test]
fn dragging_a_child_to_the_groups_heading_unparents_it() {
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
    let (ax, ay, bx, by) = (
        a[0] + 20.0,
        a[1] + a[3] / 2.0,
        b[0] + 20.0,
        b[1] + b[3] / 2.0,
    );
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.rt.pointer_move(bx, by);
    rig.rt.pointer_up(bx, by);
    rig.settle();
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
fn folding_snippets_hides_rows_and_persists_the_fold() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-section-snippets");
    assert!(!rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Favorites"));
    rig.kudzu.shutdown();
    let path = rig._scratch.0.join("ui-state.json");
    assert!(path.exists());
    assert!(std::fs::read_to_string(path).unwrap().contains("snippets"));
}

#[test]
fn collapsed_sidebar_keeps_panel_width_and_slides_it_left() {
    let mut rig = Rig::new(true);
    let initial = rig.rect("sidebar-panel").unwrap();
    assert!((initial[2] - 224.0).abs() < 2.0);
    rig.rt.update(|st, _| st.toggle_sidebar());
    super::wait_motion(&mut rig);
    assert!(!rig.rt.frame(1.0 / 120.0));
    let collapsed = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("sidebar-panel"))
        .unwrap();
    assert!((collapsed.rect[0] + 216.0).abs() < 2.0);
    assert!((collapsed.rect[2] - 224.0).abs() < 2.0);
}

#[test]
fn new_group_button_starts_renaming_the_new_group() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-new-group");
    assert!(rig.rt.state.sidebar.rename.is_some());
    assert!(rig.rect("sidebar-group-rename").is_some());
}

#[test]
fn arrow_keys_move_focus_between_visible_groups() {
    let mut rig = Rig::new(true);
    let (first, _) = rig
        .kudzu
        .library
        .create_group("First".into(), None)
        .unwrap();
    let (second, _) = rig
        .kudzu
        .library
        .create_group("Second".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", first.id));
    rig.press(rune::KeyEvent {
        key: rune::Key::Down,
        mods: rune::Mods::default(),
    });
    assert_eq!(
        rig.focused_key().as_deref(),
        Some(format!("sidebar-group-{}", second.id).as_str())
    );
}
