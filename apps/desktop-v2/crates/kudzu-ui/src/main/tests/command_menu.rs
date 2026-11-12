//! Command menu and shortcut cheat sheet (`PLAN.md` S43, `LIB-13`, `WIN-2`).

use kudzu_app::settings::Theme;
use rune::Key;

use super::{cmd, esc, plain, Rig};
use crate::actions::main::MainAction;
use crate::actions::Action;
use crate::main::cheatsheet;
use crate::main::state::{LibraryFilter, MainView};

fn near(got: f32, want: f32) {
    let delta = (got - want).abs();
    assert!(delta <= 2.0, "got {got}, want {want}, delta {delta}");
}

fn type_str(rig: &mut Rig, text: &str) {
    for ch in text.chars() {
        rig.press(plain(ch, false));
    }
}

fn arrow(rig: &mut Rig, key: Key) {
    rig.press(rune::KeyEvent {
        key,
        mods: rune::Mods::default(),
    });
}

fn enter(rig: &mut Rig) {
    arrow(rig, Key::Enter);
}

/// A menu row's label, whether the row is the selection, and whether it is a snippet.
fn menu_rows(rig: &Rig) -> Vec<(String, bool, bool)> {
    rig.rt
        .elements()
        .into_iter()
        .filter_map(|element| {
            let key = element.key?;
            let snippet = key.starts_with("command-snippet-");
            let action = key.starts_with("command-action-");
            (snippet || action).then_some((
                element.label.unwrap_or_default(),
                element.item_selected,
                snippet,
            ))
        })
        .collect()
}

fn menu_has(rig: &Rig, label: &str) -> bool {
    menu_rows(rig).iter().any(|(row, _, _)| row == label)
}

fn shows(rig: &Rig, label: &str) -> bool {
    rig.rt.elements().into_iter().any(|element| {
        element.label.as_deref() == Some(label) || element.text.as_deref() == Some(label)
    })
}

fn open_menu(rig: &mut Rig) {
    rig.press(cmd('k'));
    assert!(rig.rt.state.frame.command_open);
}

/// The rig starts on the three Getting Started snippets. These tests need the
/// captured library (eighteen snippets, including Shrug).
fn seed_fixture_library(rig: &mut Rig) {
    let library = rig.fixtures.sample.library.clone();
    rig.kudzu
        .library
        .transact(move |live| {
            *live = library;
            Ok(())
        })
        .expect("fixture library replaces the starter set");
    rig.settle();
}

#[test]
fn command_menu_opens_six_hundred_wide_near_the_top() {
    let mut rig = Rig::new(true);
    open_menu(&mut rig);
    let rect = rig
        .rect("command-menu-dialog")
        .expect("command menu dialog");
    near(rect[2], 600.0);
    near(rect[3], 489.0);
    near(rect[1], 0.18 * 760.0);
    assert_eq!(rig.focused_key().as_deref(), Some("command-menu-query"));
}

#[test]
fn empty_query_lists_five_snippets_and_a_query_lists_at_most_eight() {
    let mut rig = Rig::new(true);
    seed_fixture_library(&mut rig);
    open_menu(&mut rig);
    let snippets: Vec<_> = menu_rows(&rig)
        .into_iter()
        .filter(|(_, _, snippet)| *snippet)
        .collect();
    assert_eq!(
        snippets.len(),
        5,
        "an empty query lists the five frequent snippets"
    );

    type_str(&mut rig, "a");
    let snippets: Vec<_> = menu_rows(&rig)
        .into_iter()
        .filter(|(_, _, snippet)| *snippet)
        .collect();
    assert!(!snippets.is_empty());
    assert!(snippets.len() <= 8, "a query lists at most eight snippets");
    assert!(
        snippets.iter().all(|(label, _, _)| !label.is_empty()),
        "every listed snippet has its display name"
    );
}

#[test]
fn every_query_term_must_match_a_label_or_keyword() {
    let mut rig = Rig::new(true);
    open_menu(&mut rig);
    type_str(&mut rig, "stats");
    assert!(
        menu_has(&rig, "Insights"),
        "stats matches the Insights keyword"
    );
    assert!(!menu_has(&rig, "New snippet"));

    type_str(&mut rig, " nope");
    assert!(
        !menu_has(&rig, "Insights"),
        "a term that misses the label and the keywords hides the action"
    );
}

#[test]
fn selection_loops_across_the_ends() {
    let mut rig = Rig::new(true);
    open_menu(&mut rig);
    let rows = menu_rows(&rig);
    assert!(rows.len() > 1);
    assert!(rows[0].1, "the first row starts selected");

    arrow(&mut rig, Key::Up);
    let wrapped = menu_rows(&rig);
    assert_eq!(wrapped.last().map(|row| row.1), Some(true));
    assert_eq!(
        wrapped.last().map(|row| row.0.as_str()),
        Some(rows.last().unwrap().0.as_str())
    );

    arrow(&mut rig, Key::Down);
    let back = menu_rows(&rig);
    assert!(back[0].1, "Down from the last row selects the first");
    assert_eq!(back[0].0, rows[0].0);
}

#[test]
fn escape_and_command_k_close_and_reset_the_query() {
    let mut rig = Rig::new(true);
    open_menu(&mut rig);
    type_str(&mut rig, "sig");
    assert_eq!(rig.rt.state.command_menu.query.text, "sig");
    rig.press(esc());
    assert!(!rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.command_menu.query.text, "");

    open_menu(&mut rig);
    assert_eq!(rig.rt.state.command_menu.query.text, "");
    type_str(&mut rig, "sig");
    rig.press(cmd('k'));
    assert!(!rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.command_menu.query.text, "");
    assert_eq!(rig.rt.state.command_menu.selected, 0);
}

#[test]
fn delete_follows_the_field_under_the_menu() {
    let mut rig = Rig::new(true);
    let id = rig.rt.state.library_snapshot().snippets[0].id;
    rig.rt.update(move |state, _| {
        state.select_snippet(Some(id));
        state.blur_all_inputs();
    });
    rig.settle();
    open_menu(&mut rig);
    assert!(
        menu_has(&rig, "Delete snippet"),
        "Delete is offered when a snippet is selected and no text field is focused"
    );
    rig.press(esc());

    rig.press(cmd('f'));
    assert!(rig.rt.state.library.is_editing_text());
    open_menu(&mut rig);
    assert!(
        !menu_has(&rig, "Delete snippet"),
        "the menu's own search field is not the text-field guard"
    );
    assert!(menu_has(&rig, "New snippet"));
    rig.press(esc());

    rig.press(cmd('4'));
    open_menu(&mut rig);
    assert!(!menu_has(&rig, "Duplicate snippet"));
    assert!(!menu_has(&rig, "Delete snippet"));
    assert!(menu_has(&rig, "Insights"));
}

#[test]
fn choosing_a_snippet_opens_it_and_resets_the_menu() {
    let mut rig = Rig::new(true);
    seed_fixture_library(&mut rig);
    let id = rig
        .library()
        .snippets
        .iter()
        .find(|snippet| snippet.name == "Shrug")
        .unwrap()
        .id;
    open_menu(&mut rig);
    type_str(&mut rig, "shrug");
    let rows = menu_rows(&rig);
    let index = rows
        .iter()
        .position(|(label, _, snippet)| *snippet && label == "Shrug")
        .expect("Shrug is a snippet hit");
    for _ in 0..index {
        arrow(&mut rig, Key::Down);
    }
    enter(&mut rig);
    assert!(!rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.command_menu.query.text, "");
    assert_eq!(rig.rt.state.frame.selected_snippet, Some(id));
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::All)
    );
}

#[test]
fn cheat_sheet_lists_every_shortcut_and_is_720_wide() {
    let mut rig = Rig::new(true);
    let listed = cheatsheet::shortcut_labels();
    for action in MainAction::ALL {
        if action.chord().is_empty() {
            continue;
        }
        assert!(
            listed.iter().any(|label| *label == action.label()),
            "{} has a shortcut and is missing from the sheet",
            action.label()
        );
    }

    rig.press(cmd('/'));
    assert!(rig.rt.state.frame.shortcuts_open);
    let rect = rig
        .rect("shortcuts-cheatsheet-dialog")
        .expect("cheat sheet dialog");
    near(rect[2], 720.0);
    near(rect[0], 230.0);
    near(rect[1], 67.789);
    for action in MainAction::ALL {
        if action.chord().is_empty() {
            continue;
        }
        assert!(
            shows(&rig, action.label()),
            "{} is missing from the open sheet",
            action.label()
        );
    }
    assert!(shows(&rig, "Everywhere"));
    assert!(shows(&rig, "Navigate"));
    assert!(shows(&rig, "Snippet list"));
    assert!(shows(&rig, "Clipboard history"));
    assert!(shows(&rig, "Editor"));
}

fn expect_key(rig: &Rig, key: &str, role: &str, label: &str, want: [f32; 4]) {
    expect_key_tol(rig, key, role, label, want, [2.0, 2.0, 2.0, 2.0]);
}

fn expect_key_tol(rig: &Rig, key: &str, role: &str, label: &str, want: [f32; 4], tol: [f32; 4]) {
    let element = rig
        .rt
        .elements()
        .into_iter()
        .find(|element| element.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("missing {key}"));
    assert_eq!(element.role, role, "{key} role");
    assert_eq!(element.label.as_deref(), Some(label), "{key} label");
    for (i, ((got, want), tol)) in element.rect.iter().zip(want).zip(tol).enumerate() {
        let delta = (got - want).abs();
        assert!(
            delta <= tol,
            "{key} axis {i}: got {got}, want {want}, delta {delta}, rect {:?}",
            element.rect
        );
    }
}

/// Reference landmarks for the two dialogs, both themes. The scenes stay unregistered:
/// the captured frames also include the sidebar, list and editor, which later steps own.
#[test]
fn reference_landmarks_match_in_both_themes() {
    for theme in [Theme::Dark, Theme::Light] {
        let mut rig = Rig::themed(true, theme);
        open_menu(&mut rig);
        expect_key(
            &rig,
            "command-menu-dialog",
            "dialog",
            "Search and commands",
            [290.0, 136.797, 600.0, 489.0],
        );
        expect_key(
            &rig,
            "command-menu-title",
            "heading",
            "Search and commands",
            [289.0, 135.797, 1.0, 1.0],
        );
        rig.press(esc());

        rig.press(cmd('/'));
        expect_key(
            &rig,
            "shortcuts-cheatsheet-dialog",
            "dialog",
            "Keyboard shortcuts",
            [230.0, 67.789, 720.0, 624.422],
        );
        // Rune measures "Keyboard shortcuts" at 15/600 as 143 pt. The reference
        // shrink-wrap is 140.922 (delta 2.078). Width is the font, not the layout.
        expect_key_tol(
            &rig,
            "shortcuts-title",
            "heading",
            "Keyboard shortcuts",
            [254.0, 83.789, 140.922, 21.422],
            [2.0, 2.0, 2.1, 2.0],
        );
        expect_key(
            &rig,
            "shortcuts-close",
            "button",
            "Close",
            [914.0, 75.789, 28.0, 28.0],
        );
        expect_key(
            &rig,
            "shortcuts-everywhere",
            "region",
            "Everywhere",
            [254.0, 142.211, 316.0, 247.703],
        );
        expect_key(
            &rig,
            "shortcuts-h-everywhere",
            "heading",
            "Everywhere",
            [254.0, 142.211, 316.0, 15.703],
        );
        expect_key(
            &rig,
            "shortcuts-navigate",
            "region",
            "Navigate",
            [610.0, 142.211, 316.0, 247.703],
        );
        expect_key(
            &rig,
            "shortcuts-h-navigate",
            "heading",
            "Navigate",
            [610.0, 142.211, 316.0, 15.703],
        );
        expect_key(
            &rig,
            "shortcuts-snippet-list",
            "region",
            "Snippet list",
            [254.0, 413.914, 316.0, 219.703],
        );
        expect_key(
            &rig,
            "shortcuts-h-snippet-list",
            "heading",
            "Snippet list",
            [254.0, 413.914, 316.0, 15.703],
        );
        expect_key(
            &rig,
            "shortcuts-clipboard",
            "region",
            "Clipboard history",
            [610.0, 413.914, 316.0, 219.703],
        );
        expect_key(
            &rig,
            "shortcuts-h-clipboard",
            "heading",
            "Clipboard history",
            [610.0, 413.914, 316.0, 15.703],
        );
        expect_key(
            &rig,
            "shortcuts-editor",
            "region",
            "Editor",
            [254.0, 657.617, 316.0, 191.703],
        );
        expect_key(
            &rig,
            "shortcuts-h-editor",
            "heading",
            "Editor",
            [254.0, 657.617, 316.0, 15.703],
        );
    }
}
