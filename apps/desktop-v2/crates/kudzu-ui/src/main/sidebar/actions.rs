//! Sidebar actions (`LIB-14`): new group and the status footer's pause. S42 owns this file with
//! the sidebar, and adds subgroups, rename and the context menu here.

use crate::main::state::{LibraryFilter, MainView};
use crate::shell::MainState;

/// `⌘⇧N`: `createGroup("New group", null)` (`main-app.tsx:112`), a top-level group with the
/// same name every time, then that group's view. Subgroups are the sidebar's own action (S42).
pub fn new_group(s: &mut MainState) {
    match s.svc.library.create_group("New group".to_owned(), None) {
        Ok((group, _)) => {
            s.refresh_library_snapshot();
            s.navigate_to(MainView::Library(LibraryFilter::Group(group.id)));
        }
        Err(error) => log::warn!("a new group could not be created: {error}"),
    }
}

/// `⌘⇧P`, and the footer's pause button.
pub fn toggle_pause(s: &mut MainState) {
    let paused = !s.svc.expansion.status().get().paused;
    let _ = s.svc.expansion.set_paused(paused);
}
