mod part1;
mod part2;
mod part3;
mod part4;
mod part5;

use super::Rig;
use crate::main::{ClipFilter, LibraryFilter, MainView};

fn wait_motion(rig: &mut Rig) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while rig.rt.frame(1.0 / 120.0) {
        assert!(
            std::time::Instant::now() < deadline,
            "sidebar motion did not finish"
        );
    }
}

#[test]
fn sidebar_navigation_changes_views() {
    let mut rig = Rig::new(true);
    rig.click_text("Favorites");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Favorites)
    );
    rig.click_text("Pinned");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Clipboard(ClipFilter::Pinned)
    );
    rig.click_text("Insights");
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
}

#[test]
fn empty_active_clipboard_folder_remains_visible() {
    let mut rig = Rig::new(true);
    rig.rt
        .update(|st, _| st.navigate_to(MainView::Clipboard(ClipFilter::Image)));
    rig.settle();
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|element| element.role == "button" && element.label.as_deref() == Some("Images")));
    assert!(rig.rect("sidebar-clip-count-image").is_none());
}

#[test]
fn groups_and_pause_are_available_in_sidebar() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Example".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.role == "button" && e.label.as_deref() == Some("Example")));
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.role == "button" && e.label.as_deref() == Some("New group")));
    rig.click_text("Example");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Group(group.id))
    );
    let was_paused = rig.kudzu.settings.watch().get().paused;
    rig.click_key("sidebar-pause");
    assert_ne!(rig.kudzu.settings.watch().get().paused, was_paused);
}

#[test]
fn group_context_menu_offers_group_actions() {
    for theme in [
        kudzu_app::settings::Theme::Light,
        kudzu_app::settings::Theme::Dark,
    ] {
        let mut rig = Rig::themed(true, theme);
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
        let rect = rig
            .rt
            .elements()
            .into_iter()
            .find(|e| e.role == "button" && e.label.as_deref() == Some("Child"))
            .unwrap()
            .rect;
        let (x, y) = (rect[0] + 20.0, rect[1] + rect[3] / 2.0);
        rig.rt.secondary_down(x, y);
        rig.rt.secondary_up(x, y);
        rig.settle();
        for label in [
            "Rename",
            "New subgroup",
            "Group settings…",
            "Turn off group",
            "Move to top level",
            "Delete group, keep snippets",
            "Delete group and snippets",
        ] {
            assert!(
                rig.rt
                    .elements()
                    .iter()
                    .any(|e| e.label.as_deref() == Some(label) || e.text.as_deref() == Some(label)),
                "missing {label}"
            );
        }
    }
}

#[test]
fn group_menu_delete_keep_snippets_moves_them_to_root() {
    use kudzu_app::library::SnippetDraft;
    for theme in [
        kudzu_app::settings::Theme::Light,
        kudzu_app::settings::Theme::Dark,
    ] {
        let mut rig = Rig::themed(true, theme);
        let (group, _) = rig
            .kudzu
            .library
            .create_group("Delete me".into(), None)
            .unwrap();
        let (snippet, _) = rig
            .kudzu
            .library
            .create_snippet(SnippetDraft {
                name: "Example".into(),
                group_id: Some(group.id),
                ..Default::default()
            })
            .unwrap();
        rig.rt.update(|st, _| st.refresh_library_snapshot());
        rig.settle();
        let rect = rig
            .rt
            .elements()
            .into_iter()
            .find(|e| e.role == "button" && e.label.as_deref() == Some("Delete me"))
            .unwrap()
            .rect;
        let (x, y) = (rect[0] + 20.0, rect[1] + rect[3] / 2.0);
        rig.rt.secondary_down(x, y);
        rig.rt.secondary_up(x, y);
        rig.settle();
        rig.click_text("Delete group, keep snippets");
        let library = rig.library();
        assert!(!library.groups.iter().any(|g| g.id == group.id));
        assert_eq!(
            library
                .snippets
                .iter()
                .find(|s| s.id == snippet.id)
                .unwrap()
                .group_id,
            None
        );
    }
}

#[test]
fn group_collapse_hides_descendants_from_accessibility() {
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
    assert!(!rig.rt.accessibility_json().to_string().contains("Child"));
}

#[test]
fn disabled_filter_only_appears_when_needed() {
    use kudzu_app::library::SnippetDraft;
    let mut rig = Rig::new(true);
    assert!(!rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Turned off"));
    let (mut snippet, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Example".into(),
            ..Default::default()
        })
        .unwrap();
    snippet.is_enabled = false;
    rig.kudzu.library.update_snippet(snippet).unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_text("Turned off");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Disabled)
    );
}

#[test]
fn group_counts_include_descendants() {
    use kudzu_app::library::SnippetDraft;
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
    for group_id in [parent.id, child.id] {
        rig.kudzu
            .library
            .create_snippet(SnippetDraft {
                group_id: Some(group_id),
                ..Default::default()
            })
            .unwrap();
    }
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    let count = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&format!("sidebar-count-{}", parent.id)))
        .unwrap();
    assert_eq!(count.text.as_deref(), Some("2"));
}

#[test]
fn escape_cancels_double_click_group_rename() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Rename me".into(), None)
        .unwrap();
    rig.rt.update(|st, _| st.refresh_library_snapshot());
    rig.settle();
    rig.click_key(&format!("sidebar-group-{}", group.id));
    rig.click_key(&format!("sidebar-group-{}", group.id));
    assert!(rig.rt.state.sidebar.rename == Some(group.id));
    rig.press(super::esc());
    assert!(rig.rt.state.sidebar.rename.is_none());
    assert!(rig
        .library()
        .groups
        .iter()
        .any(|g| g.id == group.id && g.name == "Rename me"));
}

#[test]
fn dropping_a_group_on_its_descendant_is_rejected() {
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
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == parent.id)
            .unwrap()
            .parent_id,
        None
    );
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|g| g.id == child.id)
            .unwrap()
            .parent_id,
        Some(parent.id)
    );
}

#[test]
fn dropping_a_group_on_itself_is_rejected() {
    let mut rig = Rig::new(true);
    let (group, _) = rig
        .kudzu
        .library
        .create_group("Parent".into(), None)
        .unwrap();
    rig.rt.update(move |st, _| {
        crate::dnd::drop_payload(st, crate::dnd::DragPayload::Group(group.id), Some(group.id));
    });
    assert_eq!(
        rig.library()
            .groups
            .iter()
            .find(|item| item.id == group.id)
            .unwrap()
            .parent_id,
        None
    );
}
