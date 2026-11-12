//! Frame geometry, sidebar, breadcrumbs, arrivals and onboarding routing.

use kudzu_app::library::SnippetDraft;

use kudzu_core::KudzuLibrary;

use super::{cmd, Rig};
use crate::main::state::{
    clip_filter_key, clip_filter_label, ClipFilter, LibraryFilter, MainPane, MainView, CLIP_FOLDERS,
};
use crate::main::toolbar::{collapse_crumbs, compute_crumbs, VisibleCrumb};
use crate::shell::Route;

#[test]
fn frame_geometry_titlebar_inset_and_sidebar_collapse_sync_vibrancy_and_inertness() {
    let mut rig = Rig::new(true);

    // Headless / fullscreen default (`titlebar_inset == 0`): 12pt left toolbar padding.
    assert_eq!(rig.rect("main-toolbar"), Some([0.0, 0.0, 1180.0, 42.0]));
    let toggle_rect = rig.rect("toolbar-sidebar-toggle").unwrap();
    assert!(
        (toggle_rect[0] - 12.0).abs() < 0.5 && toggle_rect[2] == 32.0 && toggle_rect[3] == 32.0
    );
    let search_rect = rig.rect("toolbar-search-trigger").unwrap();
    assert_eq!((search_rect[2], search_rect[3]), (192.0, 32.0));
    assert!((search_rect[0] - (1180.0 - 16.0 - 192.0)).abs() < 0.5);

    // Sidebar open: 224pt wide, vibrancy material synced, `<main>` card inset by 8pt top/bottom/right.
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 224.0, 718.0]));
    assert_eq!(
        rig.rt.state.frame.sidebar_material_rect,
        Some([0.0, 42.0, 224.0, 718.0])
    );
    assert_eq!(rig.rect("main-content"), Some([224.0, 50.0, 948.0, 702.0]));

    // Traffic-light inset (`titlebar_inset > 0`): toolbar shifts left padding to 88pt.
    rig.rt.set_titlebar_inset(28.0);
    rig.rt.settle_all();
    rig.rt.frame(0.016);
    let inset_toggle = rig.rect("toolbar-sidebar-toggle").unwrap();
    assert!((inset_toggle[0] - 88.0).abs() < 0.5);

    // Move logical pane focus to Sidebar, then collapse via `⌘\`.
    rig.rt.state.frame.focus.move_to(MainPane::Sidebar);
    rig.press(cmd('\\'));

    assert!(!rig.rt.state.frame.sidebar_open);
    // Focus cannot stay trapped inside the hidden sidebar.
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    // Sidebar subtree remains mounted at width 0, vibrancy rect syncs width 0, and `<main>` gets 8pt left gutter.
    assert!(rig.rect("sidebar-nav").is_some());
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 0.0, 718.0]));
    assert_eq!(
        rig.rt.state.frame.sidebar_material_rect,
        Some([0.0, 42.0, 0.0, 718.0])
    );
    assert_eq!(rig.rect("main-content"), Some([8.0, 50.0, 1164.0, 702.0]));
    // Accessibility tree hides `"Sidebar"` and flips toggle label to `"Show sidebar"`.
    let a11y = rig.rt.accessibility_json().to_string();
    assert!(a11y.contains("Show sidebar"));
    assert!(!a11y.contains("\"label\":\"Sidebar\""));

    // Clicking the toolbar toggle re-opens the sidebar to 224pt.
    rig.click_key("toolbar-sidebar-toggle");
    assert!(rig.rt.state.frame.sidebar_open);
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 224.0, 718.0]));
}

#[test]
fn breadcrumbs_collapse_past_four_and_navigate_via_overflow_menu() {
    let mut rig = Rig::new(true);

    let (work, _) = rig.kudzu.library.create_group("Work".into(), None).unwrap();
    let (eng, _) = rig
        .kudzu
        .library
        .create_group("Engineering".into(), Some(work.id))
        .unwrap();
    let (rust_grp, _) = rig
        .kudzu
        .library
        .create_group("Rust".into(), Some(eng.id))
        .unwrap();
    let (snip, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Greeting".into(),
            content: "Hello!".into(),
            group_id: Some(rust_grp.id),
            ..SnippetDraft::default()
        })
        .unwrap();

    rig.rt.frame(0.5);
    rig.rt.update(|st, _| {
        st.navigate_to(MainView::Library(LibraryFilter::Group(rust_grp.id)));
        st.select_snippet(Some(snip.id));
    });
    rig.rt.settle_all();
    rig.rt.frame(0.5);
    rig.rt.frame(0.0);

    // 5 crumbs: Snippets > Work > Engineering > Rust > Greeting -> collapses Work & Engineering.
    let crumbs = compute_crumbs(
        &rig.rt.state.frame.view,
        rig.rt.state.frame.selected_snippet,
        rig.rt.state.library_snapshot(),
    );
    assert_eq!(crumbs.len(), 5);
    let collapsed = collapse_crumbs(&crumbs);
    assert_eq!(collapsed.len(), 4);
    assert!(matches!(&collapsed[1], VisibleCrumb::Overflow(hidden) if hidden.len() == 2));

    let a11y = rig.rt.accessibility_json().to_string();
    assert!(a11y.contains("Snippets"));
    assert!(a11y.contains("Show parent groups"));
    assert!(a11y.contains("Rust"));
    assert!(a11y.contains("Greeting"));
    assert!(!a11y.contains("\"label\":\"Work\""));
    assert!(!a11y.contains("\"label\":\"Engineering\""));

    // Clicking `"Rust"` (`crumb3`) navigates to `Group(rust_grp.id)`; opening `"Show parent groups"` exposes `"Work"`.
    rig.rt
        .update(|st, _| st.frame.breadcrumb_overflow_open = true);
    rig.rt.frame(0.016);
    let open_a11y = rig.rt.accessibility_json().to_string();
    assert!(open_a11y.contains("Work") && open_a11y.contains("Engineering"));

    // Clicking root `"Snippets"` (`crumb0`) navigates back to `Library(All)`.
    rig.rt
        .update(|st, _| st.frame.breadcrumb_overflow_open = false);
    rig.rt.frame(0.016);
    rig.click_key("crumb0");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
}

#[test]
fn breadcrumb_arrivals_ignore_mount_renames_and_bursts() {
    let mut rig = Rig::new(true);

    // Initial mount never counts as an arrival, even across idle frames on the same crumb.
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 0);
    rig.rt.frame(1.0);
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 0);

    // First navigation after mount plays its arrival (`count == 1`).
    rig.press(cmd('2')); // Favorites
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Favorites)
    );
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 1);

    // Rapid second navigation within `< 0.300s` burst window stays suppressed (`count == 1`).
    rig.rt.frame(0.05);
    rig.press(cmd('3')); // Recent (~0.066s later, < 0.300s)
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Recent)
    );
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 1);

    // Navigation after >= 0.300s increments arrival count (`count == 2`).
    rig.rt.frame(0.50);
    rig.press(cmd('4')); // Insights
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), 2);

    // Selecting a snippet after >= 0.300s increments arrival count (`count == 3`).
    let (snip, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Alpha".into(),
            content: "body".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    rig.rt.frame(0.50);
    rig.rt.update(|st, _| {
        st.refresh_library_snapshot();
        st.navigate_to(MainView::Library(LibraryFilter::All));
        st.select_snippet(Some(snip.id));
    });
    rig.rt.frame(0.05);
    let after_select = rig.rt.state.frame.breadcrumb_arrivals.count();
    assert_eq!(after_select, 3);

    // Renaming the snippet in-place (even > 1s later) keeps `id = "snippet:{uuid}"`, so `Arrivals` does NOT replay!
    let mut renamed = snip;
    renamed.name = "Alpha Renamed".into();
    rig.kudzu.library.update_snippet(renamed).unwrap();
    rig.rt.frame(1.0);
    assert_eq!(rig.rt.state.frame.breadcrumb_arrivals.count(), after_select);
    assert!(rig
        .rt
        .accessibility_json()
        .to_string()
        .contains("Alpha Renamed"));
}

#[test]
fn onboarding_route_renders_chromeless_and_transitions_to_library_when_completed() {
    let mut rig = Rig::new(false);

    assert_eq!(rig.rt.state.frame.view, MainView::Onboarding);
    assert_eq!(rig.rt.state.model.route, Route::Onboarding);
    assert_eq!(rig.rect("main-toolbar"), None);
    assert_eq!(rig.rect("main-sidebar"), None);
    assert_eq!(rig.rt.state.frame.sidebar_material_rect, None);
    assert!(rig.rect("onboarding-view-slot").is_some());

    // Completing onboarding transitions the main window frame into Library with toolbar & sidebar.
    rig.kudzu
        .settings
        .update(|s| s.onboarding_completed = true)
        .unwrap();
    rig.rt.settle_all();
    rig.rt.frame(0.1);
    rig.rt.frame(0.0);

    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
    assert_eq!(rig.rt.state.model.route, Route::Library);
    assert!(rig.rect("main-toolbar").is_some());
    assert_eq!(rig.rect("main-sidebar"), Some([0.0, 42.0, 224.0, 718.0]));
}

/// Minor 1 (S41 review): All with nothing selected ends in a plain "All snippets" leaf
/// (`toolbar.tsx:54`), and the clipboard's all-items leaf is "History" (`toolbar.tsx:36`).
#[test]
fn breadcrumbs_end_in_all_snippets_and_history() {
    let lib = KudzuLibrary::default();
    let labels = |view: MainView| -> Vec<(String, bool)> {
        compute_crumbs(&view, None, &lib)
            .into_iter()
            .map(|c| (c.label, c.target.is_some()))
            .collect()
    };
    assert_eq!(
        labels(MainView::Library(LibraryFilter::All)),
        [("Snippets".into(), true), ("All snippets".into(), false)]
    );
    assert_eq!(
        labels(MainView::Clipboard(ClipFilter::All)),
        [("Clipboard".into(), true), ("History".into(), false)]
    );
    assert_eq!(
        labels(MainView::Clipboard(ClipFilter::Pinned)),
        [("Clipboard".into(), true), ("Pinned".into(), false)]
    );
}

/// The clipboard folders follow Tauri's `FILTERS` (`lib/clipboard.ts`): order and labels.
#[test]
fn clip_folders_follow_the_tauri_filters() {
    let folders: Vec<(&str, &str)> = CLIP_FOLDERS
        .iter()
        .map(|f| (clip_filter_key(*f), clip_filter_label(*f)))
        .collect();
    assert_eq!(
        folders,
        [
            ("all", "History"),
            ("pinned", "Pinned"),
            ("text", "Text"),
            ("image", "Images"),
            ("link", "Links"),
            ("file", "Files"),
            ("code", "Code"),
            ("color", "Colors"),
        ]
    );
    let lib = KudzuLibrary::default();
    let crumbs = compute_crumbs(&MainView::Clipboard(ClipFilter::Link), None, &lib);
    assert_eq!(crumbs.last().map(|c| c.id.as_str()), Some("clip:link"));
    assert_eq!(crumbs.last().map(|c| c.label.as_str()), Some("Links"));
}

/// Minor 4 (S41 review): mount stamps the current time, so a navigation within 300 ms of mount
/// is a burst (`breadcrumbs.tsx` starts `at: 0`, the page's start); later ones still arrive.
#[test]
fn a_navigation_right_after_mount_is_a_burst() {
    let mut arrivals = crate::motion::Arrivals::default();
    assert_eq!(arrivals.observe("root:snippets", 10.0), 0);
    assert_eq!(arrivals.observe("filter:favorites", 10.1), 0);
    assert_eq!(arrivals.observe("filter:recent", 10.6), 1);
}

/// Minor 5 (S41 review): Search is 192 pt wide, and 144 pt below a 1000 pt window
/// (`w-48 max-[1000px]:w-36`).
#[test]
fn search_narrows_below_a_thousand_points() {
    let mut rig = Rig::new(true);
    let width = |rig: &Rig| rig.rect("toolbar-search-trigger").unwrap()[2];
    assert_eq!(width(&rig), 192.0);
    rig.rt.set_viewport((999.0, 760.0), 1.0);
    rig.settle();
    rig.settle();
    assert_eq!(width(&rig), 144.0);
    let r = rig.rect("toolbar-search-trigger").unwrap();
    assert!((r[0] + r[2] - (999.0 - 16.0)).abs() < 0.5, "{r:?}");
    rig.rt.set_viewport((1000.0, 760.0), 1.0);
    rig.settle();
    rig.settle();
    assert_eq!(width(&rig), 192.0);
}
