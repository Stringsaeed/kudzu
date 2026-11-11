//! Sidebar actions (`LIB-14`): new group and the status footer's pause. S42 owns this file with
//! the sidebar, and adds subgroups, rename and the context menu here.

use crate::main::state::{LibraryFilter, MainView};
use crate::shell::MainState;

/// `⌘⇧N`.
pub fn new_group(s: &mut MainState) {
    let parent_id = match s.frame.view {
        MainView::Library(LibraryFilter::Group(gid)) => Some(gid),
        _ => None,
    };
    let existing = s.model.library.groups.len();
    let name = if existing == 0 {
        "New group".to_owned()
    } else {
        format!("New group {}", existing + 1)
    };
    if let Ok((group, _)) = s.svc.library.create_group(name, parent_id) {
        s.refresh_library_snapshot();
        s.navigate_to(MainView::Library(LibraryFilter::Group(group.id)));
    }
}

/// `⌘⇧P`, and the footer's pause button.
pub fn toggle_pause(s: &mut MainState) {
    let paused = !s.svc.expansion.status().get().paused;
    let _ = s.svc.expansion.set_paused(paused);
}
