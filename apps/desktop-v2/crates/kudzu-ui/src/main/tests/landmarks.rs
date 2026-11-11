//! O1 (S41 review): the chrome's landmarks, in both themes, until S50's `library` scene
//! compares them against the reference. Rects are checked within 2 pt.

use kudzu_app::library::SnippetDraft;
use kudzu_app::settings::Theme as Stored;

use super::{cmd, cmd_backspace, Rig};
use crate::main::state::{LibraryFilter, MainView};
use crate::theme::{radius, Theme};

const TOLERANCE: f32 = 2.0;

fn near(rig: &Rig, key: &str, want: [f32; 4]) {
    let got = rig
        .rect(key)
        .unwrap_or_else(|| panic!("`{key}` is not on screen"));
    let close = got
        .iter()
        .zip(want)
        .all(|(g, w)| (g - w).abs() <= TOLERANCE);
    assert!(close, "`{key}` is at {got:?}, expected {want:?}");
}

fn element(rig: &Rig, key: &str) -> rune::ElementInfo {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("`{key}` is not on screen"))
}

fn both_themes() -> [(Stored, &'static Theme); 2] {
    [
        (Stored::Light, Theme::light()),
        (Stored::Dark, Theme::dark()),
    ]
}

/// The toolbar, its inset, the sidebar open and collapsed, Search and the card.
#[test]
fn the_frame_landmarks_hold_in_both_themes() {
    for (stored, theme) in both_themes() {
        let mut rig = Rig::themed(true, stored);
        near(&rig, "main-toolbar", [0.0, 0.0, 1180.0, 42.0]);
        near(&rig, "toolbar-sidebar-toggle", [12.0, 5.0, 32.0, 32.0]);
        near(
            &rig,
            "toolbar-search-trigger",
            [1180.0 - 16.0 - 192.0, 5.0, 192.0, 32.0],
        );
        near(&rig, "main-sidebar", [0.0, 42.0, 224.0, 718.0]);

        // M5: `<main>` is `bg-background shadow-card`, radius 18 (`--radius-2xl`).
        near(&rig, "main-content", [224.0, 50.0, 948.0, 702.0]);
        let card = element(&rig, "main-content");
        assert_eq!(card.fill, theme.c.background, "{stored:?} card fill");
        assert_eq!(card.radius, radius::XL2);
        let pose = rig.rt.presented_pose("main-content").unwrap();
        assert_eq!(pose.shadows(), theme.depth.card.as_slice());

        // Traffic lights: the toolbar's leading pad becomes 88 pt.
        rig.rt.set_titlebar_inset(28.0);
        rig.settle();
        near(&rig, "toolbar-sidebar-toggle", [88.0, 5.0, 32.0, 32.0]);

        // Collapsed: width 0, inert and out of the accessibility tree; the card takes the gutter.
        rig.press(cmd('\\'));
        near(&rig, "main-sidebar", [0.0, 42.0, 0.0, 718.0]);
        assert!(element(&rig, "main-sidebar").inert);
        near(&rig, "main-content", [8.0, 50.0, 1164.0, 702.0]);
        let a11y = rig.rt.accessibility_json().to_string();
        assert!(!a11y.contains("\"name\":\"Sidebar\""), "{stored:?}");
    }
}

/// Five crumbs fold the middle two behind "Show parent groups", inside the toolbar band.
#[test]
fn the_breadcrumb_overflow_holds_in_both_themes() {
    for (stored, _) in both_themes() {
        let mut rig = Rig::themed(true, stored);
        let lib = &rig.kudzu.library;
        let (a, _) = lib.create_group("Work".into(), None).unwrap();
        let (b, _) = lib.create_group("Engineering".into(), Some(a.id)).unwrap();
        let (c, _) = lib.create_group("Rust".into(), Some(b.id)).unwrap();
        let (snip, _) = lib
            .create_snippet(SnippetDraft {
                name: "Greeting".into(),
                group_id: Some(c.id),
                ..SnippetDraft::default()
            })
            .unwrap();
        rig.rt.update(move |st, _| {
            st.refresh_library_snapshot();
            st.navigate_to(MainView::Library(LibraryFilter::Group(c.id)));
            st.select_snippet(Some(snip.id));
        });
        rig.settle();
        let parents = element(&rig, "parents");
        assert_eq!(parents.label.as_deref(), Some("Show parent groups"));
        let r = parents.rect;
        assert!(r[1] >= 0.0 && r[1] + r[3] <= 42.0, "{stored:?} {r:?}");
        let crumbs = rig.rect("breadcrumb-trail").unwrap();
        let search = rig.rect("toolbar-search-trigger").unwrap();
        assert!(crumbs[0] + crumbs[2] <= search[0], "{crumbs:?} {search:?}");
    }
}

/// The Undo toast: bottom centre, 16 pt up, Sonner's 356 pt width.
#[test]
fn the_undo_toast_sits_bottom_centre_in_both_themes() {
    for (stored, _) in both_themes() {
        let mut rig = Rig::themed(true, stored);
        let first = rig.library().snippets[0].id;
        rig.rt.update(move |st, _| st.select_snippet(Some(first)));
        rig.settle();
        rig.press(cmd_backspace());
        let region = rig.rect("toast-region").expect("the toast shows");
        assert!((region[2] - 356.0).abs() <= TOLERANCE, "{region:?}");
        assert!(
            (region[0] - (1180.0 - 356.0) / 2.0).abs() <= TOLERANCE,
            "{region:?}"
        );
        assert!((region[1] + region[3] - (760.0 - 16.0)).abs() <= TOLERANCE);
    }
}

/// Onboarding replaces the whole window: no toolbar, sidebar, card or vibrancy.
#[test]
fn onboarding_is_chromeless_in_both_themes() {
    for (stored, theme) in both_themes() {
        let rig = Rig::themed(false, stored);
        for key in ["main-toolbar", "main-sidebar", "main-content"] {
            assert_eq!(rig.rect(key), None, "{stored:?} shows {key}");
        }
        assert_eq!(rig.rt.state.frame.sidebar_material_rect, None);
        near(&rig, "onboarding-view-slot", [0.0, 0.0, 1180.0, 760.0]);
        assert_eq!(
            element(&rig, "onboarding-view-slot").fill,
            theme.c.background
        );
    }
}
