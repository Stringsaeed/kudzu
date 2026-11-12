//! Opening, pasting, copying and the chords, from the hotkey to the target app.

use kudzu_core::clipboard::uti;
use rune::Key;

use super::{cmd, key, plain, with, Rig, NOTES};
use crate::picker::model::{ERROR_FLASH_SECS, FALLBACK_NOTICE};
use crate::shell::{NewSnippet, Route, SettingsPane};

#[test]
fn it_is_unavailable_while_recording_is_off_and_hides_if_recording_turns_off_while_open() {
    let mut rig = Rig::new();
    rig.kudzu
        .settings
        .update(|s| s.clipboard_enabled = false)
        .unwrap();
    rig.rt.update(|s, _| s.toggle());
    rig.settle();
    assert!(!rig.open_now(), "unavailable");
    rig.kudzu
        .settings
        .update(|s| s.clipboard_enabled = true)
        .unwrap();
    rig.open();
    assert!(rig.open_now());
    rig.kudzu
        .settings
        .update(|s| s.clipboard_enabled = false)
        .unwrap();
    rig.until("the panel to hide", |r| !r.open_now());
}

#[test]
fn every_opening_resets_the_filter_query_selection_and_undo_and_focuses_search() {
    let mut rig = Rig::with_clips(&["alpha", "beta", "gamma"]);
    rig.type_text("al");
    rig.press(key(Key::Tab));
    rig.press(with(Key::Backspace, true, false, false));
    rig.rt.update(|s, _| s.dismiss());
    rig.settle();
    assert!(!rig.open_now());
    rig.open();
    let p = &rig.state().model.picker;
    assert_eq!(p.query(), "");
    assert_eq!(p.filter, kudzu_core::clipboard::query::ClipFilter::All);
    assert!(p.undo.is_none(), "undo is for one opening");
    assert!(p.flash.is_none());
    assert_eq!(rig.active_text(), "gamma", "the newest clip is selected");
    assert!(p.edit.focused);
    assert_eq!(
        rig.rt
            .elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
            .as_deref(),
        Some("picker-field"),
        "search has Rune focus"
    );
}

#[test]
fn return_hides_the_panel_then_pastes_into_the_app_it_came_from() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    assert_eq!(rig.state().model.picker.paste_label(), "Paste into Notes");
    rig.press(key(Key::Enter));
    rig.until("the paste", |r| r.pasted() == 1);
    assert!(!rig.open_now(), "the panel hid before the paste");
    let pasted = rig.fixtures.pasteboard.pasted_into.lock().unwrap().clone();
    assert_eq!(
        pasted[0].as_ref().map(|a| a.bundle_id.as_str()),
        Some(NOTES)
    );
    let items = rig
        .fixtures
        .pasteboard
        .snapshot
        .lock()
        .unwrap()
        .items
        .clone();
    assert_eq!(
        items[0][0].1,
        b"second".to_vec(),
        "the active clip was on the pasteboard"
    );
}

#[test]
fn double_clicking_a_row_pastes_it() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    let target = rig.state().model.picker.clips.list.order()[1];
    let r = rig.rect(&format!("clip-{target}")).unwrap();
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    for _ in 0..2 {
        rig.rt.pointer_move(x, y);
        rig.rt.pointer_down(x, y);
        rig.rt.pointer_up(x, y);
    }
    rig.until("the paste", |r| r.pasted() == 1);
    let items = rig
        .fixtures
        .pasteboard
        .snapshot
        .lock()
        .unwrap()
        .items
        .clone();
    assert_eq!(items[0][0].1, b"first".to_vec());
}

#[test]
fn a_press_selects_a_row_and_the_search_field_keeps_focus() {
    let mut rig = Rig::with_clips(&["first", "second"]);
    let target = rig.state().model.picker.clips.list.order()[1];
    let r = rig.rect(&format!("clip-{target}")).unwrap();
    rig.rt.pointer_move(r[0] + 20.0, r[1] + 10.0);
    rig.rt.pointer_down(r[0] + 20.0, r[1] + 10.0);
    rig.rt.pointer_up(r[0] + 20.0, r[1] + 10.0);
    rig.settle();
    assert_eq!(rig.active_text(), "first");
    assert!(rig.state().model.picker.edit.focused);
    rig.type_text("x");
    assert_eq!(rig.query(), "x", "typing still goes to the field");
}

#[test]
fn shift_return_flips_the_plain_default_for_a_rich_clip() {
    let mut rig = Rig::new();
    rig.fixtures.pasteboard.external_copy(vec![vec![
        (uti::PLAIN_TEXT.into(), b"hello".to_vec()),
        (uti::HTML.into(), b"<b>hello</b>".to_vec()),
    ]]);
    rig.kudzu.clipboard.poll().unwrap();
    rig.open();
    rig.press(with(Key::Enter, false, true, false));
    rig.until("the paste", |r| r.pasted() == 1);
    let types: Vec<String> = rig
        .fixtures
        .pasteboard
        .snapshot
        .lock()
        .unwrap()
        .items
        .iter()
        .flat_map(|i| i.iter().map(|(t, _)| t.clone()))
        .collect();
    assert!(
        types.iter().all(|t| t != uti::HTML),
        "plain only: {types:?}"
    );
}

#[test]
fn command_digits_paste_the_nth_row() {
    let mut rig = Rig::with_clips(&["one", "two", "three"]);
    rig.press(cmd('2'));
    rig.until("the paste", |r| r.pasted() == 1);
    let items = rig
        .fixtures
        .pasteboard
        .snapshot
        .lock()
        .unwrap()
        .items
        .clone();
    assert_eq!(items[0][0].1, b"two".to_vec());
}

#[test]
fn command_return_copies_only_says_copied_and_does_not_paste() {
    let mut rig = Rig::with_clips(&["only"]);
    rig.press(with(Key::Enter, true, false, false));
    rig.until("Copied", |r| r.flash().as_deref() == Some("Copied"));
    assert_eq!(rig.pasted(), 0);
    assert!(rig.open_now(), "it hides after 420 ms, not at once");
    assert!(rig.state().model.picker.hide.is_some());
}

#[test]
fn a_clip_that_cannot_be_copied_shows_its_error_in_the_footer_and_stays_open() {
    let mut rig = Rig::new();
    // The sample history has entries but no stored data to copy back.
    let sample = kudzu_app::fixtures::SampleState::load().unwrap();
    rig.kudzu
        .clipboard
        .replace_history(sample.clipboard)
        .unwrap();
    rig.open();
    rig.press(key(Key::Enter));
    let sentence = "The saved data for this clipboard item is missing.";
    rig.until("the error", |r| r.flash().as_deref() == Some(sentence));
    assert!(rig.open_now());
    assert!(
        rig.has_text(sentence),
        "the footer shows the domain sentence"
    );
    assert_eq!(rig.pasted(), 0);
    let _ = ERROR_FLASH_SECS;
}

#[test]
fn a_paste_asked_for_before_the_session_lands_still_happens() {
    let mut rig = Rig::new();
    rig.copy("early");
    rig.rt.update(|s, _| s.toggle());
    rig.rt.update(|s, tx| {
        crate::picker::drive::apply(
            s,
            crate::picker::keys::Effect::Paste {
                nth: None,
                flip: false,
            },
            tx,
        )
    });
    rig.until("the paste", |r| r.pasted() == 1);
}

#[test]
fn pinning_flashes_and_the_order_stays_newest_first() {
    let mut rig = Rig::with_clips(&["old", "new"]);
    rig.press(cmd('p'));
    assert_eq!(rig.flash().as_deref(), Some("Pinned. It won't expire."));
    rig.press(key(Key::Down));
    rig.press(cmd('p'));
    rig.until("both pinned", |r| {
        r.kudzu
            .clipboard
            .watch()
            .get()
            .entries
            .iter()
            .all(|e| e.pinned)
    });
    assert_eq!(
        rig.order_texts(),
        vec!["new", "old"],
        "no pinned-first reorder"
    );
    rig.press(cmd('p'));
    assert_eq!(rig.flash().as_deref(), Some("Unpinned"));
}

#[test]
fn deleting_moves_to_the_neighbour_and_command_z_restores_only_with_an_empty_query() {
    let mut rig = Rig::with_clips(&["a", "b", "c"]);
    rig.press(key(Key::Down));
    assert_eq!(rig.active_text(), "b");
    rig.press(with(Key::Backspace, true, false, false));
    assert_eq!(rig.flash().as_deref(), Some("Deleted. ⌘Z to undo"));
    assert_eq!(rig.active_text(), "a", "the next row took its place");
    assert_eq!(rig.order_texts(), vec!["c", "a"]);
    // With a query ⌘Z belongs to the field: nothing is restored.
    rig.type_text("c");
    rig.press(cmd('z'));
    assert_eq!(rig.kudzu.clipboard.watch().get().entries.len(), 2);
    rig.press(with(Key::Backspace, false, false, false));
    assert_eq!(rig.query(), "");
    rig.press(cmd('z'));
    rig.until("the restore", |r| {
        r.kudzu.clipboard.watch().get().entries.len() == 3
    });
    assert_eq!(rig.flash().as_deref(), Some("Restored"));
    assert!(rig.state().model.picker.undo.is_none());
}

#[test]
fn deleting_the_last_row_selects_the_one_before() {
    let mut rig = Rig::with_clips(&["a", "b"]);
    rig.press(key(Key::Down));
    assert_eq!(rig.active_text(), "a");
    rig.press(with(Key::Backspace, true, false, false));
    assert_eq!(rig.active_text(), "b");
}

#[test]
fn command_s_saves_the_full_text_as_a_snippet_and_closes() {
    let mut rig = Rig::new();
    let long = "word ".repeat(1000);
    rig.copy(&long);
    rig.open();
    rig.press(cmd('s'));
    rig.until("the route", |r| r.state().model.last_route.is_some());
    assert_eq!(
        rig.state().model.last_route,
        Some(Route::NewSnippet(NewSnippet {
            content: long,
            ..NewSnippet::default()
        }))
    );
    assert!(!rig.open_now());
}

#[test]
fn command_o_opens_a_link_in_the_browser_and_closes() {
    let mut rig = Rig::with_clips(&["www.example.com/docs"]);
    rig.press(cmd('o'));
    assert_eq!(
        rig.rt.take_url_requests(),
        vec!["https://www.example.com/docs"]
    );
    assert!(!rig.open_now());
}

#[test]
fn an_ftp_link_stays_open_and_says_why_it_cannot_open() {
    let mut rig = Rig::with_clips(&["ftp://files.example.com/x.zip"]);
    rig.press(cmd('o'));
    assert!(rig.rt.take_url_requests().is_empty());
    assert!(rig.open_now());
    assert_eq!(rig.flash().as_deref(), Some("Kudzu can't open ftp: links."));
}

#[test]
fn command_o_opens_a_file_and_closes() {
    let mut rig = Rig::new();
    rig.fixtures.pasteboard.external_copy(vec![vec![(
        uti::FILE_URL.into(),
        b"file:///Users/me/Documents/report.pdf".to_vec(),
    )]]);
    rig.kudzu.clipboard.poll().unwrap();
    rig.open();
    rig.press(cmd('o'));
    rig.until("the open", |r| {
        !r.fixtures.files.opened.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.files.opened.lock().unwrap()[0].to_string_lossy(),
        "/Users/me/Documents/report.pdf"
    );
    assert!(!rig.open_now());
}

#[test]
fn command_comma_opens_clipboard_settings() {
    let mut rig = Rig::with_clips(&["a"]);
    rig.press(cmd(','));
    assert_eq!(
        rig.state().model.last_route,
        Some(Route::Settings(SettingsPane::Clipboard))
    );
    assert!(!rig.open_now());
}

#[test]
fn escape_clears_the_query_then_closes_and_gives_focus_back_to_the_app() {
    let mut rig = Rig::with_clips(&["alpha"]);
    rig.type_text("al");
    rig.press(key(Key::Escape));
    assert_eq!(rig.query(), "");
    assert!(rig.open_now());
    rig.press(key(Key::Escape));
    assert!(!rig.open_now());
    rig.until("the restore", |r| {
        !r.fixtures.apps.activations.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.apps.activations.lock().unwrap()[0].bundle_id,
        NOTES
    );
}

#[test]
fn a_blur_hides_without_giving_focus_back() {
    let mut rig = Rig::with_clips(&["alpha"]);
    rig.rt.update(|s, _| s.dismiss());
    rig.advance(0.5);
    assert!(!rig.open_now());
    assert!(rig.fixtures.apps.activations.lock().unwrap().is_empty());
}

#[test]
fn without_permission_the_footer_says_copy_and_the_fallback_sentence_shows() {
    let rig_setup = |allowed: bool| {
        let mut rig = Rig::new();
        *rig.fixtures.pasteboard.paste_allowed.lock().unwrap() = allowed;
        rig.copy("clip");
        rig.open();
        rig
    };
    let mut rig = rig_setup(false);
    assert_eq!(rig.state().model.picker.paste_label(), "Copy");
    assert!(rig.has_text("Copy"));
    rig.press(key(Key::Enter));
    rig.until("the sentence", |r| {
        r.flash().as_deref() == Some(FALLBACK_NOTICE)
    });
    assert!(rig.has_text(FALLBACK_NOTICE));
    assert!(rig.open_now(), "it stays long enough to read");
    assert_eq!(rig.pasted(), 0);
}

#[test]
fn keys_are_ignored_while_an_input_method_is_composing() {
    let mut rig = Rig::with_clips(&["one", "two"]);
    rig.rt.ime(rune::Ime::Preedit("に".into(), None));
    rig.settle();
    assert!(rig.state().model.picker.is_composing());
    rig.press(key(Key::Enter));
    rig.press(key(Key::Down));
    rig.press(key(Key::Escape));
    assert_eq!(
        rig.pasted(),
        0,
        "Return commits the composition, not a paste"
    );
    assert_eq!(rig.active_text(), "two", "the arrows did not move the list");
    assert!(
        rig.open_now(),
        "Escape cancels the composition, not the picker"
    );
    let _ = plain('x');
}
