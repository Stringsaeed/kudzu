//! The S58 review fixes and the gaps the S58 report listed: opening links and files, the
//! large preview not trapping chords, the live region, `?`, glide, the screen's states.

use kudzu_core::clipboard::uti;
use rune::{Key, KeyEvent, Mods};

use super::clipboard_preview::{
    active, copy, elements_with, has_key, key, open_folder, seed_sample, text_clip, texts, until,
};
use super::{cmd, cmd_backspace, plain, Rig};
use crate::main::clipboard::actions;
use crate::main::state::{ClipFilter, MainView};
use crate::shell::{MainState, Route};

fn shift(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods {
            shift: true,
            ..Mods::default()
        },
    }
}

fn toast_titles(rig: &Rig) -> Vec<String> {
    rig.rt
        .state
        .frame
        .toasts
        .entries()
        .iter()
        .map(|e| e.options.title.clone())
        .collect()
}

fn file_clip(rig: &Rig, path: &str) {
    copy(
        rig,
        vec![(uti::FILE_URL, format!("file://{path}").into_bytes())],
    );
}

// MARK: M1 — opening links and files

#[test]
fn command_o_adds_https_to_a_www_link() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "www.example.com/docs");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(cmd('o'));
    assert_eq!(
        rig.rt.take_url_requests(),
        vec!["https://www.example.com/docs"]
    );
}

#[test]
fn command_o_opens_an_https_link_as_it_is() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "https://example.com/a?b=1");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(cmd('o'));
    assert_eq!(
        rig.rt.take_url_requests(),
        vec!["https://example.com/a?b=1"]
    );
}

#[test]
fn an_ftp_link_says_why_it_cannot_open() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "ftp://files.example.com/x.zip");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(cmd('o'));
    assert!(rig.rt.take_url_requests().is_empty());
    assert!(toast_titles(&rig).contains(&"Couldn't open that link".to_owned()));
    let described = rig.rt.state.frame.toasts.entries().iter().any(|e| {
        e.options
            .description
            .as_deref()
            .is_some_and(|d| d.contains("ftp"))
    });
    assert!(described, "the toast names the scheme");
}

#[test]
fn command_o_opens_and_command_r_reveals_a_copied_file() {
    let mut rig = Rig::new(true);
    file_clip(&rig, "/Users/me/Documents/report.pdf");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(cmd('o'));
    until(&mut rig, "the file to open", |r| {
        !r.fixtures.files.opened.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.files.opened.lock().unwrap()[0].to_string_lossy(),
        "/Users/me/Documents/report.pdf"
    );
    rig.press(cmd('r'));
    until(&mut rig, "Finder", |r| {
        !r.fixtures.files.revealed.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.files.revealed.lock().unwrap()[0].to_string_lossy(),
        "/Users/me/Documents/report.pdf"
    );
}

// MARK: M2 — the large preview does not trap chords

#[test]
fn chords_work_behind_the_large_preview() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "one");
    text_clip(&rig, "two");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(plain(' ', false));
    assert!(rig.rt.state.clipboard.large_preview);
    let id = active(&rig).id;
    rig.press(cmd('p'));
    assert!(rig.kudzu.clipboard.watch().get().get(id).unwrap().pinned);
    rig.press(cmd('c'));
    until(&mut rig, "the copy toast", |r| {
        toast_titles(r).contains(&"Copied".to_owned())
    });
    rig.press(cmd_backspace());
    assert_eq!(rig.kudzu.clipboard.watch().get().entries.len(), 1);
}

#[test]
fn the_large_preview_closes_when_nothing_is_left_to_show() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "only");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(plain(' ', false));
    rig.press(cmd_backspace());
    assert!(
        !rig.rt.state.clipboard.large_preview,
        "it left with its clip"
    );
    rig.press(plain('x', false));
    assert_eq!(
        rig.rt.state.clipboard.query(),
        "x",
        "the list takes keys again"
    );
}

#[test]
fn space_does_nothing_without_a_clip_to_show() {
    let mut rig = Rig::new(true);
    open_folder(&mut rig, ClipFilter::All);
    rig.press(plain(' ', false));
    assert!(!rig.rt.state.clipboard.large_preview);
}

// MARK: M3 — the live region

fn status_text(rig: &Rig) -> Option<String> {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.role == "status" && e.key.as_deref() == Some("clip-announcement"))
        .and_then(|e| e.label)
}

#[test]
fn pin_copy_and_clear_are_announced_in_a_status_region() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "a");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(cmd('p'));
    assert_eq!(
        status_text(&rig).as_deref(),
        Some("Pinned. It won't expire.")
    );
    rig.press(cmd('c'));
    until(&mut rig, "the announcement", |r| {
        status_text(r).as_deref() == Some("Copied to clipboard")
    });
    rig.rt
        .update(|st: &mut MainState, _| actions::open_clear(st));
    rig.rt.update(|st: &mut MainState, _| {
        if let Some(d) = &mut st.clipboard.clear {
            d.include_pinned = true;
        }
    });
    rig.rt
        .update(|st: &mut MainState, _| actions::confirm_clear(st));
    rig.settle();
    assert_eq!(status_text(&rig).as_deref(), Some("Cleared 1 items"));
}

// MARK: minors

#[test]
fn a_question_mark_on_the_list_opens_the_cheat_sheet_instead_of_typing() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "a");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(shift(Key::Text("?".into())));
    assert!(rig.rt.state.frame.shortcuts_open);
    assert_eq!(rig.rt.state.clipboard.query(), "");
}

#[test]
fn an_empty_kind_folder_shows_that_kinds_glyph_and_name() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "a");
    open_folder(&mut rig, ClipFilter::Image);
    assert!(has_key(&rig, "clip-nothing"));
    assert!(texts(&rig).iter().any(|t| t == "No images yet"));
    assert!(
        has_key(&rig, "clip-nothing-image"),
        "the glyph of the folder's kind"
    );
}

#[test]
fn rows_that_only_shift_glide_while_the_list_settles() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "one");
    text_clip(&rig, "two");
    text_clip(&rig, "three");
    open_folder(&mut rig, ClipFilter::All);
    let ids = rig.rt.state.clipboard.clips.list.order().to_vec();
    let bottom = ids[2];
    let y = |rig: &Rig| rig.rect(&format!("clip-{bottom}")).map(|r| r[1]);
    let before = y(&rig).unwrap();
    // Pinning the bottom clip moves the others down by one row.
    rig.rt
        .update(move |st: &mut MainState, _| st.clipboard.clips.list.activate(bottom));
    rig.rt
        .update(|st: &mut MainState, _| actions::toggle_pin(st));
    rig.rt.frame(0.016);
    // `ids[0]` is neither pinned nor new: it only shifts down under the Pinned header.
    let shifted = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&format!("clip-{}", ids[0])))
        .unwrap();
    assert!(
        shifted.moving,
        "a row that only shifts glides to its new place"
    );
    rig.settle();
    assert_ne!(y(&rig), Some(before));
}

// MARK: gaps the S58 report listed

#[test]
fn a_double_press_on_a_row_copies_it() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "first");
    text_clip(&rig, "second");
    open_folder(&mut rig, ClipFilter::All);
    let target = rig.rt.state.clipboard.clips.list.order()[1];
    let r = rig.rect(&format!("clip-{target}")).unwrap();
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    for _ in 0..2 {
        rig.rt.pointer_move(x, y);
        rig.rt.pointer_down(x, y);
        rig.rt.pointer_up(x, y);
    }
    until(&mut rig, "the copy toast", |r| {
        toast_titles(r).contains(&"Copied".to_owned())
    });
    assert_eq!(active(&rig).text.as_deref(), Some("first"));
}

#[test]
fn clear_history_with_the_box_ticked_deletes_pinned_items_too() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "keep");
    text_clip(&rig, "drop");
    open_folder(&mut rig, ClipFilter::All);
    rig.press(cmd('p'));
    rig.rt
        .update(|st: &mut MainState, _| actions::open_clear(st));
    rig.settle();
    rig.click_text("Also delete pinned items (1)");
    rig.click_text("Clear history");
    assert!(rig.kudzu.clipboard.watch().get().entries.is_empty());
}

#[test]
fn before_the_first_sync_the_list_is_blank_not_empty() {
    let mut rig = Rig::new(true);
    // Hold `poll` back so the view is built before the first sync, as the first frame after a
    // navigation is.
    let poll = rig.rt.poll.take();
    rig.rt.update(|st: &mut MainState, tx| {
        crate::main::nav::view(st, MainView::Clipboard(ClipFilter::All), tx);
    });
    rig.rt.frame(0.016);
    let tree_before_poll = (has_key(&rig, "clip-list"), has_key(&rig, "clip-welcome"));
    assert_eq!(
        tree_before_poll,
        (true, false),
        "the loading column shows neither"
    );
    rig.rt.poll = poll;
    rig.settle();
    assert!(has_key(&rig, "clip-welcome"));
}

#[test]
fn the_options_menu_lists_its_rows_and_the_retention_choices() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig, ClipFilter::All);
    rig.rt.update(|st: &mut MainState, _| {
        st.clipboard.options_open = true;
        st.clipboard.retention_open = true;
    });
    rig.settle();
    let all = texts(&rig);
    for row in [
        "Pause recording",
        "Keep history",
        "Clipboard settings…",
        "Clear history…",
        "1 day",
        "1 month",
        "3 months",
        "1 year",
        "Forever",
    ] {
        assert!(all.iter().any(|t| t == row), "missing “{row}” in {all:?}");
    }
}

#[test]
fn a_new_copy_arrives_with_a_transition_while_the_window_is_visible() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "old");
    open_folder(&mut rig, ClipFilter::All);
    text_clip(&rig, "brand new");
    rig.rt.frame(0.016);
    rig.rt.frame(0.016);
    let fresh = rig.rt.state.clipboard.clips.list.order()[0];
    let row = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(&format!("clip-{fresh}")))
        .unwrap();
    assert!(
        row.opacity < 1.0,
        "the arriving row is still fading in: {}",
        row.opacity
    );
    rig.settle();
}

#[test]
fn shift_enter_writes_only_plain_text_for_a_rich_clip_and_enter_keeps_the_formats() {
    let mut rig = Rig::new(true);
    copy(
        &rig,
        vec![
            (uti::PLAIN_TEXT, b"hello".to_vec()),
            (uti::HTML, b"<b>hello</b>".to_vec()),
        ],
    );
    open_folder(&mut rig, ClipFilter::All);
    let types = |rig: &Rig| -> Vec<String> {
        rig.fixtures
            .pasteboard
            .snapshot
            .lock()
            .unwrap()
            .items
            .iter()
            .flat_map(|item| item.iter().map(|(t, _)| t.clone()))
            .collect()
    };
    rig.press(shift(Key::Enter));
    until(&mut rig, "the plain copy", |r| {
        toast_titles(r).contains(&"Copied as plain text".to_owned())
    });
    assert!(
        types(&rig).iter().all(|t| t != uti::HTML),
        "{:?}",
        types(&rig)
    );
    assert!(types(&rig).iter().any(|t| t == uti::PLAIN_TEXT));
    rig.press(key(Key::Enter));
    until(&mut rig, "the formatted copy", |r| {
        types(r).iter().any(|t| t == uti::HTML)
    });
    let _ = (elements_with(&rig, "button", "Copy"), Route::Library);
}

#[test]
fn more_than_ten_changes_stay_still() {
    let mut rig = Rig::new(true);
    for i in 0..14 {
        text_clip(&rig, &format!("clip {i}"));
    }
    open_folder(&mut rig, ClipFilter::All);
    rig.settle();
    // Pin twelve at once: every other row moves, but a bulk change does not glide.
    let ids = rig.rt.state.clipboard.clips.list.order().to_vec();
    for id in &ids[..12] {
        rig.kudzu.clipboard.pin(*id, true).unwrap();
    }
    rig.rt.frame(0.016);
    let moving = rig
        .rt
        .elements()
        .iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("clip-")) && e.moving)
        .count();
    assert_eq!(moving, 0, "{moving} rows glided through a bulk change");
}
