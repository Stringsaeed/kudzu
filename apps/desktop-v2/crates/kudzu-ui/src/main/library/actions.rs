//! Snippet actions of the library (`LIB-14`, `LIB-7`): new, duplicate, favorite, on/off,
//! delete with its Undo toast, and the list's focus moves. S50 owns this file with the list.

use std::cell::RefCell;
use std::rc::Rc;

use kudzu_app::library::{Deleted, SnippetDraft};
use rune::{TextEdit, Tx};
use uuid::Uuid;

use crate::main::state::{LibraryFilter, MainPane, MainView};
use crate::main::toasts::{ToastAction, ToastSpec, UNDO_TOAST_SECS};
use crate::main::{library, nav};
use crate::shell::{MainState, NewSnippet};

/// `store.ts` `newSnippet`, for ⌘N and for `Route::NewSnippet` (Quick Search ⌘N, "New Snippet
/// from Selection"). Creates the snippet in the current group, a favorite on Favorites, with
/// `seed`'s name, content and trigger (given the default prefix when it lacks it). Then it
/// clears the filter, leaves Recent, Off, Insights, Settings and Clipboard for All, selects the
/// snippet and focuses its name. `None` when the library refused it.
pub fn new_snippet(s: &mut MainState, seed: NewSnippet) -> Option<Uuid> {
    let prefix = s.svc.settings.watch().get().default_trigger_prefix.clone();
    let triggers = match seed.trigger {
        t if t.is_empty() => Vec::new(),
        t if t.starts_with(&prefix) => vec![t],
        t => vec![format!("{prefix}{t}")],
    };
    open_new(s, seed.name, seed.content, triggers)
}

/// "Create it" under a filter that matches nothing: the trimmed filter becomes the trigger when
/// it starts with `;`, else the name (`snippet-list.tsx`, which tests `;` itself, not the
/// default prefix).
pub fn create_from_filter(s: &mut MainState) -> Option<Uuid> {
    let typed = s.library.filter.text.trim().to_owned();
    if typed.starts_with(';') {
        open_new(s, String::new(), String::new(), vec![typed])
    } else {
        open_new(s, typed, String::new(), Vec::new())
    }
}

/// `store.ts` `newSnippet` once the triggers are decided.
fn open_new(
    s: &mut MainState,
    name: String,
    content: String,
    triggers: Vec<String>,
) -> Option<Uuid> {
    let view = s.frame.view.clone();
    let draft = SnippetDraft {
        name,
        content,
        triggers,
        group_id: match view {
            MainView::Library(LibraryFilter::Group(gid)) => Some(gid),
            _ => None,
        },
        is_favorite: view == MainView::Library(LibraryFilter::Favorites),
        ..SnippetDraft::default()
    };
    let created = match s.svc.library.create_snippet(draft) {
        Ok((created, _)) => created,
        Err(error) => {
            log::warn!("a new snippet could not be created: {error}");
            return None;
        }
    };
    let leaves = matches!(
        view,
        MainView::Insights
            | MainView::Settings(_)
            | MainView::Clipboard(_)
            | MainView::Library(LibraryFilter::Recent | LibraryFilter::Disabled)
    );
    if leaves {
        s.navigate_to(MainView::Library(LibraryFilter::All));
    }
    s.library.filter = TextEdit::default();
    s.refresh_library_snapshot();
    s.select_snippet(Some(created.id));
    s.blur_all_inputs();
    s.frame.focus.move_to(MainPane::Editor);
    s.editor.request_name_focus(created.id);
    Some(created.id)
}

/// The filter's clear button and its first Escape.
pub fn clear_filter(s: &mut MainState) {
    s.library.filter.replace_text("");
    library::sync(s);
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

/// `⌘⌫`: the list's selection, the multi-selection when it holds more than one row
/// (`listRef.current.selection()`), else the open snippet.
pub fn delete_selected(s: &mut MainState, tx: &mut Tx) {
    let targets = s.library.targets(s.frame.selected_snippet);
    delete_with_undo(s, &targets, tx);
}

/// `main-app.tsx` `requestDelete`: deletes `ids`, selects the open snippet's neighbour when it
/// was among them (`store.ts` `deleteSnippets`), focuses the list, and posts the 6-second Undo
/// toast (`LIB-7`).
pub fn delete_with_undo(s: &mut MainState, ids: &[Uuid], tx: &mut Tx) -> bool {
    if ids.is_empty() {
        return false;
    }
    let neighbour = neighbour_after_delete(s, ids);
    let Ok((deleted, _)) = s.svc.library.delete_snippets(ids) else {
        return false;
    };
    if deleted.snippets.is_empty() {
        return false;
    }
    s.library.selection.retain(|id| !ids.contains(&id));
    s.refresh_library_snapshot();
    if let Some(next) = neighbour {
        s.select_snippet(next);
    }
    s.observe_breadcrumbs();
    focus_list(s, tx);

    let count = deleted.snippets.len();
    let title = if count == 1 {
        match deleted.snippets[0].name.trim() {
            "" => "Deleted “Untitled”".to_owned(),
            name => format!("Deleted “{name}”"),
        }
    } else {
        format!("Deleted {count} snippets")
    };
    let first = deleted.snippets[0].id;
    let slot = Rc::new(RefCell::new(Some(deleted)));
    let spec = ToastSpec::new(title)
        .duration_secs(UNDO_TOAST_SECS)
        .action(ToastAction::fallible(
            "Undo",
            true,
            move |st: &mut MainState, _| undo(st, &slot, first),
        ));
    let now = s.frame.clock_secs;
    s.frame
        .toasts
        .show(format!("undo-delete:{}", ids[0]), spec, now);
    true
}

/// `Some(next)` when the open snippet is among `ids`: the snippet now at its place in library
/// order, or the one before it at the end (`None` inside when nothing is left).
fn neighbour_after_delete(s: &MainState, ids: &[Uuid]) -> Option<Option<Uuid>> {
    let open = s.frame.selected_snippet.filter(|id| ids.contains(id))?;
    let lib = s.svc.library.watch().get();
    let at = lib.snippets.iter().position(|sn| sn.id == open);
    let remaining: Vec<Uuid> = lib
        .snippets
        .iter()
        .map(|sn| sn.id)
        .filter(|id| !ids.contains(id))
        .collect();
    Some(at.and_then(|at| {
        remaining
            .get(at.min(remaining.len().checked_sub(1)?))
            .copied()
    }))
}

/// `store.ts` `restoreSnippets`: restores the batch and selects the first deleted snippet,
/// also when sync already put it back. The batch is taken only once the restore is accepted;
/// a failed one keeps the batch and its toast, so Undo can run again. True closes the toast.
fn undo(st: &mut MainState, slot: &RefCell<Option<Deleted>>, first: Uuid) -> bool {
    let Some(batch) = slot.borrow().clone() else {
        return true;
    };
    if let Err(error) = st.svc.library.restore(batch) {
        log::warn!("the deleted snippets could not be restored: {error}");
        return false;
    }
    slot.borrow_mut().take();
    st.refresh_library_snapshot();
    if st.model.library.snippets.iter().any(|sn| sn.id == first) {
        st.select_snippet(Some(first));
    }
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

/// `⌘L`, Escape from the editor, and after a delete.
pub fn focus_list(s: &mut MainState, tx: &mut Tx) {
    nav::focus_list_pane(s, library::LIST_KEY, tx);
}
