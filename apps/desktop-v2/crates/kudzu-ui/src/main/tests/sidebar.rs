use super::Rig;
use crate::main::{ClipFilter, LibraryFilter, MainView};

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
    rig.rt.frame(0.0);
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|element| element.role == "button" && element.label.as_deref() == Some("Images")));
}
