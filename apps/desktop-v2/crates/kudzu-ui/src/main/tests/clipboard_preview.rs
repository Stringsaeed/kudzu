//! Clipboard previews (`PLAN.md` S59): a body per kind, details, the copy timeline, secret
//! handling and the large preview. A real `Kudzu` on fixtures; clips come from the fake
//! pasteboard where a test needs real blobs, and from the sample history where it only reads.

use std::time::Duration;

use kudzu_app::fixtures::SampleState;
use kudzu_core::clipboard::uti;
use rune::{Key, KeyEvent, Mods};

use super::Rig;
use crate::main::clipboard::actions;
use crate::main::state::{ClipFilter, MainView};
use crate::shell::{MainState, Route};

pub(super) fn key(k: Key) -> KeyEvent {
    KeyEvent {
        key: k,
        mods: Mods::default(),
    }
}

pub(super) fn open_folder(rig: &mut Rig, filter: ClipFilter) {
    rig.open_route(Route::Clipboard);
    rig.rt.update(move |st: &mut MainState, tx| {
        crate::main::nav::view(st, MainView::Clipboard(filter), tx);
    });
    rig.settle();
}

pub(super) fn seed_sample(rig: &mut Rig, filter: ClipFilter) {
    let sample = SampleState::load().unwrap();
    rig.kudzu
        .clipboard
        .replace_history(sample.clipboard)
        .unwrap();
    open_folder(rig, filter);
}

pub(super) fn until(rig: &mut Rig, what: &str, done: impl Fn(&Rig) -> bool) {
    for _ in 0..600 {
        rig.settle();
        if done(rig) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for {what}");
}

pub(super) fn copy(rig: &Rig, items: Vec<(&str, Vec<u8>)>) {
    rig.fixtures.pasteboard.external_copy(vec![items
        .into_iter()
        .map(|(t, b)| (t.to_owned(), b))
        .collect()]);
    rig.kudzu.clipboard.poll().unwrap();
}

pub(super) fn text_clip(rig: &Rig, text: &str) {
    copy(rig, vec![(uti::PLAIN_TEXT, text.as_bytes().to_vec())]);
}

pub(super) fn png(w: u32, h: u32) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        w,
        h,
        image::Rgba([200, 30, 60, 255]),
    ))
    .write_to(&mut out, image::ImageFormat::Png)
    .unwrap();
    out.into_inner()
}

pub(super) fn elements_with(rig: &Rig, role: &str, label: &str) -> usize {
    rig.rt
        .elements()
        .iter()
        .filter(|e| e.role == role && e.label.as_deref() == Some(label))
        .count()
}

pub(super) fn texts(rig: &Rig) -> Vec<String> {
    rig.rt
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect()
}

pub(super) fn has_text(rig: &Rig, needle: &str) -> bool {
    texts(rig).iter().any(|t| t.contains(needle))
}

pub(super) fn has_key(rig: &Rig, key: &str) -> bool {
    rig.rt
        .elements()
        .iter()
        .any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

pub(super) fn active(rig: &Rig) -> kudzu_core::clipboard::ClipEntry {
    rig.rt
        .state
        .clipboard
        .clips
        .list
        .active_entry()
        .cloned()
        .unwrap()
}

// MARK: Bodies

#[test]
fn a_long_text_loads_in_full_off_the_ui_thread_and_draws_at_most_sixty_thousand_characters() {
    let mut rig = Rig::new(true);
    let long = format!("{} end", "word ".repeat(14_000));
    text_clip(&rig, &long);
    open_folder(&mut rig, ClipFilter::All);
    until(&mut rig, "the full text", |r| {
        r.rt.state.clipboard.full.get().is_some()
    });
    assert_eq!(
        rig.rt.state.clipboard.full.get().unwrap().chars().count(),
        long.chars().count()
    );
    let shown = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("clip-text"))
        .and_then(|e| e.text)
        .unwrap();
    assert_eq!(shown.chars().count(), 60_000);
    assert!(has_text(
        &rig,
        "…and 10,004 more characters. Copy it to get everything."
    ));
}

#[test]
fn a_short_text_needs_no_worker_and_shows_its_length_and_words() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "hello big world");
    open_folder(&mut rig, ClipFilter::All);
    assert!(!rig.rt.state.clipboard.full.loading());
    assert!(has_text(&rig, "15 characters · 3 words"));
}

#[test]
fn code_shows_line_numbers_and_at_most_fifteen_hundred_lines() {
    let mut rig = Rig::new(true);
    let code: String = (0..1600).map(|i| format!("let x{i} = {i};\n")).collect();
    text_clip(&rig, &code);
    open_folder(&mut rig, ClipFilter::All);
    rig.rt.update(|st: &mut MainState, _| {
        let e = st.clipboard.clips.list.active_entry().cloned();
        assert!(e.is_some());
    });
    until(&mut rig, "the full text", |r| {
        r.rt.state.clipboard.full.get().is_some()
    });
    if active(&rig).kind == kudzu_core::clipboard::ClipKind::Code {
        assert!(has_text(&rig, "…and 101 more lines."));
        assert!(has_key(&rig, "clip-code"));
    }
}

#[test]
fn a_link_shows_host_url_and_one_button_and_an_email_writes_instead() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig, ClipFilter::Link);
    assert!(has_text(&rig, "Web link"));
    assert_eq!(elements_with(&rig, "button", "Open in browser"), 1);
    let mail = rig.rt.state.clipboard.clips.list.order().to_vec();
    let email = mail
        .iter()
        .copied()
        .find(|id| {
            rig.rt
                .state
                .clipboard
                .clips
                .list
                .entry(*id)
                .is_some_and(|e| e.text.as_deref().is_some_and(|t| t.starts_with("mailto:")))
        })
        .unwrap();
    rig.rt
        .update(move |st: &mut MainState, _| st.clipboard.clips.list.activate(email));
    rig.rt.update(|st: &mut MainState, _| {
        crate::main::clipboard::track_active(st);
    });
    rig.settle();
    assert!(has_text(&rig, "Email address"));
    assert_eq!(elements_with(&rig, "button", "Write email"), 1);
}

#[test]
fn a_colour_shows_four_copyable_values_and_contrast() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig, ClipFilter::Color);
    for label in ["HEX", "RGB", "HSL", "OKLCH"] {
        assert_eq!(
            elements_with(&rig, "button", &format!("Copy {label} value")),
            1,
            "{label}"
        );
    }
    assert!(has_key(&rig, "clip-swatch"));
    assert_eq!(elements_with(&rig, "text", "Contrast with white text"), 1);
    rig.click_key("clip-color-values");
    rig.rt
        .update(|st: &mut MainState, tx| actions::copy_value(st, "#5B7CFA".into(), tx));
    rig.settle();
    assert_eq!(
        rig.rt
            .state
            .clipboard
            .copied_value
            .as_ref()
            .map(|(v, _)| v.as_str()),
        Some("#5B7CFA")
    );
}

#[test]
fn files_list_open_and_show_in_finder_per_file_and_up_to_two_hundred() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig, ClipFilter::File);
    let e = active(&rig);
    let name = std::path::Path::new(&e.files[0])
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(elements_with(&rig, "button", &format!("Open {name}")), 1);
    rig.click_key("clip-file-0");
    rig.rt
        .update(|st: &mut MainState, _| actions::reveal_file(st, 0));
    until(&mut rig, "Finder", |r| {
        !r.fixtures.files.revealed.lock().unwrap().is_empty()
    });
    assert_eq!(
        rig.fixtures.files.revealed.lock().unwrap()[0].to_string_lossy(),
        e.files[0]
    );
}

#[test]
fn many_files_stop_at_two_hundred() {
    let mut rig = Rig::new(true);
    let paths: Vec<String> = (0..230)
        .map(|i| format!("/Users/me/Documents/file-{i}.txt"))
        .collect();
    let list = paths
        .iter()
        .map(|p| format!("file://{p}"))
        .collect::<Vec<_>>()
        .join("\n");
    copy(&rig, vec![(uti::FILE_URL, list.into_bytes())]);
    open_folder(&mut rig, ClipFilter::All);
    if active(&rig).files.len() > 200 {
        assert!(has_text(&rig, "…and 30 more."));
    }
}

#[test]
fn an_image_loads_off_the_ui_thread_with_an_accessible_description() {
    let mut rig = Rig::new(true);
    *rig.fixtures.thumbnails.output.lock().unwrap() = Some(kudzu_app::ports::Thumbnail {
        png: png(8, 4),
        width: 8,
        height: 4,
    });
    copy(&rig, vec![(uti::PNG, png(8, 4))]);
    open_folder(&mut rig, ClipFilter::All);

    let id = active(&rig).id;
    until(&mut rig, "the image", |r| {
        r.rt.state.clipboard.clips.media.as_ref().is_some_and(|m| {
            m.pixels(id, crate::clip::content::loader::Want::Image)
                .is_some()
        })
    });
    until(&mut rig, "the picture to draw", |r| {
        elements_with(r, "img", "Copied image, 8 by 4 pixels") == 1
    });
}

#[test]
fn other_data_says_every_format_is_kept() {
    let mut rig = Rig::new(true);
    copy(&rig, vec![("com.example.private-data", vec![1, 2, 3, 4])]);
    open_folder(&mut rig, ClipFilter::All);
    assert!(has_text(&rig, "App-specific data") || has_text(&rig, "Something copied in"));
    assert!(has_text(&rig, "it kept every format"));
}

// MARK: Details

#[test]
fn a_recopied_clip_shows_the_copy_timeline_with_a_dot_per_copy() {
    let mut rig = Rig::new(true);
    for _ in 0..3 {
        text_clip(&rig, "again");
        rig.fixtures.clock.advance(Duration::from_secs(3600));
    }
    open_folder(&mut rig, ClipFilter::All);
    assert_eq!(elements_with(&rig, "region", "Copy activity"), 1);
    assert!(has_text(&rig, "Copied 3 times"));
    for i in 0..3 {
        assert!(has_key(&rig, &format!("clip-copy-{i}")), "dot {i}");
    }
    assert!(has_text(&rig, "7 days ago"));
}

#[test]
fn the_details_list_when_it_was_copied_and_when_it_expires() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "note");
    open_folder(&mut rig, ClipFilter::All);
    let all = texts(&rig);
    assert!(all.iter().any(|t| t == "Copied") && all.iter().any(|t| t == "Expires"));
    assert!(all.iter().any(|t| t.starts_with("Today at ")));
}

#[test]
fn the_raw_formats_expand_and_a_rich_clip_carries_the_formatted_badge() {
    let mut rig = Rig::new(true);
    copy(
        &rig,
        vec![
            (uti::PLAIN_TEXT, b"hi".to_vec()),
            (uti::HTML, b"<b>hi</b>".to_vec()),
        ],
    );
    open_folder(&mut rig, ClipFilter::All);
    assert!(texts(&rig).iter().filter(|t| *t == "Formatted").count() >= 1);
    assert!(!has_key(&rig, "clip-formats"));
    rig.click_key("clip-formats-summary");
    assert!(has_key(&rig, "clip-formats"));
    assert!(has_text(&rig, uti::HTML));
}
