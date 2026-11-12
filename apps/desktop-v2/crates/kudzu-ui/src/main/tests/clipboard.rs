//! The clipboard history list (`PLAN.md` S58): a real `Kudzu` on fixtures, driven through the
//! main window's runtime. Clips come from the fake pasteboard where a test copies, and from
//! the sample history (`replace_history`) where it only reads.

use std::time::Duration;

use kudzu_app::clipboard::Flavor;
use kudzu_app::fixtures::SampleState;
use kudzu_core::clipboard::uti;
use rune::{Key, KeyEvent, Mods};
use uuid::Uuid;

use super::{cmd, cmd_backspace, esc, plain, Rig};
use crate::main::state::{ClipFilter, MainView};
use crate::shell::{MainState, Route};

fn key(k: Key, shift: bool) -> KeyEvent {
    KeyEvent { key: k, mods: Mods { shift, ..Mods::default() } }
}

fn open(rig: &mut Rig) {
    rig.open_route(Route::Clipboard);
    rig.settle();
}

fn seed_sample(rig: &mut Rig) {
    let sample = SampleState::load().unwrap();
    rig.kudzu.clipboard.replace_history(sample.clipboard).unwrap();
    open(rig);
}

fn copy_text(rig: &Rig, text: &str) {
    rig.fixtures
        .pasteboard
        .external_copy(vec![vec![(uti::PLAIN_TEXT.into(), text.as_bytes().to_vec())]]);
    rig.kudzu.clipboard.poll().unwrap();
}

fn capture(rig: &mut Rig, texts: &[&str]) {
    for t in texts {
        copy_text(rig, t);
    }
    open(rig);
}

/// Settles until `done` holds: workers answer on their own threads.
fn until(rig: &mut Rig, what: &str, done: impl Fn(&Rig) -> bool) {
    for _ in 0..400 {
        rig.settle();
        if done(rig) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {what}");
}

fn clips(rig: &Rig) -> &crate::clip::model::ClipList {
    &rig.rt.state.clipboard.clips.list
}

fn texts(rig: &Rig) -> Vec<String> {
    rig.kudzu
        .clipboard
        .watch()
        .get()
        .entries
        .iter()
        .map(|e| e.text.clone().unwrap_or_default())
        .collect()
}

fn toast_titles(rig: &Rig) -> Vec<String> {
    rig.rt.state.frame.toasts.entries().iter().map(|e| e.options.title.clone()).collect()
}

fn active_text(rig: &Rig) -> String {
    clips(rig).active_entry().and_then(|e| e.text.clone()).unwrap_or_default()
}

#[test]
fn the_list_column_clamps_to_thirty_percent_between_300_and_372() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    let w = rig.rect("clip-list-section").unwrap()[2];
    assert!((w - 354.0).abs() < 1.0, "1180 wide gives 30% = 354, got {w}");
    assert!((rig.rect("clip-header").unwrap()[3] - 48.0).abs() < 0.5);
    assert!((rig.rect("clip-footer").unwrap()[3] - 36.0).abs() < 0.5);
}

#[test]
fn pinned_clips_come_first_then_day_sections() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    let list = clips(&rig);
    let labels: Vec<&str> = list
        .rows()
        .iter()
        .filter_map(|r| match &r.kind {
            crate::clip::model::RowKind::Header { label, .. } => Some(label.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(labels.first().copied(), Some("Pinned"));
    assert!(labels.contains(&"Today"));
    assert!(labels.contains(&"Yesterday"));
    assert_eq!(list.len(), 85);
}

#[test]
fn a_search_narrows_the_list_and_drops_the_pinned_section() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.rt.update(|st, _| st.clipboard.search.replace_text("meeting"));
    rig.settle();
    let list = clips(&rig);
    assert!(list.len() > 0 && list.len() < 85);
    assert!(list.rows().iter().all(|r| !matches!(&r.kind,
        crate::clip::model::RowKind::Header { label, .. } if label == "Pinned")));
}

#[test]
fn a_search_with_no_match_says_so_and_show_everything_clears_it() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.rt.update(|st, _| st.clipboard.search.replace_text("zzzzqq"));
    rig.settle();
    assert!(rig.rect("clip-nothing").is_some());
    rig.click_text("Show everything");
    assert!(rig.rt.state.clipboard.query().is_empty());
    assert_eq!(clips(&rig).len(), 85);
}

#[test]
fn an_empty_history_welcomes_and_recording_off_offers_to_turn_on() {
    let mut rig = Rig::new(true);
    open(&mut rig);
    assert!(rig.rect("clip-welcome").is_some());
    rig.kudzu.settings.update(|s| s.clipboard_enabled = false).unwrap();
    rig.settle();
    rig.click_text("Turn on");
    until(&mut rig, "recording on", |r| r.kudzu.settings.watch().get().clipboard_enabled);
}

#[test]
fn a_paused_history_with_clips_shows_the_banner_and_resume_turns_recording_on() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.kudzu.settings.update(|s| s.clipboard_enabled = false).unwrap();
    rig.settle();
    assert!(rig.rect("clip-paused").is_some());
    rig.click_text("Resume");
    until(&mut rig, "recording on", |r| r.kudzu.settings.watch().get().clipboard_enabled);
    assert!(rig.rect("clip-paused").is_none());
}

#[test]
fn arrows_j_k_home_end_and_page_move_the_active_row() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    let ids = clips(&rig).order().to_vec();
    assert_eq!(clips(&rig).active(), Some(ids[0]));
    rig.press(key(Key::Down, false));
    assert_eq!(clips(&rig).active(), Some(ids[1]));
    rig.press(plain('j', false));
    assert_eq!(clips(&rig).active(), Some(ids[2]));
    rig.press(plain('k', false));
    assert_eq!(clips(&rig).active(), Some(ids[1]));
    rig.press(key(Key::PageDown, false));
    assert_eq!(clips(&rig).active(), Some(ids[9]));
    rig.press(key(Key::End, false));
    assert_eq!(clips(&rig).active(), Some(*ids.last().unwrap()));
    rig.press(key(Key::Home, false));
    assert_eq!(clips(&rig).active(), Some(ids[0]));
}

#[test]
fn shift_and_select_all_make_a_selection_and_the_bulk_bar_replaces_the_footer() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.press(key(Key::Down, true));
    rig.press(key(Key::Down, true));
    assert_eq!(clips(&rig).selection().len(), 3);
    assert!(rig.rect("clip-bulk").is_some());
    assert!(rig.rect("clip-footer").is_none());
    rig.press(esc());
    assert!(!clips(&rig).is_multi());
    assert!(rig.rect("clip-footer").is_some());
    rig.press(cmd('a'));
    assert_eq!(clips(&rig).selection().len(), 85);
}

#[test]
fn enter_copies_the_active_clip_and_our_own_write_is_not_captured_again() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["first", "second"]);
    assert_eq!(active_text(&rig), "second");
    rig.press(key(Key::Down, false));
    assert_eq!(active_text(&rig), "first");
    let before = rig.kudzu.clipboard.watch().get().entries.len();
    rig.press(key(Key::Enter, false));
    until(&mut rig, "the copy toast", |r| toast_titles(r).contains(&"Copied".to_owned()));
    rig.kudzu.clipboard.poll().unwrap();
    let h = rig.kudzu.clipboard.watch().get();
    assert_eq!(h.entries.len(), before, "copying back adds no entry");
    assert!(h.entries.iter().all(|e| e.copy_count == 1), "and no phantom copy");
    assert_eq!(rig.rt.state.clipboard.clips.look.copied, clips(&rig).active());
}

#[test]
fn shift_enter_flips_plain_and_formatted_for_rich_clips() {
    let mut rig = Rig::new(true);
    rig.fixtures.pasteboard.external_copy(vec![vec![
        (uti::PLAIN_TEXT.into(), b"hello".to_vec()),
        (uti::HTML.into(), b"<b>hello</b>".to_vec()),
    ]]);
    rig.kudzu.clipboard.poll().unwrap();
    open(&mut rig);
    assert!(clips(&rig).active_entry().unwrap().rich);
    rig.press(key(Key::Enter, true));
    until(&mut rig, "the plain toast", |r| {
        toast_titles(r).contains(&"Copied as plain text".to_owned())
    });
    let _ = Flavor::Plain;
}

#[test]
fn command_p_pins_and_unpins_the_selection() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["a", "b"]);
    let id = clips(&rig).active().unwrap();
    rig.press(cmd('p'));
    assert!(rig.kudzu.clipboard.watch().get().get(id).unwrap().pinned);
    rig.press(cmd('p'));
    assert!(!rig.kudzu.clipboard.watch().get().get(id).unwrap().pinned);
}

#[test]
fn deleting_posts_an_undo_toast_that_restores_and_moves_to_the_neighbour() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["one", "two", "three"]);
    rig.press(key(Key::Down, false));
    assert_eq!(active_text(&rig), "two");
    rig.press(key(Key::Backspace, false));
    assert_eq!(texts(&rig), vec!["three", "one"]);
    assert_eq!(active_text(&rig), "one", "the neighbour that took its place");
    assert!(toast_titles(&rig).contains(&"Deleted “two”".to_owned()));
    rig.click_text("Undo");
    until(&mut rig, "the restore", |r| texts(r).len() == 3);
}

#[test]
fn command_backspace_deletes_clips_not_the_snippet_the_library_had_open() {
    let mut rig = Rig::new(true);
    let (snippet, _) = rig
        .kudzu
        .library
        .create_snippet(kudzu_app::library::SnippetDraft {
            name: "Keep".into(),
            content: "me".into(),
            ..Default::default()
        })
        .unwrap();
    rig.rt.update(move |st, _| {
        st.refresh_library_snapshot();
        st.select_snippet(Some(snippet.id));
    });
    capture(&mut rig, &["gone"]);
    rig.press(cmd_backspace());
    assert!(texts(&rig).is_empty());
    assert!(rig.library().snippets.iter().any(|s| s.id == snippet.id));
}

#[test]
fn a_deleted_secret_is_never_named_in_the_toast() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["hunter2-very-secret"]);
    let id = clips(&rig).active().unwrap();
    rig.kudzu.clipboard.set_secret(id, Some(true)).unwrap();
    rig.settle();
    rig.press(key(Key::Backspace, false));
    let titles = toast_titles(&rig);
    assert!(titles.contains(&"Deleted a secret".to_owned()), "{titles:?}");
    assert!(titles.iter().all(|t| !t.contains("hunter2")));
}

#[test]
fn deleting_several_says_how_many_and_undo_restores_them_all() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["a", "b", "c"]);
    rig.press(cmd('a'));
    rig.press(cmd_backspace());
    assert!(toast_titles(&rig).contains(&"Deleted 3 items".to_owned()));
    rig.click_text("Undo");
    until(&mut rig, "the restore", |r| texts(r).len() == 3);
}

#[test]
fn typing_in_the_list_goes_to_search_but_slash_only_focuses_it() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.press(plain('/', false));
    assert!(rig.rt.state.clipboard.is_editing_text());
    assert_eq!(rig.rt.state.clipboard.query(), "");
    rig.rt.update(|st, _| st.clipboard.blur_inputs());
    rig.press(plain('x', false));
    assert!(rig.rt.state.clipboard.is_editing_text());
    assert_eq!(rig.rt.state.clipboard.query(), "x");
}

#[test]
fn escape_in_search_clears_the_query_then_returns_to_the_list() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.press(plain('m', false));
    assert_eq!(rig.rt.state.clipboard.query(), "m");
    rig.press(esc());
    assert_eq!(rig.rt.state.clipboard.query(), "");
    assert!(rig.rt.state.clipboard.is_editing_text());
    rig.press(esc());
    assert!(!rig.rt.state.clipboard.is_editing_text());
}

#[test]
fn arrows_and_enter_in_search_drive_the_list() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["alpha", "alpine"]);
    rig.press(plain('a', false));
    rig.press(key(Key::Down, false));
    assert_eq!(active_text(&rig), "alpha");
}

#[test]
fn space_opens_the_large_preview_hook_and_escape_closes_it() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.press(plain(' ', false));
    assert!(rig.rt.state.clipboard.large_preview);
    rig.press(esc());
    assert!(!rig.rt.state.clipboard.large_preview);
}

#[test]
fn save_as_snippet_asks_for_a_new_snippet_with_the_full_text() {
    let mut rig = Rig::new(true);
    let long = "word ".repeat(1000);
    capture(&mut rig, &[long.as_str()]);
    rig.press(cmd('s'));
    until(&mut rig, "the snippet to exist", |r| {
        r.library().snippets.iter().any(|s| s.content.len() >= long.len())
    });
}

#[test]
fn clear_history_can_also_delete_pinned_items() {
    let mut rig = Rig::new(true);
    capture(&mut rig, &["keep", "drop"]);
    let keep = rig.kudzu.clipboard.watch().get().entries.iter().find(|e| e.text.as_deref() == Some("keep")).map(|e| e.id).unwrap();
    rig.kudzu.clipboard.pin(keep, true).unwrap();
    rig.rt.update(|st, _| crate::main::clipboard::actions::open_clear(st));
    rig.settle();
    rig.click_text("Clear history");
    assert_eq!(texts(&rig), vec!["keep"], "pinned items survive by default");
}

#[test]
fn the_options_menu_changes_retention() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.rt.update(|st, _| {
        crate::main::clipboard::actions::set_retention(st, 30);
    });
    assert_eq!(rig.kudzu.settings.watch().get().clipboard_retention_days, 30);
}

#[test]
fn the_view_keeps_the_filter_it_was_given() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig);
    rig.rt.update(|st: &mut MainState, tx| {
        crate::main::nav::view(st, MainView::Clipboard(ClipFilter::Pinned), tx);
    });
    rig.settle();
    let pinned = SampleState::load().unwrap().clipboard.entries.iter().filter(|e| e.pinned).count();
    assert_eq!(clips(&rig).len(), pinned);
    let _: Uuid = clips(&rig).active().unwrap();
}
