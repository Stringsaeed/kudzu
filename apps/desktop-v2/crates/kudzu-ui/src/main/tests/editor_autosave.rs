//! S51 autosave (EDT-2, FEATURES §7): the 350 ms debounce, every flush path, "Saved" only once
//! the writer reaches the draft's revision, and "Couldn't save" until a write succeeds.

use std::os::unix::fs::PermissionsExt;

use kudzu_app::persist::PersistError;
use rune::{Key, KeyEvent, Mods};

use super::editor::{
    draft, has_text, open, rig_with, snippet, stored, type_str, wait_durable, wait_save_error,
};
use super::{cmd, Rig};
use crate::main::editor::{self, autosave, SaveStatus};

fn status(rig: &Rig) -> SaveStatus {
    let open = rig.rt.state.editor.open_id().expect("a snippet is open");
    rig.rt
        .state
        .editor
        .autosave
        .status(open, rig.rt.state.editor.is_dirty())
}

/// The header status's accessible name (its visible copy); `None` while idle, when the
/// transparent "Saved" is hidden from accessibility.
fn save_label(rig: &Rig) -> Option<String> {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(crate::main::editor::header::SAVE_KEY))
        .expect("the save status")
        .label
}

fn edit_name(rig: &mut Rig, suffix: &str) {
    rig.click_key(editor::NAME_KEY);
    rig.press(KeyEvent {
        key: Key::End,
        mods: Mods::default(),
    });
    type_str(rig, suffix);
}

fn two() -> Rig {
    rig_with(
        vec![
            snippet(1, "Sig", &[";s"], "x"),
            snippet(2, "Other", &[";o"], "y"),
        ],
        vec![],
    )
}

/// The data folder is unwritable while it lives; dropped, it is writable again.
struct ReadOnly(std::path::PathBuf);

impl ReadOnly {
    fn new(rig: &Rig) -> ReadOnly {
        let path = rig.data_dir().to_path_buf();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
        ReadOnly(path)
    }
}

impl Drop for ReadOnly {
    fn drop(&mut self) {
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
    }
}

#[test]
fn the_draft_is_written_350_ms_after_the_last_edit() {
    let mut rig = two();
    edit_name(&mut rig, "!");
    rig.rt.frame(0.2);
    edit_name(&mut rig, "?");
    rig.rt.frame(0.3);
    assert_eq!(
        stored(&rig, 1).name,
        "Sig",
        "the second edit restarted the debounce"
    );
    assert_eq!(status(&rig), SaveStatus::Saving);
    assert!(has_text(&rig, "Saving…"));
    rig.rt.frame(0.06);
    rig.rt.frame(0.0);
    assert_eq!(stored(&rig, 1).name, "Sig!?");
    assert!(!rig.rt.state.editor.is_dirty());
}

#[test]
fn saved_appears_only_once_the_writer_reaches_the_drafts_revision() {
    let mut rig = two();
    wait_durable(&mut rig);
    edit_name(&mut rig, "!");
    rig.press(cmd('s'));
    assert_eq!(stored(&rig, 1).name, "Sig!", "⌘S writes now");
    let accepted = rig.kudzu.library.watch().revision();
    assert!(rig.kudzu.library.saved().get().durable < accepted);
    rig.settle();
    assert_eq!(
        status(&rig),
        SaveStatus::Saving,
        "accepted in memory, not on disk"
    );
    assert_eq!(save_label(&rig).as_deref(), Some("Saving…"));
    assert!(!has_text(&rig, "Saved"));
    wait_durable(&mut rig);
    assert_eq!(status(&rig), SaveStatus::Saved);
    assert_eq!(save_label(&rig).as_deref(), Some("Saved"));
}

#[test]
fn a_failed_write_shows_couldnt_save_until_a_write_succeeds() {
    let mut rig = two();
    wait_durable(&mut rig);
    let lock = ReadOnly::new(&rig);
    edit_name(&mut rig, "!");
    rig.press(cmd('s'));
    wait_save_error(&mut rig);
    let sentence =
        PersistError::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied)).to_string();
    assert_eq!(sentence, "The file could not be saved.");
    assert_eq!(status(&rig), SaveStatus::Failed(sentence.clone()));
    assert_eq!(save_label(&rig).as_deref(), Some("Couldn't save"));
    assert!(has_text(&rig, "Couldn't save"));
    assert!(
        has_text(&rig, &sentence),
        "the toast carries the writer's sentence"
    );

    // Another snippet, then hiding and showing the window: still failed.
    open(&mut rig, 2);
    assert_eq!(status(&rig), SaveStatus::Failed(sentence.clone()));
    assert_eq!(save_label(&rig).as_deref(), Some("Couldn't save"));
    rig.rt.update(|s, _| s.hide());
    rig.settle();
    rig.rt.update(|s, _| s.show());
    rig.settle();
    assert_eq!(status(&rig), SaveStatus::Failed(sentence));

    // The writer's retry succeeds once the folder is writable.
    drop(lock);
    wait_durable(&mut rig);
    assert_eq!(
        status(&rig),
        SaveStatus::Idle,
        "the other snippet has no write of its own"
    );
    assert_eq!(save_label(&rig), None, "idle, so nothing is announced");
    open(&mut rig, 1);
    assert_eq!(stored(&rig, 1).name, "Sig!");
}

#[test]
fn a_refused_draft_shows_the_librarys_sentence() {
    let mut rig = two();
    edit_name(&mut rig, "!");
    // Another window deletes the snippet while the draft has edits.
    rig.kudzu
        .library
        .delete_snippets(&[super::editor::id(1)])
        .unwrap();
    rig.settle();
    assert!(has_text(&rig, "Couldn't save"));
    assert!(has_text(&rig, "That snippet no longer exists."));
}

#[test]
fn navigation_window_blur_and_hiding_write_the_draft() {
    let mut rig = two();
    edit_name(&mut rig, "1");
    rig.press(cmd('4'));
    assert_eq!(
        stored(&rig, 1).name,
        "Sig1",
        "leaving the library writes it"
    );
    rig.press(cmd('1'));
    open(&mut rig, 1);

    edit_name(&mut rig, "2");
    open(&mut rig, 2);
    assert_eq!(
        stored(&rig, 1).name,
        "Sig12",
        "opening another snippet writes it"
    );

    open(&mut rig, 1);
    edit_name(&mut rig, "3");
    rig.rt.update(|s, _| editor::window_focus(s, false));
    assert_eq!(
        stored(&rig, 1).name,
        "Sig123",
        "the window losing focus writes it"
    );

    edit_name(&mut rig, "4");
    rig.rt.update(|s, _| s.commit_drafts());
    assert_eq!(
        stored(&rig, 1).name,
        "Sig1234",
        "every hide and quit path writes it"
    );
}

#[test]
fn an_outside_change_is_taken_only_while_the_draft_is_clean() {
    let mut rig = two();
    let mut changed = stored(&rig, 1);
    changed.name = "From another Mac".into();
    rig.kudzu.library.update_snippet(changed).unwrap();
    rig.settle();
    assert_eq!(rig.rt.state.editor.name.text, "From another Mac");

    edit_name(&mut rig, "!");
    let mut again = stored(&rig, 1);
    again.name = "Ignored".into();
    again.content = "outside".into();
    rig.kudzu.library.update_snippet(again).unwrap();
    rig.settle();
    assert_eq!(rig.rt.state.editor.name.text, "From another Mac!");
    assert_eq!(
        draft(&rig).content,
        "x",
        "the dirty draft keeps its own copy"
    );
    rig.press(cmd('s'));
    assert_eq!(stored(&rig, 1).name, "From another Mac!");
}

#[test]
fn the_toggle_shortcuts_write_the_draft_first() {
    let mut rig = two();
    edit_name(&mut rig, "!");
    rig.press(rune::KeyEvent {
        key: Key::Escape,
        mods: Mods::default(),
    });
    rig.press(super::cmd_shift('f'));
    let saved = stored(&rig, 1);
    assert!(saved.is_favorite, "⌘⇧F flips the stored snippet");
    assert_eq!(saved.name, "Sig!", "after the draft was written");
    assert!(draft(&rig).is_favorite, "the clean draft takes the change");
    rig.rt.frame(0.4);
    assert!(stored(&rig, 1).is_favorite, "and never writes it back");
}

#[test]
fn flush_is_a_no_op_for_a_clean_draft() {
    let mut rig = two();
    let before = rig.kudzu.library.watch().revision();
    rig.rt.update(|s, _| {
        autosave::flush(s);
    });
    assert_eq!(rig.kudzu.library.watch().revision(), before);
}
