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
        Err(error) => show_error(s, error),
    }
}

/// `⌘⇧P`, and the footer's pause button.
pub fn toggle_pause(s: &mut MainState) {
    if let Err(error) = s.svc.expansion.toggle_paused() {
        show_error(s, error);
    }
}

pub fn begin_rename(s: &mut MainState, id: uuid::Uuid) {
    if s.is_composing() {
        return;
    }
    if let Some(group) = s
        .library_snapshot()
        .groups
        .iter()
        .find(|group| group.id == id)
    {
        let mut edit = rune::TextEdit::new(&group.name);
        edit.anchor = 0;
        edit.focused = true;
        s.sidebar.rename = Some(id);
        s.sidebar.edit = edit;
        s.sidebar.menu = None;
    }
}

pub fn commit_rename(s: &mut MainState) {
    let Some(id) = s.sidebar.rename else { return };
    let name = s.sidebar.edit.text.trim().to_owned();
    if !name.is_empty() {
        let Some(mut group) = s
            .library_snapshot()
            .groups
            .iter()
            .find(|group| group.id == id)
            .cloned()
        else {
            show_error(s, kudzu_app::library::LibraryError::MissingGroup);
            return;
        };
        group.name = name;
        if let Err(error) = s.svc.library.update_group(group) {
            show_error(s, error);
            return;
        }
        s.refresh_library_snapshot();
    }
    s.sidebar.rename = None;
    s.sidebar.edit.focused = false;
}

pub fn subgroup(s: &mut MainState, id: uuid::Uuid) {
    s.sidebar.menu = None;
    match s.svc.library.create_group("New group".into(), Some(id)) {
        Ok((group, _)) => {
            s.sidebar.collapsed.remove(&id);
            s.refresh_library_snapshot();
            s.navigate_to(MainView::Library(LibraryFilter::Group(group.id)));
            begin_rename(s, group.id);
        }
        Err(error) => show_error(s, error),
    }
}

pub fn toggle_group(s: &mut MainState, id: uuid::Uuid) {
    if let Some(mut group) = s
        .library_snapshot()
        .groups
        .iter()
        .find(|g| g.id == id)
        .cloned()
    {
        group.is_enabled = !group.is_enabled;
        match s.svc.library.update_group(group) {
            Ok(_) => s.refresh_library_snapshot(),
            Err(error) => show_error(s, error),
        }
    }
    s.sidebar.menu = None;
}

pub fn unparent(s: &mut MainState, id: uuid::Uuid) {
    match s.svc.library.move_group(id, None, i64::MAX) {
        Ok(_) => s.refresh_library_snapshot(),
        Err(error) => show_error(s, error),
    }
    s.sidebar.menu = None;
}

pub fn delete(s: &mut MainState, id: uuid::Uuid, snippets: bool) {
    use kudzu_app::library::GroupDeletion;
    let mode = if snippets {
        GroupDeletion::DeleteSnippets
    } else {
        GroupDeletion::KeepSnippets
    };
    let removed = kudzu_core::library::groups::descendant_ids(&s.library_snapshot().groups, id);
    match s.svc.library.delete_group(id, mode) {
        Ok(_) => {
            s.refresh_library_snapshot();
            if matches!(s.frame.view, MainView::Library(LibraryFilter::Group(selected)) if removed.contains(&selected))
            {
                s.navigate_to(MainView::Library(LibraryFilter::All));
            }
        }
        Err(error) => show_error(s, error),
    }
    s.sidebar.menu = None;
}

pub fn new_group_from_button(s: &mut MainState) {
    match s.svc.library.create_group("New group".to_owned(), None) {
        Ok((group, _)) => {
            s.refresh_library_snapshot();
            s.navigate_to(MainView::Library(LibraryFilter::Group(group.id)));
            begin_rename(s, group.id);
        }
        Err(error) => show_error(s, error),
    }
}

pub(crate) fn show_error(s: &mut MainState, error: impl std::fmt::Display) {
    let toast = crate::main::ToastSpec::new("Couldn't update the library")
        .description(error.to_string())
        .persistent()
        .closable(true);
    s.frame
        .toasts
        .show("sidebar-error", toast, s.frame.clock_secs);
}
