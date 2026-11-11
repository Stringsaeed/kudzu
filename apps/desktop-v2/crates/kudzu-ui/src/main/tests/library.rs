//! Snippet actions: new, delete and the Undo toast.

use kudzu_app::library::SnippetDraft;
use kudzu_core::SnippetRecord;

use super::{cmd, cmd_backspace, cmd_shift, Rig};
use crate::actions::main::MainAction;
use crate::actions::Action;
use crate::main::state::{ClipFilter, LibraryFilter, MainPane, MainView};
use crate::main::toasts::Toasts;
use crate::main::{editor, library};
use crate::shell::{MainState, SettingsPane};

fn create(rig: &Rig, name: &str) -> SnippetRecord {
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: name.into(),
            content: format!("{name} body"),
            ..SnippetDraft::default()
        })
        .unwrap();
    snippet
}

fn select(rig: &mut Rig, id: uuid::Uuid) {
    rig.rt.update(move |st, _| {
        st.refresh_library_snapshot();
        st.select_snippet(Some(id));
        st.frame.focus.move_to(MainPane::List);
    });
    rig.settle();
}

fn present(rig: &Rig, id: uuid::Uuid) -> bool {
    rig.library().snippets.iter().any(|s| s.id == id)
}

#[test]
fn undo_delete_toast_restores_snippet_and_expires_after_six_seconds() {
    let mut rig = Rig::new(true);
    let (snip, _) = rig
        .kudzu
        .library
        .create_snippet(SnippetDraft {
            name: "Signature".into(),
            content: "Best,\nSaeed".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    rig.rt.frame(0.1);
    rig.rt.update(|st, _| {
        st.select_snippet(Some(snip.id));
        st.frame.focus.move_to(MainPane::List);
    });
    rig.rt.frame(0.05);

    // `⌘⌫` from the list deletes it, selects its neighbour (M2: it used to clear the
    // selection) and posts the 6 s Undo toast.
    let neighbour = rig.library().snippets[1].id;
    rig.press(cmd_backspace());
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(neighbour));
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert!(!rig
        .kudzu
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.id == snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    assert!(rig.rt.wake_deadline().is_some());

    let a11y = rig.rt.accessibility_json().to_string();
    assert!(a11y.contains("Deleted “Signature”"));
    assert!(a11y.contains("Undo"));

    // Clicking the `"Undo"` action button restores the snippet and dismisses the toast.
    rig.click_text("Undo");
    assert!(rig
        .kudzu
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.id == snip.id));
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);

    // Move cursor off the bottom-center toast area, delete a second time, and verify the 6-second auto-dismiss timer (`UNDO_TOAST_SECS = 6.0`).
    rig.rt.pointer_move(10.0, 10.0);
    rig.rt.frame(0.0);
    rig.press(cmd_backspace());
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    rig.rt.frame(5.8);
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    rig.rt.frame(0.4);
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);
}

/// M2 (S41 review): deleting the open snippet selects its neighbour in library order, as
/// `store.ts` `deleteSnippets` does, and moves Rune focus to the list with a `Tx`, as
/// `FocusList` does. Focus used to stay where it was while the model said `List`.
#[test]
fn deleting_the_open_snippet_selects_its_neighbour_and_focuses_the_list() {
    let mut rig = Rig::new(true);
    let _a = create(&rig, "A");
    let b = create(&rig, "B");
    let _c = create(&rig, "C");
    select(&mut rig, b.id);
    let order: Vec<_> = rig.library().snippets.iter().map(|s| s.id).collect();
    let at = order.iter().position(|id| *id == b.id).unwrap();

    // Keyboard focus sits outside the list (the toolbar's search button).
    rig.rt
        .update(|_, tx| tx.focus("toolbar-search-trigger", true));
    rig.settle();
    assert_eq!(rig.focused_key().as_deref(), Some("toolbar-search-trigger"));

    rig.press(cmd_backspace());
    assert!(!present(&rig, b.id));
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(order[at + 1]));
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert_eq!(rig.focused_key().as_deref(), Some(library::LIST_KEY));

    // The last snippet's neighbour is the one before it.
    let lib = rig.library();
    let (last, before_last) = (
        lib.snippets[lib.snippets.len() - 1].id,
        lib.snippets[lib.snippets.len() - 2].id,
    );
    select(&mut rig, last);
    rig.press(cmd_backspace());
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(before_last));
}

/// M3 (S41 review): when sync has already put the id back, Undo still selects it and closes.
#[test]
fn undo_selects_the_first_id_even_when_sync_already_restored_it() {
    let mut rig = Rig::new(true);
    let snip = create(&rig, "Signature");
    select(&mut rig, snip.id);
    rig.press(cmd_backspace());
    assert!(!present(&rig, snip.id));

    let back = snip.clone();
    rig.kudzu
        .library
        .transact(move |lib| {
            lib.snippets.push(back);
            Ok(())
        })
        .unwrap();
    rig.settle();
    rig.click_text("Undo");
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);
}

/// M3: a failed restore keeps the batch, and the toast, so Undo can be tried again.
#[test]
fn a_failed_undo_keeps_the_batch_and_its_toast() {
    let mut rig = Rig::new(true);
    let snip = create(&rig, "Signature");
    select(&mut rig, snip.id);
    rig.press(cmd_backspace());
    let _ = rig.kudzu.shutdown(); // The writer stops, so `restore` returns `Err`.
    rig.click_text("Undo");
    assert!(!present(&rig, snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    assert!(rig.rt.accessibility_json().to_string().contains("Undo"));
}

/// Minor 7 (S41 review): the keyboard path to a toast's action dismisses it, as the button does.
#[test]
fn trigger_front_action_runs_the_action_and_dismisses_the_toast() {
    let mut rig = Rig::new(true);
    let snip = create(&rig, "Signature");
    select(&mut rig, snip.id);
    rig.press(cmd_backspace());
    assert_eq!(rig.rt.state.frame.toasts.len(), 1);
    let mut ran = None;
    rig.rt.update(|st, tx| {
        ran = Toasts::trigger_front_action(st, tx, |s: &mut MainState| {
            let now = s.frame.clock_secs;
            (&mut s.frame.toasts, now)
        });
    });
    rig.settle();
    assert!(ran.is_some());
    assert!(present(&rig, snip.id));
    assert_eq!(rig.rt.state.frame.toasts.len(), 0);
}

/// M4 (S41 review): ⌘N follows `store.ts` `newSnippet`: Recent, Disabled, Insights, Settings and
/// Clipboard switch to All; Favorites makes a favorite and stays; a group files it there.
#[test]
fn new_snippet_follows_the_view_like_the_tauri_store() {
    let mut rig = Rig::new(true);
    let leaves = [
        MainView::Library(LibraryFilter::Recent),
        MainView::Library(LibraryFilter::Disabled),
        MainView::Insights,
        MainView::Settings(SettingsPane::General),
        MainView::Clipboard(ClipFilter::All),
    ];
    for from in leaves {
        rig.rt.update(move |st, _| st.navigate_to(from));
        rig.settle();
        rig.press(cmd('n'));
        let created = rig.library().snippets[0].clone();
        assert_eq!(rig.rt.state.frame.selected_snippet, Some(created.id));
        assert_eq!(
            rig.rt.state.frame.view,
            MainView::Library(LibraryFilter::All)
        );
        assert!(!created.is_favorite && created.group_id.is_none());
        assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
    }

    rig.press(cmd('2'));
    rig.press(cmd('n'));
    let favorite = rig.library().snippets[0].clone();
    assert!(favorite.is_favorite);
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Favorites)
    );

    let (group, _) = rig.kudzu.library.create_group("Work".into(), None).unwrap();
    rig.rt
        .update(move |st, _| st.navigate_to(MainView::Library(LibraryFilter::Group(group.id))));
    rig.settle();
    rig.press(cmd('n'));
    let filed = rig.library().snippets[0].clone();
    assert_eq!((filed.group_id, filed.is_favorite), (Some(group.id), false));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Group(group.id))
    );
}

/// M4: ⌘⇧N is `createGroup("New group", null)`: top level and the same name every time, even
/// from inside a group. Subgroups are S42's sidebar action.
#[test]
fn new_group_is_a_top_level_group_named_new_group() {
    let mut rig = Rig::new(true);
    let (work, _) = rig.kudzu.library.create_group("Work".into(), None).unwrap();
    rig.rt
        .update(move |st, _| st.navigate_to(MainView::Library(LibraryFilter::Group(work.id))));
    rig.settle();
    for _ in 0..2 {
        let before = rig.library().groups.len();
        rig.press(cmd_shift('n'));
        let lib = rig.library();
        assert_eq!(lib.groups.len(), before + 1);
        let group = lib.groups.last().unwrap();
        assert_eq!((group.name.as_str(), group.parent_id), ("New group", None));
        assert_eq!(
            rig.rt.state.frame.view,
            MainView::Library(LibraryFilter::Group(group.id))
        );
    }
}

/// Minor 3 (S41 review): `Action::enabled` applies the text-field guard `resolve_in_context`
/// does, so a cheat sheet or menu built from it never offers ⌘⌫ inside a field.
#[test]
fn enabled_applies_the_text_field_guard() {
    let mut rig = Rig::new(true);
    let snip = create(&rig, "Signature");
    select(&mut rig, snip.id);
    assert!(MainAction::DeleteSnippet.enabled(&rig.rt.state));
    rig.press(cmd('f'));
    assert!(rig.rt.state.is_editing_text());
    assert!(!MainAction::DeleteSnippet.enabled(&rig.rt.state));
    assert!(MainAction::NewSnippet.enabled(&rig.rt.state));
}
