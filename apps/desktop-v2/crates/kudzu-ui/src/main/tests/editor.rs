//! The snippet editor shell (S51): helpers for the editor test files, then the name field,
//! focus, the banners and the empty state.

use std::time::Duration;

use kudzu_core::{CaseMode, FireMode, GroupRecord, KudzuLibrary, SnippetRecord};
use rune::{Ime, Key, KeyEvent, Mods};
use uuid::Uuid;

use super::{cmd, esc, plain, Rig};
use crate::main::editor::{self, view as pane};
use crate::main::library;
use crate::main::state::MainPane;

pub(super) fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

pub(super) fn snippet(n: u128, name: &str, triggers: &[&str], content: &str) -> SnippetRecord {
    SnippetRecord {
        id: id(n),
        name: name.into(),
        triggers: triggers.iter().map(|t| t.to_string()).collect(),
        content: content.into(),
        formatting: None,
        group_id: None,
        fire_mode: FireMode::default(),
        case_mode: CaseMode::default(),
        insertion: None,
        rule: None,
        is_favorite: false,
        is_enabled: true,
        created_at: chrono::DateTime::from_timestamp(1_790_000_000, 0).unwrap(),
        updated_at: chrono::DateTime::from_timestamp(1_790_000_000, 0).unwrap(),
    }
}

pub(super) fn group(n: u128, name: &str, parent: Option<u128>) -> GroupRecord {
    GroupRecord {
        id: id(n),
        name: name.into(),
        parent_id: parent.map(id),
        sort_index: n as i64,
        is_enabled: true,
        insertion: None,
        rule: None,
    }
}

/// Replaces the library and opens the list as a fresh window does: the first snippet opens.
pub(super) fn seed(rig: &mut Rig, snippets: Vec<SnippetRecord>, groups: Vec<GroupRecord>) {
    rig.kudzu
        .library
        .transact(move |lib| {
            *lib = KudzuLibrary {
                snippets,
                groups,
                ..KudzuLibrary::default()
            };
            Ok(())
        })
        .unwrap();
    rig.rt.update(|s, _| library::restart(s));
    rig.settle();
}

/// A rig on its own library, the first snippet open.
pub(super) fn rig_with(snippets: Vec<SnippetRecord>, groups: Vec<GroupRecord>) -> Rig {
    let mut rig = Rig::new(true);
    seed(&mut rig, snippets, groups);
    rig
}

pub(super) fn open(rig: &mut Rig, n: u128) {
    rig.rt.update(move |s, _| s.select_snippet(Some(id(n))));
    rig.settle();
    assert_eq!(rig.rt.state.editor.open_id(), Some(id(n)));
}

pub(super) fn type_str(rig: &mut Rig, text: &str) {
    for ch in text.chars() {
        rig.press(plain(ch, false));
    }
}

pub(super) fn key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods::default(),
    }
}

pub(super) fn compose(rig: &mut Rig) {
    rig.rt.ime(Ime::Preedit("かん".into(), Some((0, 6))));
    rig.rt.frame(0.0);
}

pub(super) fn stored(rig: &Rig, n: u128) -> SnippetRecord {
    rig.library()
        .snippets
        .iter()
        .find(|s| s.id == id(n))
        .cloned()
        .unwrap_or_else(|| panic!("snippet {n} is in the library"))
}

pub(super) fn draft(rig: &Rig) -> SnippetRecord {
    rig.rt
        .state
        .editor
        .draft()
        .cloned()
        .expect("a snippet is open")
}

pub(super) fn texts(rig: &Rig) -> Vec<String> {
    rig.rt
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect()
}

pub(super) fn has_text(rig: &Rig, needle: &str) -> bool {
    texts(rig).iter().any(|t| t == needle)
}

/// Waits until the writer has written everything the library holds now.
pub(super) fn wait_durable(rig: &mut Rig) {
    let target = rig.kudzu.library.watch().revision();
    for _ in 0..200 {
        if rig.kudzu.library.saved().get().durable >= target {
            rig.settle();
            return;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(10));
        rig.settle();
    }
    panic!("the library never became durable at {target:?}");
}

/// Waits until the writer reports a failure.
pub(super) fn wait_save_error(rig: &mut Rig) {
    for _ in 0..200 {
        if rig.kudzu.library.saved().get().error.is_some() {
            rig.settle();
            return;
        }
        rig.fixtures.clock.advance(Duration::from_millis(100));
        std::thread::sleep(Duration::from_millis(10));
        rig.settle();
    }
    panic!("the writer never reported a failure");
}

/// Waits until the preview's request has landed (or was dropped).
pub(super) fn wait_preview(rig: &mut Rig) {
    for _ in 0..400 {
        rig.settle();
        if !rig.rt.state.editor.preview.is_pending() {
            rig.settle();
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the preview never landed");
}

#[test]
fn return_in_the_name_moves_to_the_triggers_and_suggests_the_prefix() {
    let mut rig = rig_with(vec![snippet(1, "Bare", &[], "")], vec![]);
    rig.click_key(editor::NAME_KEY);
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
    rig.press(key(Key::Enter));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::TRIGGER_KEY));
    assert_eq!(
        rig.rt.state.editor.trigger.edit.text, ";",
        "the prefix, on first focus"
    );
}

#[test]
fn the_name_ignores_return_while_composing() {
    let mut rig = rig_with(vec![snippet(1, "Bare", &[], "")], vec![]);
    rig.click_key(editor::NAME_KEY);
    compose(&mut rig);
    assert!(rig.rt.state.editor.name.is_composing());
    rig.press(key(Key::Enter));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
    rig.press(esc());
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
}

#[test]
fn typing_a_name_updates_the_draft_at_once() {
    let mut rig = rig_with(vec![snippet(1, "Sig", &[";s"], "x")], vec![]);
    rig.click_key(editor::NAME_KEY);
    rig.press(KeyEvent {
        key: Key::End,
        mods: Mods::default(),
    });
    type_str(&mut rig, "!!");
    assert_eq!(draft(&rig).name, "Sig!!");
    assert_eq!(stored(&rig, 1).name, "Sig", "not written yet");
    assert!(rig.rt.state.editor.is_dirty());
}

#[test]
fn new_and_duplicate_focus_the_name() {
    let mut rig = rig_with(vec![snippet(1, "Sig", &[";s"], "x")], vec![]);
    rig.press(cmd('n'));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
    open(&mut rig, 1);
    rig.press(esc());
    rig.press(cmd('d'));
    let copy = rig.rt.state.frame.selected_snippet.unwrap();
    assert_ne!(copy, id(1));
    assert_eq!(stored(&rig, copy.as_u128()).name, "Sig copy");
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::Editor);
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
}

#[test]
fn escape_in_the_content_returns_to_the_list() {
    let mut rig = rig_with(vec![snippet(1, "Sig", &[";s"], "x")], vec![]);
    rig.press(cmd('e'));
    assert_eq!(rig.focused_key().as_deref(), Some(editor::CONTENT_KEY));
    compose(&mut rig);
    rig.press(esc());
    assert_eq!(
        rig.focused_key().as_deref(),
        Some(editor::CONTENT_KEY),
        "not mid-composition"
    );
    rig.rt.ime(Ime::Commit("漢".into()));
    rig.settle();
    rig.press(esc());
    assert_eq!(rig.rt.state.frame.focus.active(), MainPane::List);
    assert!(!rig.rt.state.editor.is_editing_text());
}

#[test]
fn the_off_banners_and_the_no_trigger_note_show_tauris_copy() {
    let mut off = snippet(1, "Off", &[";o"], "x");
    off.is_enabled = false;
    let mut grouped = snippet(2, "Grouped", &[";g"], "x");
    grouped.group_id = Some(id(20));
    let mut work = group(10, "Work", None);
    work.is_enabled = false;
    let mut rig = rig_with(
        vec![off, grouped, snippet(3, "Lonely", &[], "text")],
        vec![work, group(20, "Email", Some(10))],
    );
    open(&mut rig, 1);
    assert!(has_text(&rig, pane::SNIPPET_OFF));
    open(&mut rig, 2);
    assert!(has_text(
        &rig,
        "This snippet's group “Work › Email” is turned off, so it won't expand."
    ));
    open(&mut rig, 3);
    assert!(!rig
        .rt
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(pane::BANNER_KEY)));
    assert!(has_text(&rig, pane::NO_TRIGGER));
}

#[test]
fn no_selection_shows_the_empty_state_with_new_snippet() {
    let mut rig = rig_with(vec![], vec![]);
    assert!(has_text(&rig, "No snippet selected"));
    assert!(has_text(
        &rig,
        "Pick one from the list, or start a new one."
    ));
    let before = rig.library().snippets.len();
    rig.click_key(pane::EMPTY_NEW_KEY);
    assert_eq!(rig.library().snippets.len(), before + 1);
    assert_eq!(rig.focused_key().as_deref(), Some(editor::NAME_KEY));
}
