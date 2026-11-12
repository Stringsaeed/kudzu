//! The S59 review fixes (`reviews/S59-clipboard-previews.md`): reveal, the 200-file preview,
//! the large preview's size, the cap tests, and the minors. Assertions are unconditional.

use std::time::Duration;

use super::clipboard_preview::{
    active, copy, has_key, has_text, open_folder, png, seed_sample, text_clip, texts, until,
};
use super::Rig;
use crate::clip::content::loader::MAX_WORKERS;
use crate::main::state::ClipFilter;
use crate::shell::MainState;
use kudzu_core::clipboard::facts::describe;
use kudzu_core::clipboard::uti;
use kudzu_core::clipboard::ClipKind;
use kudzu_core::system::AppInfo;

const SECRET: &str = "hunter2-correct-horse-battery";

fn secret_clip(rig: &mut Rig) -> uuid::Uuid {
    text_clip(rig, SECRET);
    open_folder(rig, ClipFilter::All);
    let id = active(rig).id;
    rig.kudzu.clipboard.set_secret(id, Some(true)).unwrap();
    rig.settle();
    id
}

fn element(rig: &Rig, key: &str) -> rune::ElementInfo {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("no element keyed `{key}`"))
}

fn readable(rig: &Rig, needle: &str) -> bool {
    rig.rt.accessibility_json().to_string().contains(needle)
}

fn frames(rig: &mut Rig) {
    for _ in 0..30 {
        rig.rt.frame(0.016);
    }
}

// MARK: M1 — a revealed secret is selectable and readable

#[test]
fn keyboard_focus_on_a_concealed_preview_makes_its_text_selectable_and_readable() {
    let mut rig = Rig::new(true);
    secret_clip(&mut rig);
    assert!(element(&rig, "clip-text").inert, "covered: not selectable");
    assert!(!readable(&rig, SECRET), "covered: silent");
    rig.rt
        .update(|_: &mut MainState, tx| tx.focus("clip-concealed", true));
    rig.settle();
    frames(&mut rig);
    let text = element(&rig, "clip-text");
    assert!(!text.inert, "revealed by focus: selectable");
    assert_eq!(
        text.text.as_deref(),
        Some(SECRET),
        "its name is the clip text"
    );
    assert!(readable(&rig, SECRET), "VoiceOver can read it");
    assert!(text.opacity > 0.95);
    rig.rt.update(|_: &mut MainState, tx| tx.blur());
    rig.settle();
    frames(&mut rig);
    assert!(element(&rig, "clip-text").inert, "covered again");
    assert!(!readable(&rig, SECRET));
}

#[test]
fn hovering_a_concealed_preview_uncovers_it_for_selection_and_assistive_clients() {
    let mut rig = Rig::new(true);
    secret_clip(&mut rig);
    let r = rig.rect("clip-concealed").unwrap();
    rig.rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.settle();
    frames(&mut rig);
    assert!(!element(&rig, "clip-text").inert);
    assert!(readable(&rig, SECRET));
    rig.rt.pointer_move(5.0, 5.0);
    rig.settle();
    frames(&mut rig);
    assert!(element(&rig, "clip-text").inert);
    assert!(!readable(&rig, SECRET));
}

#[test]
fn hovering_a_concealed_row_puts_its_title_in_the_tree_and_leaving_takes_it_out() {
    let mut rig = Rig::new(true);
    let id = secret_clip(&mut rig);
    let row_key = format!("clip-{id}");
    // The list is the left column: the covered preview on the right keeps its (silent) text.
    let in_list = |rig: &Rig| {
        rig.rt
            .elements()
            .into_iter()
            .filter(|e| e.rect[0] < 570.0)
            .filter_map(|e| e.text)
            .any(|t| t.contains("hunter2"))
    };
    assert!(!in_list(&rig), "a covered row title is not in the tree");
    assert!(!rig.rt.state.clipboard.clips.look.show_secrets);
    let r = rig.rect(&row_key).unwrap();
    rig.rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.settle();
    frames(&mut rig);
    assert!(in_list(&rig), "the title appears under the pointer");
    let title = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.rect[0] < 570.0 && e.text.as_deref().is_some_and(|t| t.contains("hunter2")))
        .unwrap();
    assert!(title.opacity > 0.95);
    assert!(
        readable(&rig, "hunter2"),
        "VoiceOver reads the hovered title"
    );
    let spoken_while_hovered = rig
        .rt
        .accessibility_json()
        .to_string()
        .matches("hunter2")
        .count();
    assert!(spoken_while_hovered >= 1);
    rig.rt.pointer_move(5.0, 5.0);
    rig.settle();
    frames(&mut rig);
    assert!(!in_list(&rig));
    assert!(!readable(&rig, "hunter2"));
}

#[test]
fn crossing_a_concealed_row_rebuilds_its_title_once_each_way() {
    let mut rig = Rig::new(true);
    let id = secret_clip(&mut rig);
    let r = rig.rect(&format!("clip-{id}")).unwrap();
    let before = crate::main::clipboard::VIEWS.get();
    rig.rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.settle();
    rig.rt.pointer_move(5.0, 5.0);
    rig.settle();
    assert!(
        crate::main::clipboard::VIEWS.get() - before <= 4,
        "enter and leave, not every frame"
    );
}

// MARK: M2 — a 200-file clip

fn file_items(n: usize) -> Vec<Vec<(String, Vec<u8>)>> {
    (0..n)
        .map(|i| {
            vec![(
                uti::FILE_URL.to_owned(),
                format!("file:///Users/me/Documents/file-{i}.txt").into_bytes(),
            )]
        })
        .collect()
}

#[test]
fn two_hundred_files_load_their_icons_a_few_at_a_time_and_keep_them() {
    let mut rig = Rig::new(true);
    for i in 0..200 {
        rig.fixtures
            .apps
            .icons
            .lock()
            .unwrap()
            .insert(format!("/Users/me/Documents/file-{i}.txt"), png(16, 16));
    }
    rig.fixtures.pasteboard.external_copy(file_items(200));
    rig.kudzu.clipboard.poll().unwrap();
    open_folder(&mut rig, ClipFilter::All);
    let e = active(&rig);
    assert_eq!((e.kind, e.files.len()), (ClipKind::File, 200));
    let id = e.id;
    let busiest = std::cell::Cell::new(0);
    until(&mut rig, "all 200 icons", |r| {
        let media = r.rt.state.clipboard.clips.media.as_ref().unwrap();
        busiest.set(busiest.get().max(media.in_flight()));
        (0..200).all(|i| {
            media
                .pixels(id, crate::clip::content::loader::Want::Icon(i))
                .is_some()
        })
    });
    assert!(
        busiest.get() <= MAX_WORKERS,
        "{} reads at once",
        busiest.get()
    );
    assert!(busiest.get() > 0, "the reads did run");
    assert!(
        has_key(&rig, "clip-file-icon-0"),
        "the rows on screen draw their icons"
    );
    assert!(has_key(&rig, "clip-file-icon-5"));

    // Pressure from other requests never evicts what the open clip shows.
    let service = rig.kudzu.clipboard.clone();
    rig.rt.update(move |st: &mut MainState, _| {
        let media = st.clipboard.clips.media.as_mut().unwrap();
        for _ in 0..700 {
            media.want(
                &service,
                uuid::Uuid::new_v4(),
                crate::clip::content::loader::Want::Thumbnail,
            );
        }
    });
    until(&mut rig, "the other requests", |r| {
        r.rt.state.clipboard.clips.media.as_ref().unwrap().idle()
    });
    let media = rig.rt.state.clipboard.clips.media.as_ref().unwrap();
    assert!(
        (0..200).all(|i| media
            .pixels(id, crate::clip::content::loader::Want::Icon(i))
            .is_some()),
        "the open clip's icons survive"
    );

    // Away and back: the icons are there again.
    text_clip(&rig, "another clip");
    rig.settle();
    rig.rt.update(move |st: &mut MainState, _| {
        let order = st.clipboard.clips.list.order().to_vec();
        st.clipboard.clips.list.activate(order[0]);
        crate::main::clipboard::track_active(st);
    });
    rig.settle();
    rig.rt.update(move |st: &mut MainState, _| {
        st.clipboard.clips.list.activate(id);
        crate::main::clipboard::track_active(st);
    });
    until(&mut rig, "the icons again", |r| {
        let media = r.rt.state.clipboard.clips.media.as_ref().unwrap();
        (0..200).all(|i| {
            media
                .pixels(id, crate::clip::content::loader::Want::Icon(i))
                .is_some()
        })
    });
    assert!(has_key(&rig, "clip-file-icon-0"));
}

// MARK: M3 — the large preview's size

fn open_large(rig: &mut Rig, window: (f32, f32)) {
    rig.rt.set_viewport(window, 1.0);
    rig.settle();
    rig.settle();
    rig.rt
        .update(|st: &mut MainState, _| st.clipboard.large_preview = true);
    rig.settle();
    rig.settle();
}

#[test]
fn the_large_preview_is_capped_by_the_window_not_the_card() {
    for (window, w, h) in [
        ((1180.0, 760.0), 920.0, 653.6),
        ((900.0, 560.0), 810.0, 481.6),
    ] {
        let mut rig = Rig::new(true);
        let long = "a long line of text that fills the large preview. ".repeat(400);
        text_clip(&rig, &long);
        open_folder(&mut rig, ClipFilter::All);
        until(&mut rig, "the full text", |r| {
            r.rt.state.clipboard.full.get().is_some()
        });
        open_large(&mut rig, window);
        let panel = element(&rig, "clip-large-dialog");
        assert!(
            (panel.rect[2] - w).abs() <= 1.0,
            "{window:?}: width {} not {w}",
            panel.rect[2]
        );
        assert!(
            (panel.rect[3] - h).abs() <= 1.0,
            "{window:?}: height {} not {h}",
            panel.rect[3]
        );
    }
}

#[test]
fn the_large_preview_has_no_visible_title_or_description() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "a short clip");
    open_folder(&mut rig, ClipFilter::All);
    open_large(&mut rig, (1180.0, 760.0));
    let spoken = |needle: &str| {
        rig.rt
            .elements()
            .into_iter()
            .filter(|e| e.text.as_deref().is_some_and(|t| t.contains(needle)))
            .collect::<Vec<_>>()
    };
    let description = spoken("Large preview. Press Space or Escape to close.");
    assert_eq!(description.len(), 1, "it is still there for VoiceOver");
    assert!(description[0].opacity == 0.0 && description[0].rect[2] <= 1.0);
    let title_texts: Vec<_> = rig
        .rt
        .elements()
        .into_iter()
        .filter(|e| e.role == "heading" && e.text.as_deref() == Some("a short clip"))
        .collect();
    assert_eq!(title_texts.len(), 1);
    assert!(title_texts[0].opacity == 0.0 || title_texts[0].rect[3] <= 1.0);
    assert!(readable(
        &rig,
        "Large preview. Press Space or Escape to close."
    ));
}

// MARK: M4 — the caps, unconditionally

#[test]
fn a_clip_of_230_files_lists_200_and_says_30_more() {
    let mut rig = Rig::new(true);
    rig.fixtures.pasteboard.external_copy(file_items(230));
    rig.kudzu.clipboard.poll().unwrap();
    open_folder(&mut rig, ClipFilter::All);
    let e = active(&rig);
    assert_eq!((e.kind, e.files.len()), (ClipKind::File, 230));
    assert!(has_text(&rig, "…and 30 more."));
    assert!(has_key(&rig, "clip-file-199"));
    assert!(!has_key(&rig, "clip-file-200"));
}

#[test]
fn a_1600_line_code_clip_draws_1500_numbered_lines_and_says_how_many_more() {
    let mut rig = Rig::new(true);
    let code: String = (0..1600)
        .map(|i| format!("    let value_{i} = compute({i}); // keep\n"))
        .collect();
    let code = format!("fn main() {{\n{code}}}\n");
    let lines = code.split('\n').count();
    text_clip(&rig, &code);
    open_folder(&mut rig, ClipFilter::All);
    until(&mut rig, "the full text", |r| {
        r.rt.state.clipboard.full.get().is_some()
    });
    assert_eq!(active(&rig).kind, ClipKind::Code);
    assert_eq!(
        lines, 1603,
        "1602 lines and the final newline's empty one, as Tauri's split"
    );
    assert!(has_text(&rig, "…and 103 more lines."));
    let gutter = texts(&rig)
        .into_iter()
        .find(|t| t.starts_with("1\n2\n3"))
        .expect("the line numbers");
    let numbers: Vec<&str> = gutter.lines().collect();
    assert_eq!(numbers.len(), 1500);
    assert!(numbers.contains(&"1") && numbers.contains(&"1500") && !numbers.contains(&"1501"));
}

// MARK: minors

#[test]
fn space_keeps_the_large_preview_open_while_text_is_selected() {
    let mut rig = Rig::new(true);
    text_clip(&rig, "select some of this text please");
    open_folder(&mut rig, ClipFilter::All);
    open_large(&mut rig, (1180.0, 760.0));
    // Two elements are keyed `clip-text`: the preview column's, behind the scrim, and the
    // large preview's, last in the tree.
    let r = rig
        .rt
        .elements()
        .into_iter()
        .rfind(|e| e.key.as_deref() == Some("clip-text"))
        .unwrap()
        .rect;
    let (y, x0, x1) = (r[1] + 8.0, r[0] + 2.0, r[0] + 80.0);
    rig.rt.pointer_move(x0, y);
    rig.rt.pointer_down(x0, y);
    rig.rt.pointer_move(x1, y);
    rig.rt.pointer_up(x1, y);
    rig.settle();
    assert!(
        rig.rt.elements().iter().any(|e| e.selected.is_some()),
        "text is selected"
    );
    assert!(
        rig.rt.state.clipboard.large_preview,
        "still open before Space"
    );
    rig.press(super::plain(' ', false));
    assert!(
        rig.rt.state.clipboard.large_preview,
        "Space inside a selection is not close"
    );
}

#[test]
fn a_timeline_dot_shows_the_styled_tooltip_with_the_exact_time() {
    let mut rig = Rig::new(true);
    for _ in 0..3 {
        text_clip(&rig, "again");
        rig.fixtures.clock.advance(Duration::from_secs(3600));
    }
    open_folder(&mut rig, ClipFilter::All);
    assert!(rig.rt.elements().iter().all(|e| e.role != "tooltip"));
    let dot = element(&rig, "clip-copy-1");
    assert!(dot.help.is_none(), "not the native tooltip");
    let r = dot.rect;
    rig.rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.settle();
    frames(&mut rig);
    let tooltip = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.role == "tooltip")
        .expect("the styled tooltip");
    assert!(tooltip
        .label
        .as_deref()
        .is_some_and(|l| l.starts_with("Today at ")));
    rig.rt.pointer_move(5.0, 5.0);
    rig.settle();
    frames(&mut rig);
    assert!(rig
        .rt
        .elements()
        .iter()
        .all(|e| e.role != "tooltip" || e.exiting));
}

fn image_file(rig: &Rig) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("kudzu-s59-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("picture.png");
    std::fs::write(&path, png(8, 4)).unwrap();
    *rig.fixtures.thumbnails.output.lock().unwrap() = Some(kudzu_app::ports::Thumbnail {
        png: png(8, 4),
        width: 8,
        height: 4,
    });
    path
}

fn copy_file(rig: &Rig, path: &std::path::Path) {
    rig.fixtures.pasteboard.external_copy(vec![vec![(
        uti::FILE_URL.to_owned(),
        format!("file://{}", path.display()).into_bytes(),
    )]]);
    rig.kudzu.clipboard.poll().unwrap();
}

#[test]
fn a_single_image_file_previews_itself_and_falls_back_to_its_thumbnail() {
    let mut rig = Rig::new(true);
    let path = image_file(&rig);
    copy_file(&rig, &path);
    open_folder(&mut rig, ClipFilter::All);
    let id = active(&rig).id;
    assert!(active(&rig).image.is_some());
    until(&mut rig, "the file's own picture", |r| {
        has_key(r, "clip-file-preview-image")
            && r.rt
                .state
                .clipboard
                .clips
                .media
                .as_ref()
                .unwrap()
                .pixels(id, crate::clip::content::loader::Want::File(0))
                .is_some()
    });

    // The file is gone before it is opened: its thumbnail stands in.
    let mut rig = Rig::new(true);
    let path = image_file(&rig);
    copy_file(&rig, &path);
    std::fs::remove_file(&path).unwrap();
    open_folder(&mut rig, ClipFilter::All);
    let id = active(&rig).id;
    until(&mut rig, "the thumbnail", |r| {
        let media = r.rt.state.clipboard.clips.media.as_ref().unwrap();
        media.missing(id, crate::clip::content::loader::Want::File(0))
            && has_key(r, "clip-file-preview-image")
    });
}

#[test]
fn pressing_copy_hex_value_copies_it_and_shows_the_check() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig, ClipFilter::Color);
    let hex = active(&rig).color.clone().unwrap().to_uppercase();
    let r = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.role == "button" && e.label.as_deref() == Some("Copy HEX value"))
        .unwrap()
        .rect;
    rig.rt.pointer_move(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.rt.pointer_down(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.rt.pointer_up(r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    rig.settle();
    assert_eq!(
        rig.rt
            .state
            .clipboard
            .copied_value
            .as_ref()
            .map(|(v, _)| v.as_str()),
        Some(hex.as_str())
    );
}

#[test]
fn a_link_preview_names_the_host_and_shows_the_url() {
    let mut rig = Rig::new(true);
    seed_sample(&mut rig, ClipFilter::Link);
    let e = active(&rig);
    let host = describe(&e).title;
    assert!(!host.is_empty());
    assert!(has_text(&rig, &host), "the host `{host}` is the title");
    assert!(
        has_text(&rig, e.text.as_deref().unwrap().trim()),
        "the full URL is in the well"
    );
}

#[test]
fn an_image_preview_draws_the_loaded_picture() {
    let mut rig = Rig::new(true);
    *rig.fixtures.thumbnails.output.lock().unwrap() = Some(kudzu_app::ports::Thumbnail {
        png: png(8, 4),
        width: 8,
        height: 4,
    });
    copy(&rig, vec![(uti::PNG, png(8, 4))]);
    open_folder(&mut rig, ClipFilter::All);
    until(&mut rig, "the picture", |r| has_key(r, "clip-image-pixels"));
}

#[test]
fn the_source_app_badge_shows_on_the_row_and_in_the_details_once_its_icon_has_loaded() {
    let mut rig = Rig::new(true);
    rig.fixtures
        .apps
        .icons
        .lock()
        .unwrap()
        .insert("com.test.Notes".into(), png(16, 16));
    *rig.fixtures.apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "com.test.Notes".into(),
        name: "Notes".into(),
        path: None,
        pid: None,
    });
    text_clip(&rig, "from the notes app");
    open_folder(&mut rig, ClipFilter::All);
    assert_eq!(
        active(&rig).source.as_ref().map(|s| s.name.as_str()),
        Some("Notes")
    );
    until(&mut rig, "both badges", |r| {
        has_key(r, "clip-app-badge") && has_key(r, "clip-tile-badge")
    });
}
