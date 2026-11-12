//! S43 review: the command catalog, one shortcut list, viewport sizing, and the five minors.

use std::time::{Duration, Instant};

use kudzu_app::settings::Theme as StoredTheme;
use kudzu_core::keys::key_labels;
use rune::{FileDialogKind, Ime, Key};
use uuid::Uuid;

use super::{cmd, esc, plain, Rig};
use crate::actions::main::MainAction;
use crate::actions::Action;
use crate::main::state::{LibraryFilter, MainView};
use crate::main::{cheatsheet, clipboard, keys};

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

fn menu_rows(rig: &Rig) -> Vec<(String, bool)> {
    rig.rt
        .elements()
        .into_iter()
        .filter_map(|element| {
            let key = element.key?;
            (key.starts_with("command-snippet-") || key.starts_with("command-action-"))
                .then_some((element.label.unwrap_or_default(), element.item_selected))
        })
        .collect()
}

fn menu_has(rig: &Rig, label: &str) -> bool {
    menu_rows(rig).iter().any(|(row, _)| row == label)
}

fn open_menu(rig: &mut Rig) {
    rig.press(cmd('k'));
    assert!(rig.rt.state.frame.command_open);
}

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

fn section_titles(rig: &Rig) -> Vec<String> {
    rig.rt
        .elements()
        .into_iter()
        .filter_map(|element| {
            element
                .key
                .as_deref()
                .filter(|key| key.starts_with("command-section-"))
                .and(element.text.or(element.label))
        })
        .collect()
}

fn activate(rig: &mut Rig, label: &str) {
    for _ in 0..64 {
        let rows = menu_rows(rig);
        if rows.iter().any(|(row, on)| *on && row == label) {
            enter(rig);
            return;
        }
        assert!(
            rows.iter().any(|(row, _)| row == label),
            "the menu has no row {label}"
        );
        arrow(rig, Key::Down);
    }
    panic!("could not reach {label}");
}

fn snippet_labels(rig: &Rig) -> Vec<String> {
    rig.rt
        .elements()
        .into_iter()
        .filter_map(|element| {
            let key = element.key?;
            key.starts_with("command-snippet-")
                .then_some(element.label.unwrap_or_default())
        })
        .collect()
}

fn inside(elements: &[rune::ElementInfo], ancestor: &str, key: &str) -> bool {
    let Some(index) = elements
        .iter()
        .position(|element| element.key.as_deref() == Some(key))
    else {
        return false;
    };
    let mut cursor = Some(index);
    while let Some(at) = cursor {
        if elements[at].key.as_deref() == Some(ancestor) {
            return true;
        }
        cursor = elements[at].parent;
    }
    false
}

fn caps_under(rig: &Rig, row_key: &str) -> Vec<String> {
    let elements = rig.rt.elements();
    let Some(row) = elements
        .iter()
        .position(|element| element.key.as_deref() == Some(row_key))
    else {
        return Vec::new();
    };
    elements
        .iter()
        .filter_map(|element| {
            let mut cursor = element.parent;
            let mut under = false;
            while let Some(at) = cursor {
                if at == row {
                    under = true;
                    break;
                }
                cursor = elements[at].parent;
            }
            if !under {
                return None;
            }
            element.text.clone()
        })
        .filter(|text| text != "Keyboard shortcuts")
        .collect()
}

fn wait_until(rig: &mut Rig, ready: impl Fn(&Rig) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready(rig) {
        assert!(Instant::now() < deadline, "the command did not finish");
        std::thread::sleep(Duration::from_millis(5));
        rig.settle();
    }
}

fn displayed_chord(action: MainAction) -> String {
    match action {
        MainAction::OpenSettings => "mod+,".to_string(),
        MainAction::OpenShortcuts => "mod+/".to_string(),
        other => other.chord().to_string(),
    }
}

#[test]
fn catalog_sections_name_the_selection_and_hide_groups_until_a_query() {
    let mut rig = Rig::new(true);
    // Dev starts paused, so the live label would be Resume until expansion is on.
    rig.kudzu
        .settings
        .update(|settings| settings.paused = false)
        .unwrap();
    seed_fixture_library(&mut rig);
    let id = rig
        .library()
        .snippets
        .iter()
        .find(|snippet| snippet.name == "Email signature")
        .unwrap()
        .id;
    rig.rt.update(move |state, _| {
        state.select_snippet(Some(id));
        state.blur_all_inputs();
    });
    rig.settle();
    open_menu(&mut rig);

    assert_eq!(
        section_titles(&rig),
        ["Frequently used", "Selected snippet", "Actions", "Go to"]
    );
    assert!(menu_has(&rig, "Unfavorite “Email signature”"));
    assert!(menu_has(&rig, "Turn off “Email signature”"));
    assert!(menu_has(&rig, "Duplicate “Email signature”"));
    assert!(menu_has(&rig, "Delete “Email signature”"));
    assert!(menu_has(&rig, "Pause expanding"));
    assert!(menu_has(&rig, "Import snippets…"));
    assert!(menu_has(&rig, "Export library…"));
    assert!(menu_has(&rig, "Switch to light appearance"));
    assert!(menu_has(&rig, "Pause clipboard history"));
    assert!(menu_has(&rig, "Settings › Clipboard"));
    assert!(menu_has(&rig, "Why didn’t this expand?"));
    assert!(menu_has(&rig, "Settings › Apps"));
    assert!(menu_has(&rig, "Settings › Backups & data"));
    assert!(!menu_has(&rig, "Work › Email"));
    assert!(menu_rows(&rig)
        .iter()
        .all(|(label, _)| !label.starts_with("Group settings:")));

    type_str(&mut rig, "rules");
    assert!(menu_has(&rig, "Group settings: Work"));
    assert!(menu_has(&rig, "Group settings: Work › Email"));
    assert!(!menu_has(&rig, "Work › Email"));

    rig.press(esc());
    open_menu(&mut rig);
    type_str(&mut rig, "email");
    assert!(menu_has(&rig, "Work › Email"));
    assert!(menu_has(&rig, "Group settings: Work › Email"));
}

#[test]
fn import_export_appearance_clipboard_and_settings_commands_run() {
    let mut rig = Rig::new(true);
    rig.kudzu
        .settings
        .update(|settings| settings.paused = false)
        .unwrap();
    seed_fixture_library(&mut rig);
    let before = rig.library().snippets.len();

    open_menu(&mut rig);
    type_str(&mut rig, "raycast");
    assert!(menu_has(&rig, "Import snippets…"));
    activate(&mut rig, "Import snippets…");
    assert!(!rig.rt.state.frame.command_open);
    let (kind, options) = rig.rt.file_dialog().expect("import opens a file dialog");
    assert_eq!(kind, FileDialogKind::Open);
    assert_eq!(options.title, "Import snippets");
    assert_eq!(
        options.filters,
        vec![(
            "Snippet files".into(),
            ["json", "yml", "yaml", "textexpander", "csv"]
                .into_iter()
                .map(str::to_string)
                .collect()
        )]
    );
    let path = rig._scratch.0.join("raycast.json");
    std::fs::write(
        &path,
        r#"[{"name":"Raycast fixture","keyword":";rayfix","text":"fixture"}]"#,
    )
    .unwrap();
    rig.rt.resolve_file_dialog(Some(path)).unwrap();
    rig.settle();
    wait_until(&mut rig, |rig| rig.library().snippets.len() > before);
    assert!(rig
        .library()
        .snippets
        .iter()
        .any(|snippet| snippet.name == "Raycast fixture"));

    open_menu(&mut rig);
    type_str(&mut rig, "backup");
    assert!(menu_has(&rig, "Export library…"));
    activate(&mut rig, "Export library…");
    let (kind, options) = rig.rt.file_dialog().expect("export opens a save dialog");
    assert_eq!(kind, FileDialogKind::Save);
    assert_eq!(options.title, "Export library");
    let file_name = options.file_name.clone().unwrap_or_default();
    assert!(
        file_name.starts_with("kudzu-") && file_name.ends_with(".json"),
        "export suggests the data service's file name, got {file_name}"
    );
    assert_eq!(
        options.filters,
        vec![("Kudzu library".into(), vec!["json".into()])]
    );
    let destination = rig._scratch.0.join(&file_name);
    rig.rt
        .resolve_file_dialog(Some(destination.clone()))
        .unwrap();
    rig.settle();
    wait_until(&mut rig, |_| destination.is_file());

    open_menu(&mut rig);
    type_str(&mut rig, "theme");
    assert!(menu_has(&rig, "Switch to light appearance"));
    activate(&mut rig, "Switch to light appearance");
    assert_eq!(rig.kudzu.settings.watch().get().theme, StoredTheme::Light);
    open_menu(&mut rig);
    type_str(&mut rig, "theme");
    assert!(menu_has(&rig, "Switch to dark appearance"));
    activate(&mut rig, "Switch to dark appearance");
    assert_eq!(rig.kudzu.settings.watch().get().theme, StoredTheme::Dark);

    open_menu(&mut rig);
    type_str(&mut rig, "record");
    assert!(menu_has(&rig, "Pause clipboard history"));
    activate(&mut rig, "Pause clipboard history");
    assert!(!rig.kudzu.settings.watch().get().clipboard_enabled);
    open_menu(&mut rig);
    type_str(&mut rig, "record");
    assert!(menu_has(&rig, "Resume clipboard history"));
    activate(&mut rig, "Resume clipboard history");
    assert!(rig.kudzu.settings.watch().get().clipboard_enabled);

    open_menu(&mut rig);
    type_str(&mut rig, "disable");
    assert!(menu_has(&rig, "Pause expanding"));
    activate(&mut rig, "Pause expanding");
    assert!(rig.kudzu.settings.watch().get().paused);
    open_menu(&mut rig);
    type_str(&mut rig, "disable");
    assert!(menu_has(&rig, "Resume expanding"));
    rig.press(esc());

    let panes = [
        (
            "retention",
            "Settings › Clipboard",
            MainView::Settings(crate::shell::SettingsPane::Clipboard),
        ),
        (
            "troubleshooting",
            "Why didn’t this expand?",
            MainView::Settings(crate::shell::SettingsPane::Activity),
        ),
        (
            "terminal",
            "Settings › Apps",
            MainView::Settings(crate::shell::SettingsPane::Apps),
        ),
        (
            "restore",
            "Settings › Backups & data",
            MainView::Settings(crate::shell::SettingsPane::Data),
        ),
    ];
    for (query, label, view) in panes {
        open_menu(&mut rig);
        type_str(&mut rig, query);
        assert!(menu_has(&rig, label), "{query} lists {label}");
        activate(&mut rig, label);
        assert_eq!(rig.rt.state.frame.view, view);
    }
}

#[test]
fn group_rows_open_the_group_and_its_settings() {
    let mut rig = Rig::new(true);
    seed_fixture_library(&mut rig);
    let email = Uuid::parse_str("d199e481-217b-4d81-97e5-778d9a849eee").unwrap();
    let work = Uuid::parse_str("294be326-0bf6-4380-8e1a-5f5ce269c306").unwrap();

    open_menu(&mut rig);
    type_str(&mut rig, "email");
    activate(&mut rig, "Work › Email");
    assert_eq!(
        rig.rt.state.frame.view,
        MainView::Library(LibraryFilter::Group(email))
    );
    assert!(!rig.rt.state.frame.command_open);

    open_menu(&mut rig);
    type_str(&mut rig, "rules");
    activate(&mut rig, "Group settings: Work");
    assert_eq!(rig.rt.state.frame.group_settings_id, Some(work));
    assert!(!rig.rt.state.frame.command_open);
}

#[test]
fn cheat_sheet_rows_identify_the_action_and_show_the_registry_chord() {
    let mut rig = Rig::new(true);
    rig.press(cmd('/'));
    assert!(rig.rt.state.frame.shortcuts_open);
    let elements = rig.rt.elements();
    for action in MainAction::ALL {
        let key = format!("shortcuts-action-{action:?}");
        if action.chord().is_empty() {
            assert!(
                !inside(&elements, "shortcuts-cheatsheet-dialog", &key),
                "{} has no shortcut, so the sheet has no row for it",
                action.label()
            );
            continue;
        }
        assert!(
            inside(&elements, "shortcuts-cheatsheet-dialog", &key),
            "{} is missing from the cheat sheet dialog",
            action.label()
        );
        assert_eq!(
            caps_under(&rig, &key),
            key_labels(&displayed_chord(*action)),
            "{} shows a chord the registry does not",
            action.label()
        );
    }
    for shortcut in clipboard::SCREEN_SHORTCUTS {
        let key = format!("shortcuts-local-{}", shortcut.id);
        assert!(
            inside(&elements, "shortcuts-cheatsheet-dialog", &key),
            "clipboard shortcut {} is missing from the dialog",
            shortcut.label
        );
        assert_eq!(
            caps_under(&rig, &key),
            key_labels(shortcut.chord),
            "{} shows a chord other than the clipboard screen's",
            shortcut.label
        );
    }
}

#[test]
fn menu_and_sheet_follow_the_viewport_and_an_empty_menu_hugs_its_rows() {
    let mut rig = Rig::new(true);
    rig.rt.set_viewport((900.0, 560.0), 1.0);
    rig.settle();
    open_menu(&mut rig);
    let menu = rig.rect("command-menu-dialog").expect("menu");
    assert!((menu[2] - 600.0).abs() <= 2.0, "width {}", menu[2]);
    assert!(
        (menu[1] - 0.18 * 560.0).abs() <= 2.0,
        "18% placement, y {}",
        menu[1]
    );
    let list_cap = 440.0_f32.min(0.6 * 560.0);
    let full = 48.0 + 1.0 + list_cap;
    assert!(
        (menu[3] - full).abs() <= 2.0,
        "a full menu is the viewport list cap tall, height {}",
        menu[3]
    );
    assert!(menu[1] + menu[3] <= 560.0, "the menu stays inside 560");
    rig.press(esc());

    rig.press(cmd('/'));
    let sheet = rig.rect("shortcuts-cheatsheet-dialog").expect("sheet");
    assert!((sheet[2] - 720.0).abs() <= 2.0, "width {}", sheet[2]);
    let body_cap = 640.0_f32.min(0.75 * 560.0);
    let sheet_h = 54.422 + body_cap;
    assert!(
        (sheet[3] - sheet_h).abs() <= 2.0,
        "the sheet body is min(640, 75vh), height {}",
        sheet[3]
    );
    assert!(sheet[1] >= 0.0 && sheet[1] + sheet[3] <= 560.0);
    rig.press(esc());

    rig.rt.set_viewport((1180.0, 760.0), 1.0);
    rig.settle();
    open_menu(&mut rig);
    type_str(&mut rig, "zzzz-no-such");
    let empty = rig.rect("command-menu-dialog").expect("empty menu");
    assert!((empty[2] - 600.0).abs() <= 2.0);
    assert!((empty[1] - 0.18 * 760.0).abs() <= 2.0);
    assert!(
        empty[3] < 160.0,
        "no results hugs the search row and the message, height {}",
        empty[3]
    );
    assert!(menu_rows(&rig).is_empty());
}

#[test]
fn toolbar_search_opens_through_the_same_transition_as_command_k() {
    let mut rig = Rig::new(true);
    let id = rig.rt.state.library_snapshot().snippets[0].id;
    rig.rt.update(move |state, _| {
        state.select_snippet(Some(id));
        state.blur_all_inputs();
    });
    rig.settle();

    rig.press(cmd('f'));
    assert!(rig.rt.state.library.is_editing_text());
    rig.click_key("toolbar-search-trigger");
    assert!(rig.rt.state.frame.command_open);
    assert_eq!(rig.focused_key().as_deref(), Some("command-menu-query"));
    assert!(
        menu_rows(&rig)
            .iter()
            .all(|(label, _)| !label.starts_with("Delete “")),
        "the toolbar snapshots the filter under the menu"
    );
    rig.press(esc());

    rig.press(cmd('f'));
    open_menu(&mut rig);
    assert_eq!(rig.focused_key().as_deref(), Some("command-menu-query"));
    assert!(menu_rows(&rig)
        .iter()
        .all(|(label, _)| !label.starts_with("Delete “")));
}

#[test]
fn escape_with_both_dialogs_open_dismisses_the_sheet_then_resets_the_menu() {
    let mut rig = Rig::new(true);
    open_menu(&mut rig);
    type_str(&mut rig, "sig");
    rig.rt.update(|state, _| cheatsheet::open(state));
    rig.settle();
    assert!(rig.rt.state.frame.command_open);
    assert!(rig.rt.state.frame.shortcuts_open);

    rig.rt.update(|state, tx| {
        let _ = keys::handle_key(state, &esc(), tx);
    });
    rig.settle();
    assert!(
        !rig.rt.state.frame.shortcuts_open,
        "the sheet is drawn above the menu, so Escape dismisses it first"
    );
    assert!(rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.command_menu.query.text, "sig");

    rig.press(esc());
    assert!(!rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.command_menu.query.text, "");
}

#[test]
fn the_menu_field_ignores_keys_while_composing() {
    let mut rig = Rig::new(true);
    seed_fixture_library(&mut rig);
    open_menu(&mut rig);
    type_str(&mut rig, "sig");
    let selected = rig.rt.state.command_menu.selected;
    let view = rig.rt.state.frame.view.clone();
    // Library sync already selects the first snippet. Enter must leave it.
    let snippet = rig.rt.state.frame.selected_snippet;
    rig.rt.ime(Ime::Preedit("かん".into(), Some((0, 6))));
    rig.rt.frame(0.016);
    assert!(rig.rt.state.command_menu.query.is_composing());

    arrow(&mut rig, Key::Down);
    assert_eq!(rig.rt.state.command_menu.selected, selected);
    enter(&mut rig);
    assert!(rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.frame.view, view);
    assert_eq!(rig.rt.state.frame.selected_snippet, snippet);
}

#[test]
fn frequent_snippets_follow_usage_then_name_and_insights_runs_from_the_menu() {
    let mut rig = Rig::new(true);
    seed_fixture_library(&mut rig);
    open_menu(&mut rig);
    assert_eq!(
        snippet_labels(&rig),
        [
            "Email signature",
            "Meeting follow-up",
            "Out of office",
            "Today",
            "Current time"
        ]
    );
    type_str(&mut rig, "stats");
    activate(&mut rig, "Insights");
    assert_eq!(rig.rt.state.frame.view, MainView::Insights);
    assert!(!rig.rt.state.frame.command_open);
    assert_eq!(rig.rt.state.command_menu.query.text, "");
}
