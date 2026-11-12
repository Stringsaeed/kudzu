//! The multi-selection's actions: ⌘⌫, the bulk bar, the empty states and the drag source
//! (LIB-5, LIB-12, LIB-11's source half).

use rune::Mods;

use super::library_list::{
    bar_count, click_row, cursor_name, focus_list, group, id, row_names, sample_rig,
};
use super::{cmd, cmd_backspace, plain, Rig};
use crate::dnd::DragPayload;
use crate::main::library;
use crate::main::state::{LibraryFilter, MainView};

fn toggle() -> Mods {
    Mods {
        cmd: true,
        ..Mods::default()
    }
}

fn select_three(rig: &mut Rig) {
    click_row(rig, "Bug report", Mods::default());
    click_row(rig, "Calendar link", toggle());
    click_row(rig, "Console log", toggle());
    assert_eq!(rig.rt.state.library.selection.bulk().len(), 3);
}

fn snippet(rig: &Rig, name: &str) -> kudzu_core::SnippetRecord {
    rig.library()
        .snippets
        .iter()
        .find(|s| s.name == name)
        .unwrap()
        .clone()
}

fn present(rig: &Rig, name: &str) -> bool {
    rig.library().snippets.iter().any(|s| s.name == name)
}

fn a11y(rig: &Rig) -> String {
    rig.rt.accessibility_json().to_string()
}

/// S41 follow-up: ⌘⌫ deletes the list's multi-selection, and the open snippet's neighbour in
/// library order opens (`store.ts` `deleteSnippets`).
#[test]
fn command_backspace_deletes_the_multi_selection() {
    let mut rig = sample_rig();
    click_row(&mut rig, "Bug report", Mods::default());
    click_row(&mut rig, "Calendar link", toggle());
    focus_list(&mut rig);
    rig.press(cmd_backspace());
    assert!(!present(&rig, "Bug report") && !present(&rig, "Calendar link"));
    assert!(a11y(&rig).contains("Deleted 2 snippets"));
    assert_eq!(cursor_name(&rig), "Quote the selection");
    assert!(rig.rt.state.library.selection.bulk().is_empty());
    assert_eq!(rig.focused_key().as_deref(), Some(library::LIST_KEY));
    rig.click_text("Undo");
    assert!(present(&rig, "Bug report") && present(&rig, "Calendar link"));
    assert_eq!(cursor_name(&rig), "Bug report");
}

#[test]
fn the_bulk_bar_favorites_toggles_moves_and_deletes() {
    let mut rig = sample_rig();
    select_three(&mut rig);
    assert_eq!(bar_count(&rig).as_deref(), Some("3"));

    rig.click_key(library::bulk::FAVORITE_KEY);
    for name in ["Bug report", "Calendar link", "Console log"] {
        assert!(snippet(&rig, name).is_favorite, "{name}");
    }

    // "Turn off all" toggles each snippet's own flag.
    let mut console = snippet(&rig, "Console log");
    console.is_enabled = false;
    rig.kudzu.library.update_snippet(console).unwrap();
    rig.settle();
    rig.click_key(library::bulk::TOGGLE_KEY);
    assert!(!snippet(&rig, "Bug report").is_enabled);
    assert!(!snippet(&rig, "Calendar link").is_enabled);
    assert!(snippet(&rig, "Console log").is_enabled);

    rig.click_key(library::bulk::MOVE_KEY);
    assert!(a11y(&rig).contains("Move 3 to"));
    rig.click_text("Personal");
    let personal = group(&rig, "Personal");
    for name in ["Bug report", "Calendar link", "Console log"] {
        assert_eq!(snippet(&rig, name).group_id, Some(personal), "{name}");
    }
    rig.click_key(library::bulk::MOVE_KEY);
    rig.click_text("No group");
    assert_eq!(snippet(&rig, "Bug report").group_id, None);

    rig.click_key(library::bulk::DELETE_KEY);
    assert!(!present(&rig, "Bug report") && !present(&rig, "Console log"));
    assert!(a11y(&rig).contains("Deleted 3 snippets"));
    assert!(rig.rt.state.library.selection.bulk().is_empty());
}

/// The bar keeps its last selection while it sinks away, and is inert meanwhile.
#[test]
fn the_bulk_bar_is_inert_while_it_exits() {
    let mut rig = sample_rig();
    select_three(&mut rig);
    // A plain click, without settling: the bar's exit is still playing on the next frame.
    let r = rig
        .rect(&library::row_key(id(&rig, "Current time")))
        .unwrap();
    let (x, y) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
    rig.rt.frame(0.016);
    rig.rt.frame(0.016);
    let bar = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(library::bulk::BAR_KEY))
        .expect("the bar is still drawn while it leaves");
    assert!(bar.exiting && bar.inert);
    let count = rig.rt.elements().into_iter().any(|e| {
        e.exiting
            && e.key.as_deref() == Some(library::bulk::COUNT_KEY)
            && e.text.as_deref() == Some("3")
    });
    assert!(count, "the leaving bar keeps its count");
    rig.rt.frame(1.0);
    rig.rt.frame(0.0);
    assert!(rig.rect(library::bulk::BAR_KEY).is_none());
}

fn empty_copy(rig: &mut Rig, view: MainView) -> String {
    rig.rt.update(move |st, _| st.navigate_to(view));
    rig.settle();
    a11y(rig)
}

#[test]
fn every_empty_state_has_its_copy_and_actions() {
    let mut rig = sample_rig();
    let developer = group(&rig, "Developer");
    rig.kudzu
        .library
        .transact(|lib| {
            lib.snippets.clear();
            lib.usage.clear();
            Ok(())
        })
        .unwrap();
    rig.settle();
    let all = empty_copy(&mut rig, MainView::Library(LibraryFilter::All));
    assert!(all.contains("Your library is empty"));
    assert!(all.contains("Snippets turn a short trigger like ;sig into text you type often."));
    let fav = empty_copy(&mut rig, MainView::Library(LibraryFilter::Favorites));
    assert!(fav.contains("No favorites yet"));
    let recent = empty_copy(&mut rig, MainView::Library(LibraryFilter::Recent));
    assert!(recent.contains("Nothing used yet"));
    assert!(rig.rect(library::empty::NEW_KEY).is_none());
    let off = empty_copy(&mut rig, MainView::Library(LibraryFilter::Disabled));
    assert!(off.contains("Everything is on"));
    assert!(rig.rect(library::empty::NEW_KEY).is_none());
    let grp = empty_copy(&mut rig, MainView::Library(LibraryFilter::Group(developer)));
    assert!(grp.contains("This group is empty"));
    assert!(grp.contains("Create a snippet here or drag snippets onto the group."));
    // "New snippet" files it in the group.
    rig.click_key(library::empty::NEW_KEY);
    assert_eq!(rig.library().snippets[0].group_id, Some(developer));
}

#[test]
fn a_filter_with_no_match_offers_clear_and_create() {
    let mut rig = sample_rig();
    rig.press(cmd('f'));
    for ch in "zzq".chars() {
        rig.press(plain(ch, false));
    }
    assert!(a11y(&rig).contains("No snippets match “zzq”"));
    rig.click_key(library::empty::CLEAR_KEY);
    assert_eq!(rig.rt.state.library.filter.text, "");
    assert_eq!(row_names(&rig).len(), 18);

    // A filter that starts with ";" becomes the trigger, anything else the name.
    rig.press(cmd('f'));
    for ch in ";zz".chars() {
        rig.press(plain(ch, false));
    }
    rig.click_key(library::empty::CREATE_KEY);
    let made = rig.library().snippets[0].clone();
    assert_eq!(
        (made.name.as_str(), made.triggers.clone()),
        ("", vec![";zz".to_owned()])
    );
    assert_eq!(rig.rt.state.library.filter.text, "");
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(made.id));

    rig.press(cmd('f'));
    for ch in "Hello".chars() {
        rig.press(plain(ch, false));
    }
    rig.click_key(library::empty::CREATE_KEY);
    assert_eq!(rig.library().snippets[0].name, "Hello");
}

/// The drag source for S42: a row in the multi-selection carries the selection, any other row
/// itself; a press that only wobbles is a click.
#[test]
fn rows_start_drags_with_the_selection() {
    let mut rig = sample_rig();
    select_three(&mut rig);
    let r = rig
        .rect(&library::row_key(id(&rig, "Calendar link")))
        .unwrap();
    let (x, y) = (r[0] + 40.0, r[1] + 20.0);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.frame(0.016);
    rig.rt.pointer_move(x + 2.0, y + 1.0);
    rig.rt.frame(0.016);
    assert_eq!(
        library::drag::payload(&rig.rt.state),
        None,
        "under the threshold"
    );
    rig.rt.pointer_move(x + 30.0, y + 12.0);
    rig.rt.frame(0.016);
    let selected = vec![
        id(&rig, "Bug report"),
        id(&rig, "Calendar link"),
        id(&rig, "Console log"),
    ];
    assert_eq!(
        library::drag::payload(&rig.rt.state),
        Some(&DragPayload::Snippets(selected))
    );
    rig.rt.pointer_up(x + 30.0, y + 12.0);
    rig.settle();
    assert_eq!(library::drag::payload(&rig.rt.state), None);
    // Rune #551: the release after a drag is no click, so the selection stays.
    assert_eq!(rig.rt.state.library.selection.bulk().len(), 3);

    let r = rig
        .rect(&library::row_key(id(&rig, "Current time")))
        .unwrap();
    let (x, y) = (r[0] + 40.0, r[1] + 20.0);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.frame(0.016);
    rig.rt.pointer_move(x + 40.0, y);
    rig.rt.frame(0.016);
    assert_eq!(
        library::drag::payload(&rig.rt.state),
        Some(&DragPayload::Snippets(vec![id(&rig, "Current time")]))
    );
    rig.rt.pointer_up(x + 40.0, y);
    rig.settle();
}
