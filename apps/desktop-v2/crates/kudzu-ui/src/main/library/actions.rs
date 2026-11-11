//! Snippet actions of the library (`LIB-14`, `LIB-7`): new, duplicate, favorite, on/off,
//! delete with its Undo toast, and the list's focus moves. S50 owns this file with the list.

use std::cell::RefCell;
use std::rc::Rc;

use kudzu_app::library::SnippetDraft;
use rune::Tx;
use uuid::Uuid;

use crate::main::state::{LibraryFilter, MainPane, MainView};
use crate::main::toasts::{ToastAction, ToastSpec, UNDO_TOAST_SECS};
use crate::main::{library, nav};
use crate::shell::MainState;

/// `⌘N`: a new snippet in the current group, selected, with the editor focused.
pub fn new_snippet(s: &mut MainState, tx: &mut Tx) {
    let group_id = match s.frame.view {
        MainView::Library(LibraryFilter::Group(gid)) => Some(gid),
        _ => None,
    };
    if !s.frame.view.in_library() {
        s.navigate_to(MainView::Library(LibraryFilter::All));
    }
    let draft = SnippetDraft {
        group_id,
        ..SnippetDraft::default()
    };
    if let Ok((created, _)) = s.svc.library.create_snippet(draft) {
        s.refresh_library_snapshot();
        s.select_snippet(Some(created.id));
    }
    if s.frame.selected_snippet.is_some() {
        s.blur_all_inputs();
        s.frame.focus.move_to(MainPane::Editor);
        s.editor.focus_content(tx);
    }
}

/// `⌘D`.
pub fn duplicate_selected(s: &mut MainState) {
    let Some(id) = s.frame.selected_snippet else {
        return;
    };
    if let Ok((copy, _)) = s.svc.library.duplicate_snippet(id) {
        s.refresh_library_snapshot();
        s.select_snippet(Some(copy.id));
    }
}

/// `⌘⇧F`.
pub fn toggle_favorite(s: &mut MainState) {
    update_selected(s, |sn| sn.is_favorite = !sn.is_favorite);
}

/// `⌘⇧E`.
pub fn toggle_enabled(s: &mut MainState) {
    update_selected(s, |sn| sn.is_enabled = !sn.is_enabled);
}

fn update_selected(s: &mut MainState, edit: impl FnOnce(&mut kudzu_core::SnippetRecord)) {
    let Some(id) = s.frame.selected_snippet else {
        return;
    };
    let Some(mut sn) = s
        .model
        .library
        .snippets
        .iter()
        .find(|sn| sn.id == id)
        .cloned()
    else {
        return;
    };
    edit(&mut sn);
    if s.svc.library.update_snippet(sn).is_ok() {
        s.refresh_library_snapshot();
    }
}

/// `⌘⌫`.
pub fn delete_selected(s: &mut MainState) {
    if let Some(id) = s.frame.selected_snippet {
        delete_with_undo(s, &[id]);
    }
}

/// Deletes `ids` and posts the 6-second Undo toast (`LIB-7`). Deleting the open snippet clears
/// the selection and moves pane focus back to the list.
pub fn delete_with_undo(s: &mut MainState, ids: &[Uuid]) -> bool {
    if ids.is_empty() {
        return false;
    }
    let Ok((deleted, _)) = s.svc.library.delete_snippets(ids) else {
        return false;
    };
    if deleted.snippets.is_empty() {
        return false;
    }
    let count = deleted.snippets.len();
    let single_name = (count == 1).then(|| {
        let n = deleted.snippets[0].name.trim();
        if n.is_empty() {
            "Untitled".to_owned()
        } else {
            n.to_owned()
        }
    });

    let deleted_open = s
        .frame
        .selected_snippet
        .is_some_and(|sel| ids.contains(&sel));
    s.refresh_library_snapshot();
    if deleted_open {
        s.frame.selected_snippet = None;
        s.blur_all_inputs();
        s.frame.focus.move_to(MainPane::List);
    }
    s.observe_breadcrumbs();

    let title = match single_name {
        Some(name) => format!("Deleted “{name}”"),
        None => format!("Deleted {count} snippets"),
    };
    let slot = Rc::new(RefCell::new(Some(deleted)));
    let now = s.frame.clock_secs;
    let spec = ToastSpec::new(title)
        .duration_secs(UNDO_TOAST_SECS)
        .action(ToastAction::new("Undo", true, move |st: &mut MainState| {
            if let Some(batch) = slot.borrow_mut().take() {
                if let Ok((restored, _)) = st.svc.library.restore(batch) {
                    st.refresh_library_snapshot();
                    if let Some(&first) = restored.first() {
                        st.select_snippet(Some(first));
                    }
                }
            }
        }));
    let key = format!("undo-delete:{}", ids[0]);
    s.frame.toasts.show(key, spec, now);
    true
}

/// `⌘F` in the library: leaves other screens for All, then focuses the filter.
pub fn focus_filter(s: &mut MainState, tx: &mut Tx) {
    if !s.frame.view.in_library() {
        s.navigate_to(MainView::Library(LibraryFilter::All));
    }
    s.blur_all_inputs();
    s.frame.focus.move_to(MainPane::List);
    s.library.focus_filter(tx);
}

/// `⌘L`, and Escape from the editor.
pub fn focus_list(s: &mut MainState, tx: &mut Tx) {
    nav::focus_list_pane(s, library::LIST_KEY, tx);
}
