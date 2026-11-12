use super::{LibraryFilter, MainView, Rig};

#[test]
fn snippet_filters_show_counts() {
    let mut rig = Rig::new(true);
    let before = rig.library().snippets.len();
    rig.kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft::default())
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let all = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("sidebar-count-all"))
        .unwrap();
    assert_eq!(all.text.as_deref(), Some((before + 1).to_string().as_str()));
}

#[test]
fn snippet_drag_payload_drops_on_the_visible_group() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Destination".into(), None)
        .unwrap();
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft::default())
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let rect = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    rig.rt.update(move |st, tx| {
        crate::dnd::finish_drag(
            st,
            crate::dnd::DragPayload::Snippets(vec![snippet.id]),
            rect[0] + 20.0,
            rect[1] + 14.0,
            tx,
        )
    });
    rig.settle();
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
fn snippet_drag_ghost_displays_selection_count() {
    let mut rig = Rig::new(true);
    let ids = vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()];
    rig.rt.update(move |st, _| {
        crate::dnd::move_drag(st, crate::dnd::DragPayload::Snippets(ids), 100.0, 300.0)
    });
    rig.settle();
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some("2 snippets")));
}

#[test]
fn group_drag_keeps_a_drop_indicator_when_moving_within_target() {
    let mut rig = Rig::new(true);
    let (source, _) = rig
        .kudzu
        .library
        .create_group("Source".into(), None)
        .unwrap();
    let (target, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let a = rig.rect(&format!("sidebar-group-{}", source.id)).unwrap();
    let b = rig.rect(&format!("sidebar-group-{}", target.id)).unwrap();
    let (ax, ay, bx, by) = (a[0] + 20.0, a[1] + 14.0, b[0] + 20.0, b[1] + 14.0);
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.rt.pointer_move(bx, by);
    rig.settle();
    assert!(rig.rect("sidebar-drop-indicator").is_some());
    rig.rt.pointer_move(bx + 1.0, by);
    rig.settle();
    assert!(rig.rect("sidebar-drop-indicator").is_some());
    rig.rt.pointer_up(bx + 1.0, by);
    rig.settle();
}

#[test]
fn selection_pill_follows_the_active_sidebar_row() {
    let mut rig = Rig::new(true);
    rig.settle();
    assert!(rig.rect("sidebar-selection-pill").is_some());
    rig.click_key("sidebar-item-favorites");
    super::wait_motion(&mut rig);
    let pill = rig.rect("sidebar-selection-pill").unwrap();
    let row = rig.rect("sidebar-item-favorites").unwrap();
    assert!((pill[1] - row[1]).abs() < 2.0);
}

#[test]
fn selection_pill_motion_gate() {
    let mut rig = Rig::new(true);
    super::wait_motion(&mut rig);
    let mut sampler = rune::motion_gate::Sampler::new(&mut rig.rt, 120.0);
    sampler.before(&mut rig.rt);
    let awake = rig.rt.frame(1.0 / 120.0);
    sampler.frame(&mut rig.rt, awake);
    let start = rig.rt.presented("sidebar-selection-pill").unwrap().0[1];
    sampler.input("select Favorites");
    rune::motion_gate::Run::tap(&mut rig.rt, "sidebar-item-favorites");
    let mut positions = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        sampler.before(&mut rig.rt);
        let awake = rig.rt.frame(1.0 / 120.0);
        sampler.frame(&mut rig.rt, awake);
        positions.push(rig.rt.presented("sidebar-selection-pill").unwrap().0[1]);
        if !awake {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "selection motion did not finish"
        );
    }
    let end = *positions.last().unwrap();
    assert!(positions.iter().any(|y| *y > start + 1.0 && *y < end - 1.0));
    let findings: Vec<_> = rune::motion_gate::check(&sampler.rec)
        .into_iter()
        .filter(|f| f.element.contains("sidebar-selection-pill"))
        .collect();
    assert!(findings.is_empty(), "{findings:?}");
}

#[test]
fn folding_selected_section_hides_the_selection_pill() {
    let mut rig = Rig::new(true);
    rig.settle();
    rig.click_key("sidebar-section-snippets");
    rig.settle();
    assert!(rig.rect("sidebar-selection-pill").is_none());
}

#[test]
fn selection_pill_does_not_lag_the_row_when_section_folds() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-item-pinned");
    super::wait_motion(&mut rig);
    rune::motion_gate::Run::tap(&mut rig.rt, "sidebar-section-snippets");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let animating = rig.rt.frame(1.0 / 120.0);
        let pill = rig
            .rt
            .presented("sidebar-selection-pill")
            .expect("selection pill");
        let row = rig.rt.presented("sidebar-item-pinned").expect("pinned row");
        assert!((pill.0[1] - row.0[1]).abs() < 2.0);
        if !animating {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "fold motion did not finish"
        );
    }
}

#[test]
fn group_rename_commits_when_focus_moves_away() {
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
        .update(|st, _| st.sidebar.edit.replace_text("  Updated  "));
    rig.click_key("sidebar-item-favorites");
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == group.id)
            .unwrap()
            .name,
        "Updated"
    );
}

#[test]
fn keyboard_navigation_continues_from_the_fixed_footer() {
    let mut rig = Rig::new(true);
    rig.click_key("sidebar-item-favorites");
    rig.press(rune::KeyEvent {
        key: rune::Key::End,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.focused_key().as_deref(), Some("sidebar-pause"));
    rig.press(rune::KeyEvent {
        key: rune::Key::Up,
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.focused_key().as_deref(), Some("sidebar-item-settings"));
}

#[test]
fn group_menu_opens_settings_and_toggles_enabled() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Menu target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let open = |rig: &mut Rig| {
        let a = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
        let (x, y) = (a[0] + 20.0, a[1] + 14.0);
        rig.rt.secondary_down(x, y);
        rig.rt.secondary_up(x, y);
        rig.settle();
    };
    open(&mut rig);
    rig.click_text("Turn off group");
    assert!(
        !rig.library()
            .groups
            .iter()
            .find(|g| g.id == group.id)
            .unwrap()
            .is_enabled
    );
    open(&mut rig);
    rig.click_text("Group settings…");
    assert_eq!(rig.rt.state.frame.group_settings_id, Some(group.id));
    assert!(rig.rect("group-settings-dialog").is_some());
}

#[test]
fn failed_group_rename_keeps_the_draft() {
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
    assert_eq!(rig.rt.state.sidebar.edit.text, "Changed");
}

#[test]
fn deleting_the_selected_group_returns_to_all() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", group.id));
    let rect = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (x, y) = (rect[0] + 20.0, rect[1] + 14.0);
    rig.rt.secondary_down(x, y);
    rig.rt.secondary_up(x, y);
    rig.settle();
    rig.click_text("Delete group, keep snippets");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
}

#[test]
fn escape_closes_group_context_menu() {
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
    assert!(rig.rect("sidebar-group-menu").is_some());
    rig.press(super::super::esc());
    assert!(rig.rt.state.sidebar.menu.is_none());
}

#[test]
fn group_drag_rejects_descendant_target() {
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
    let a = rig.rect(&format!("sidebar-group-{}", parent.id)).unwrap();
    let b = rig.rect(&format!("sidebar-group-{}", child.id)).unwrap();
    let (ax, ay, bx, by) = (a[0] + 20.0, a[1] + 14.0, b[0] + 20.0, b[1] + 14.0);
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.rt.pointer_move(bx, by);
    rig.settle();
    assert!(rig.rt.state.sidebar.drop_target.is_none());
    rig.rt.pointer_up(bx, by);
    rig.settle();
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == parent.id)
            .unwrap()
            .parent_id,
        None
    );
}

#[test]
fn dragging_a_library_row_to_a_sidebar_group_moves_the_snippet() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Destination".into(), None)
        .unwrap();
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft {
            name: "Drag source".into(),
            ..Default::default()
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.settle();
    let a = rig
        .rect(&crate::main::library::row_key(snippet.id))
        .expect("snippet row");
    let b = rig
        .rect(&format!("sidebar-group-{}", group.id))
        .expect("group row");
    let (ax, ay, bx, by) = (a[0] + 40.0, a[1] + 20.0, b[0] + 20.0, b[1] + 14.0);
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.settle();
    rig.rt.pointer_move(bx, by);
    rig.settle();
    assert!(rig.rect("sidebar-drag-ghost").is_some());
    rig.rt.pointer_up(bx, by);
    rig.settle();
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
fn cancelling_a_library_drag_removes_the_ghost() {
    let mut rig = Rig::new(true);
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft {
            name: "Drag source".into(),
            ..Default::default()
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.settle();
    let a = rig
        .rect(&crate::main::library::row_key(snippet.id))
        .unwrap();
    let (x, y) = (a[0] + 40.0, a[1] + 20.0);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.settle();
    rig.rt.pointer_move(x + 20.0, y + 20.0);
    rig.settle();
    rig.settle();
    assert!(rig.rect("sidebar-drag-ghost").is_some());
    rig.press(super::super::esc());
    assert!(rig.rect("sidebar-drag-ghost").is_none());
}

#[test]
fn dragging_a_library_row_to_groups_unparents_the_snippet() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Group".into(), None)
        .unwrap();
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft {
            name: "Drag source".into(),
            group_id: Some(group.id),
            ..Default::default()
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.settle();
    let a = rig
        .rect(&crate::main::library::row_key(snippet.id))
        .unwrap();
    let b = rig.rect("sidebar-groups-root").unwrap();
    let (ax, ay, bx, by) = (a[0] + 40.0, a[1] + 20.0, b[0] + 20.0, b[1] + 8.0);
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.settle();
    rig.rt.pointer_move(bx, by);
    rig.settle();
    rig.rt.pointer_up(bx, by);
    rig.settle();
    assert_eq!(
        rig.library()
            .snippets
            .iter()
            .find(|s| s.id == snippet.id)
            .unwrap()
            .group_id,
        None
    );
}

#[test]
fn quick_snippet_drop_clears_the_ghost_and_hover() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Destination".into(), None)
        .unwrap();
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft {
            name: "Drag source".into(),
            ..Default::default()
        })
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.settle();
    let a = rig
        .rect(&crate::main::library::row_key(snippet.id))
        .unwrap();
    let b = rig.rect(&format!("sidebar-group-{}", group.id)).unwrap();
    let (ax, ay, bx, by) = (a[0] + 40.0, a[1] + 20.0, b[0] + 20.0, b[1] + 14.0);
    rig.rt.pointer_move(ax, ay);
    rig.rt.pointer_down(ax, ay);
    rig.settle();
    rig.rt.pointer_move(bx, by);
    rig.rt.pointer_up(bx, by);
    rig.settle();
    assert_eq!(
        rig.library()
            .snippets
            .iter()
            .find(|s| s.id == snippet.id)
            .unwrap()
            .group_id,
        Some(group.id)
    );
    assert!(rig.rt.state.sidebar.drag.is_none());
    assert!(rig.rt.state.sidebar.drop_target.is_none());
}

#[test]
fn typing_after_rename_starts_replaces_the_selected_name() {
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
    rig.press(rune::KeyEvent {
        key: rune::Key::Text("R".into()),
        mods: rune::Mods::default(),
    });
    assert_eq!(rig.rt.state.sidebar.edit.text, "R");
}

#[test]
fn context_menu_stays_inside_the_window_near_bottom() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Target".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.rt
        .update(move |st, _| st.sidebar.menu = Some((group.id, 180.0, 750.0)));
    rig.settle();
    let menu = rig.rect("sidebar-group-menu").unwrap();
    assert!(menu[1] + menu[3] <= 760.0);
}

#[test]
fn clicking_outside_closes_the_context_menu() {
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
    rig.rt.pointer_move(900.0, 500.0);
    rig.rt.pointer_down(900.0, 500.0);
    rig.rt.pointer_up(900.0, 500.0);
    rig.settle();
    assert!(rig.rt.state.sidebar.menu.is_none());
}
