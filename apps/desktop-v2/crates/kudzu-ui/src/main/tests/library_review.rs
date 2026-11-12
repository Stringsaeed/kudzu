//! The S50 review (`docs/migration/reviews/S50-library.md`): list motion that ends with its
//! window (M1), Back and Forward keeping the bulk bar honest (minor 3), and the row metrics the
//! first pass left unasserted (the text inset, the 8 pt bottom padding, the summary text).

use std::collections::HashMap;
use std::path::PathBuf;

use kudzu_app::library::SnippetDraft;

use super::library_list::{bar_count, focus_list, id, sample_rig};
use super::{cmd, Rig};
use crate::main::library::{self, bulk};

/// Presses where `needle` is drawn, without settling: the next frame still shows any motion.
fn tap_text(rig: &mut Rig, needle: &str) {
    let r = rig
        .rt
        .elements()
        .into_iter()
        .find(|e| e.text.as_deref() == Some(needle) && !e.exiting)
        .unwrap_or_else(|| panic!("nothing reads {needle}"))
        .rect;
    let (x, y) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
}

/// The rows still travelling to their places.
fn gliding(rig: &Rig) -> Vec<String> {
    rig.rt
        .elements()
        .into_iter()
        .filter(|e| e.role == "option" && !e.exiting && e.moving)
        .filter_map(|e| e.label)
        .collect()
}

fn row_opacity(rig: &Rig, name: &str) -> Option<f32> {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| e.role == "option" && e.label.as_deref() == Some(name) && !e.exiting)
        .map(|e| e.opacity)
}

fn create(rig: &Rig, name: &str) {
    rig.kudzu
        .library
        .create_snippet(SnippetDraft {
            name: name.into(),
            ..SnippetDraft::default()
        })
        .unwrap();
}

/// M1: starring adds and removes nothing, so the next sort lands every row at once.
#[test]
fn starring_then_sorting_lands_rows_at_once() {
    let mut rig = sample_rig();
    let mut bug = rig
        .library()
        .snippets
        .iter()
        .find(|s| s.name == "Bug report")
        .unwrap()
        .clone();
    bug.is_favorite = true;
    rig.kudzu.library.update_snippet(bug).unwrap();
    rig.settle();
    assert!(
        rig.rt.state.library.motion().still,
        "a star adds and removes nothing"
    );
    rig.click_key(library::SORT_KEY);
    tap_text(&mut rig, "Most used");
    rig.rt.frame(0.016);
    assert_eq!(rig.rt.state.library.sort, library::model::Sort::Used);
    assert_eq!(
        gliding(&rig),
        Vec::<String>::new(),
        "rows glide after a star"
    );
}

/// M1: a new row rises once; filtering it out and clearing the filter after the motion window
/// mounts it again without a second rise.
#[test]
fn a_row_cleared_back_by_the_filter_does_not_rise_again() {
    let mut rig = sample_rig();
    create(&rig, "Brand new");
    rig.rt.frame(0.016);
    assert!(
        row_opacity(&rig, "Brand new").is_some_and(|o| o < 1.0),
        "it rises in"
    );
    rig.rt.frame(0.5);
    rig.settle();
    let motion = rig.rt.state.library.motion();
    assert!(
        motion.fresh.is_empty() && motion.still,
        "the motion window ended"
    );
    rig.press(cmd('f'));
    for ch in "zzq".chars() {
        rig.press(super::plain(ch, false));
    }
    assert_eq!(row_opacity(&rig, "Brand new"), None, "filtered out");
    // The empty state's Clear filter: the whole list mounts again.
    let r = rig.rect(library::empty::CLEAR_KEY).unwrap();
    let (x, y) = (r[0] + r[2] * 0.5, r[1] + r[3] * 0.5);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
    rig.rt.frame(0.016);
    assert_eq!(rig.rt.state.library.filter.text, "");
    assert_eq!(
        row_opacity(&rig, "Brand new"),
        Some(1.0),
        "a cleared filter remounts it without a second rise"
    );
}

/// M1: within its window a small add still rises and the rows below glide; once the window
/// ends the list is still again.
#[test]
fn a_small_add_rises_and_glides_only_within_its_window() {
    let mut rig = sample_rig();
    create(&rig, "Calendar invite");
    rig.rt.frame(0.016);
    assert!(row_opacity(&rig, "Calendar invite").is_some_and(|o| o < 1.0));
    assert!(!gliding(&rig).is_empty(), "the rows below make room for it");
    // Let the rise and the glide play out, past the window.
    for _ in 0..60 {
        rig.rt.frame(1.0 / 60.0);
    }
    assert!(rig.rt.state.library.motion().still, "the window ended");
    assert!(gliding(&rig).is_empty(), "the add's own motion is over");
    rig.click_key(library::SORT_KEY);
    tap_text(&mut rig, "Most used");
    rig.rt.frame(0.016);
    assert_eq!(gliding(&rig), Vec::<String>::new(), "the window is over");
}

/// Minor 3: ⌘[ leaves Favorites for the view before it, and the bulk bar of the old view's
/// selection is gone in the first frame, not one frame later.
#[test]
fn back_with_a_multi_selection_paints_no_stale_bar() {
    let mut rig = sample_rig();
    rig.press(cmd('2'));
    focus_list(&mut rig);
    rig.press(cmd('a'));
    assert_eq!(bar_count(&rig).as_deref(), Some("3"));
    rig.rt.key(cmd('['));
    rig.rt.frame(0.0);
    assert!(
        rig.rt.state.library.selection.bulk().is_empty(),
        "Back drops the multi-selection"
    );
    let stale = rig
        .rt
        .elements()
        .into_iter()
        .any(|e| e.key.as_deref() == Some(bulk::BAR_KEY) && !e.exiting);
    assert!(!stale, "the old view's bar painted after ⌘[");
}

/// The item is `border` (1 pt, transparent) plus `px-3` (12 pt), so its content starts 13 pt
/// in, as the reference PNG shows (first ink 14 pt in, after the glyph's side bearing).
#[test]
fn row_text_starts_thirteen_points_in() {
    let rig = sample_rig();
    let els = rig.rt.elements();
    let row = els
        .iter()
        .find(|e| e.role == "option" && e.label.as_deref() == Some("Bug report"))
        .unwrap();
    let name = els
        .iter()
        .find(|e| e.text.as_deref() == Some("Bug report") && e.path.starts_with(&row.path))
        .unwrap();
    assert_eq!(name.rect[0] - row.rect[0], 13.0);
}

/// LIB-3: 4 pt above the first row and 8 pt below the last (`paddingStart`, `paddingEnd`).
#[test]
fn the_rows_have_four_points_above_and_eight_below() {
    let rig = sample_rig();
    let rows = rig.rect("library-rows").unwrap();
    assert_eq!(rows[3], 4.0 + 18.0 * 64.0 + 8.0);
    let last = rig.rect(&library::row_key(id(&rig, "Today"))).unwrap();
    assert_eq!(rows[1] + rows[3] - (last[1] + last[3]), 8.0);
}

/// LIB-3: a row's summary is the Tauri app's `plainSummary` of its content: the S02 recording,
/// for each sample snippet that names no other snippet (the recording looked names up as
/// `undefined`; rows name them).
#[test]
fn row_summaries_match_the_recorded_plain_summaries() {
    let rig = sample_rig();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../kudzu-core/tests/fixtures/ts-parity/plain-summaries.json");
    let recorded: HashMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let els = rig.rt.elements();
    let mut checked = 0;
    for snippet in rig
        .library()
        .snippets
        .iter()
        .filter(|s| !s.content.contains("{{snippet"))
    {
        let want = &recorded[&snippet.content];
        let row = els
            .iter()
            .find(|e| e.role == "option" && e.label.as_deref() == Some(snippet.name.as_str()))
            .unwrap_or_else(|| panic!("no row for {}", snippet.name));
        let shown = els.iter().any(|e| {
            e.path.starts_with(&format!("{}/", row.path)) && e.text.as_deref() == Some(want)
        });
        assert!(shown, "{}: the row doesn't show “{want}”", snippet.name);
        checked += 1;
    }
    assert!(checked >= 15, "checked {checked} rows");
}
