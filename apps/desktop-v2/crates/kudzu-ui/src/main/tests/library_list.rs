//! The library list on the S02 sample library (`lib/mock.ts`): what opens, rows, the list and
//! filter keys, clicks with modifiers, the sort menu and the breadcrumb trail (LIB-1 to LIB-4).

use rune::{Key, KeyEvent, Mods, PhysicalInput};
use uuid::Uuid;

use super::{cmd, esc, plain, Rig};
use crate::main::editor;
use crate::main::library::{self, scenes};
use crate::main::state::{LibraryFilter, MainPane, MainView};
use crate::main::toolbar::compute_crumbs;

pub(super) fn sample_rig() -> Rig {
    let mut rig = Rig::new(true);
    rig.rt.update(|st, _| scenes::seed_sample(st));
    rig.settle();
    rig
}

pub(super) fn id(rig: &Rig, name: &str) -> Uuid {
    rig.library()
        .snippets
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no snippet {name}"))
        .id
}

pub(super) fn group(rig: &Rig, name: &str) -> Uuid {
    rig.library()
        .groups
        .iter()
        .find(|g| g.name == name)
        .unwrap()
        .id
}

pub(super) fn cursor_name(rig: &Rig) -> String {
    let id = rig.rt.state.frame.selected_snippet.expect("a selection");
    rig.library()
        .snippets
        .iter()
        .find(|s| s.id == id)
        .unwrap()
        .name
        .clone()
}

/// The names of the rows the list built, top to bottom.
pub(super) fn row_names(rig: &Rig) -> Vec<String> {
    rig.rt
        .elements()
        .into_iter()
        .filter(|e| e.role == "option" && !e.exiting)
        .filter_map(|e| e.label)
        .collect()
}

pub(super) fn key(key: Key, shift: bool) -> KeyEvent {
    KeyEvent {
        key,
        mods: Mods {
            shift,
            ..Mods::default()
        },
    }
}

pub(super) fn hold(rig: &mut Rig, mods: Mods) {
    rig.rt.physical_input(PhysicalInput::Modifiers(mods));
}

/// A click on the row of `name` with `mods` held.
pub(super) fn click_row(rig: &mut Rig, name: &str, mods: Mods) {
    let row = library::row_key(id(rig, name));
    hold(rig, mods);
    rig.click_key(&row);
    hold(rig, Mods::default());
}

pub(super) fn focus_list(rig: &mut Rig) {
    rig.press(cmd('l'));
    assert_eq!(rig.focused_key().as_deref(), Some(library::LIST_KEY));
}

/// Keyboard focus is in the filter field (rune-kit focuses the editor inside its box).
pub(super) fn filter_focused(rig: &Rig) -> bool {
    let els = rig.rt.elements();
    let Some(field) = els
        .iter()
        .find(|e| e.key.as_deref() == Some(library::FILTER_KEY))
    else {
        return false;
    };
    els.iter().any(|e| {
        e.focused && (e.path == field.path || e.path.starts_with(&format!("{}/", field.path)))
    })
}

/// The bulk bar's count, while the bar is up.
pub(super) fn bar_count(rig: &Rig) -> Option<String> {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(library::bulk::COUNT_KEY) && !e.exiting)
        .and_then(|e| e.text)
}

fn filter_text(rig: &Rig) -> String {
    rig.rt.state.library.filter.text.clone()
}

/// `store.ts` `init` opens the first snippet in library order, not the first row.
#[test]
fn the_window_opens_on_the_first_snippet_in_library_order() {
    let rig = sample_rig();
    assert_eq!(cursor_name(&rig), "Email signature");
    let rows = row_names(&rig);
    assert_eq!(rows.first().map(String::as_str), Some("Bug report"));
    assert_eq!(rows[4], "Email signature");
    let heading = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.role == "heading" && e.label.as_deref() == Some("All snippets"))
        .expect("the list heading");
    assert_eq!(heading.rect[0], 244.0);
    assert!(rig
        .rt
        .elements()
        .iter()
        .any(|e| e.text.as_deref() == Some("18")));
}

#[test]
fn list_keys_move_with_an_anchor_and_select_everything() {
    let mut rig = sample_rig();
    focus_list(&mut rig);
    rig.press(key(Key::Down, false));
    assert_eq!(cursor_name(&rig), "Git commit co-author");
    rig.press(key(Key::Down, true));
    rig.press(key(Key::Down, true));
    assert_eq!(cursor_name(&rig), "Lorem ipsum");
    assert_eq!(rig.rt.state.library.selection.bulk().len(), 3);
    assert_eq!(bar_count(&rig).as_deref(), Some("3"));
    rig.press(esc());
    assert!(rig.rt.state.library.selection.bulk().is_empty());
    assert_eq!(cursor_name(&rig), "Lorem ipsum");
    rig.press(cmd('a'));
    assert_eq!(rig.rt.state.library.selection.bulk().len(), 18);
    rig.press(esc());
    rig.press(key(Key::Home, false));
    assert_eq!(cursor_name(&rig), "Bug report");
    rig.press(key(Key::End, false));
    assert_eq!(cursor_name(&rig), "Today");
    rig.press(key(Key::PageUp, false));
    assert_eq!(cursor_name(&rig), "Out of office");
    rig.press(plain('k', false));
    assert_eq!(cursor_name(&rig), "Meeting follow-up");
    rig.press(plain('j', false));
    assert_eq!(cursor_name(&rig), "Out of office");
    rig.press(key(Key::PageDown, false));
    assert_eq!(cursor_name(&rig), "Today");
    // The list keeps keyboard focus and the cursor row stays built.
    assert_eq!(rig.focused_key().as_deref(), Some(library::LIST_KEY));
    assert!(row_names(&rig).contains(&"Today".to_owned()));
}

#[test]
fn return_and_right_open_the_editor() {
    let mut rig = sample_rig();
    for open in [Key::Enter, Key::Right] {
        focus_list(&mut rig);
        rig.press(key(open, false));
        assert_eq!(rig.focused_key().as_deref(), Some(editor::CONTENT_KEY));
        assert_eq!(rig.rt.state.frame.focus.active(), MainPane::Editor);
    }
}

#[test]
fn slash_and_typing_in_the_list_focus_the_filter() {
    let mut rig = sample_rig();
    focus_list(&mut rig);
    rig.press(plain('/', false));
    assert!(filter_focused(&rig), "the filter has focus");
    assert_eq!(filter_text(&rig), "");
    focus_list(&mut rig);
    // A character lands in the filter; Shift+J is a capital J, not a move.
    rig.press(plain('J', true));
    assert!(filter_focused(&rig), "the filter has focus");
    assert_eq!(filter_text(&rig), "J");
    assert_eq!(cursor_name(&rig), "Email signature");
}

#[test]
fn filter_keys_move_open_and_clear() {
    let mut rig = sample_rig();
    rig.press(cmd('f'));
    for ch in ['c', 'o'] {
        rig.press(plain(ch, false));
    }
    assert_eq!(filter_text(&rig), "co");
    let rows = row_names(&rig);
    assert!(rows.len() < 18 && rows.contains(&"Console log".to_owned()));
    // The filter keeps the open snippet while it is outside the results.
    assert_eq!(cursor_name(&rig), "Email signature");
    rig.press(key(Key::Down, false));
    assert_eq!(cursor_name(&rig), rows[0]);
    rig.press(key(Key::Down, false));
    assert_eq!(cursor_name(&rig), rows[1]);
    assert!(filter_focused(&rig), "the filter has focus");
    rig.press(esc());
    assert_eq!(filter_text(&rig), "");
    assert!(filter_focused(&rig), "the filter has focus");
    rig.press(esc());
    assert_eq!(rig.focused_key().as_deref(), Some(library::LIST_KEY));
    rig.press(cmd('f'));
    rig.press(key(Key::Enter, false));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::CONTENT_KEY));
}

#[test]
fn clicks_select_toggle_and_range() {
    let mut rig = sample_rig();
    click_row(&mut rig, "Bug report", Mods::default());
    assert_eq!(cursor_name(&rig), "Bug report");
    let toggle = Mods {
        cmd: true,
        ..Mods::default()
    };
    click_row(&mut rig, "Console log", toggle);
    assert_eq!(cursor_name(&rig), "Console log");
    assert_eq!(
        rig.rt.state.library.selection.bulk(),
        [id(&rig, "Bug report"), id(&rig, "Console log")]
    );
    let range = Mods {
        shift: true,
        ..Mods::default()
    };
    click_row(&mut rig, "Email signature", range);
    assert_eq!(rig.rt.state.library.selection.bulk().len(), 5);
    click_row(&mut rig, "Current time", Mods::default());
    assert!(rig.rt.state.library.selection.bulk().is_empty());
    assert_eq!(cursor_name(&rig), "Current time");
}

#[test]
fn the_sort_menu_reorders_the_rows() {
    let mut rig = sample_rig();
    rig.click_key(library::SORT_KEY);
    rig.click_text("Most used");
    assert_eq!(rig.rt.state.library.sort, library::model::Sort::Used);
    let rows = row_names(&rig);
    assert_eq!(&rows[..2], ["Email signature", "Meeting follow-up"]);
    rig.click_key(library::SORT_KEY);
    rig.click_text("Name");
    assert_eq!(row_names(&rig)[0], "Bug report");
}

/// Changing views drops the multi-selection, and another view opens on its first row.
#[test]
fn a_new_view_drops_the_multi_selection_and_keeps_a_valid_selection() {
    let mut rig = sample_rig();
    focus_list(&mut rig);
    rig.press(cmd('a'));
    rig.press(cmd('2'));
    assert!(rig.rt.state.library.selection.bulk().is_empty());
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Favorites)
    );
    assert_eq!(cursor_name(&rig), "Email signature");
    let developer = group(&rig, "Developer");
    rig.rt
        .update(move |st, _| st.navigate_to(MainView::Library(LibraryFilter::Group(developer))));
    rig.settle();
    assert_eq!(cursor_name(&rig), "Bug report");
}

/// S41 follow-up: the group trail comes from the selected snippet in every library view, and
/// the filter crumbs are always links (`toolbar.tsx`).
#[test]
fn breadcrumbs_follow_the_selected_snippet_in_every_view() {
    let rig = sample_rig();
    let lib = rig.library();
    let signature = id(&rig, "Email signature");
    let trail = |view: MainView, selected: Option<Uuid>| -> Vec<(String, bool)> {
        compute_crumbs(&view, selected, &lib)
            .into_iter()
            .map(|c| (c.label, c.target.is_some()))
            .collect()
    };
    let crumb = |label: &str, link: bool| (label.to_owned(), link);
    assert_eq!(
        trail(MainView::Library(LibraryFilter::Favorites), Some(signature)),
        [
            crumb("Snippets", true),
            crumb("Favorites", true),
            crumb("Work", true),
            crumb("Email", true),
            crumb("Email signature", false),
        ]
    );
    assert_eq!(
        trail(MainView::Library(LibraryFilter::Recent), None),
        [crumb("Snippets", true), crumb("Recently used", true)]
    );
    // In a group view, the snippet's own group decides the trail.
    let work = group(&rig, "Work");
    assert_eq!(
        trail(
            MainView::Library(LibraryFilter::Group(work)),
            Some(signature)
        )
        .iter()
        .map(|c| c.0.as_str())
        .collect::<Vec<_>>(),
        ["Snippets", "Work", "Email", "Email signature"]
    );
    // A snippet without a group falls back to the view's group.
    let today = id(&rig, "Today");
    assert_eq!(
        trail(MainView::Library(LibraryFilter::Group(work)), Some(today))
            .iter()
            .map(|c| c.0.as_str())
            .collect::<Vec<_>>(),
        ["Snippets", "Work", "Today"]
    );
}
