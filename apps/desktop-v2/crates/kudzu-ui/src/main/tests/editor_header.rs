//! S51 toolbar (EDT-1, EDT-18): the group menu, Favorite with its burst (only when turning on,
//! and it ends), the Enabled switch and its label, and More actions (copy, duplicate, move,
//! delete). Every toggle flips the draft, which the library then holds.

use std::time::Duration;

use kudzu_app::error::PortError;
use kudzu_app::ports::Pasteboard;

use super::editor::{draft, group, has_text, id, open, rig_with, snippet, stored};
use super::Rig;
use crate::main::editor::{header, view};

fn wait_for_text(rig: &mut Rig, needle: &str) {
    for _ in 0..400 {
        rig.settle();
        if has_text(rig, needle) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("“{needle}” never showed");
}

fn grouped() -> Rig {
    let mut mine = snippet(1, "Sig", &[";s"], "Hello");
    mine.group_id = Some(id(20));
    rig_with(
        vec![mine, snippet(2, "Other", &[";o"], "x")],
        vec![group(10, "Work", None), group(20, "Email", Some(10))],
    )
}

#[test]
fn the_group_menu_shows_the_path_and_moves_the_draft() {
    let mut rig = grouped();
    assert!(has_text(&rig, "Work") && has_text(&rig, "Email"));
    rig.click_key(header::GROUP_KEY);
    assert!(rig.rt.state.editor.header.group_open);
    assert!(has_text(&rig, "Move to"));
    rig.click_text("No group");
    assert!(!rig.rt.state.editor.header.group_open);
    assert_eq!(draft(&rig).group_id, None);
    assert!(
        rig.rt.state.editor.is_dirty(),
        "through the draft, as Tauri's update"
    );
    assert!(has_text(&rig, "No group"));
    rig.rt.frame(0.4);
    rig.rt.frame(0.0);
    assert_eq!(stored(&rig, 1).group_id, None);
}

#[test]
fn favorite_flips_the_draft_and_bursts_only_when_turning_on() {
    let mut rig = grouped();
    assert!(
        !rig.rt.state.editor.header.star.running(),
        "opening stays still"
    );
    rig.click_key(header::FAVORITE_KEY);
    assert!(draft(&rig).is_favorite);
    assert!(rig.rt.state.editor.header.star.running());
    let labels: Vec<String> = rig
        .rt
        .elements()
        .into_iter()
        .filter_map(|e| e.label)
        .collect();
    assert!(labels.iter().any(|l| l == "Remove from favorites"));
    for _ in 0..5 {
        rig.rt.frame(0.1);
    }
    assert!(
        !rig.rt.state.editor.header.star.running(),
        "the burst ends after 420 ms"
    );
    assert!(stored(&rig, 1).is_favorite, "written");
    rig.click_key(header::FAVORITE_KEY);
    assert!(!draft(&rig).is_favorite);
    assert!(
        !rig.rt.state.editor.header.star.running(),
        "turning off stays still"
    );
    open(&mut rig, 2);
    open(&mut rig, 1);
    assert!(
        !rig.rt.state.editor.header.star.running(),
        "switching snippets stays still"
    );
}

#[test]
fn the_enabled_switch_flips_the_draft_and_swaps_its_label() {
    let mut rig = grouped();
    assert!(has_text(&rig, "Enabled"));
    rig.click_key(header::ENABLED_KEY);
    assert!(!draft(&rig).is_enabled);
    assert!(has_text(&rig, "Off"));
    assert_eq!(rig.rt.state.editor.header.enabled_swaps, 1);
    assert!(has_text(&rig, view::SNIPPET_OFF));
    rig.press(super::cmd('s'));
    assert!(!stored(&rig, 1).is_enabled);
}

#[test]
fn copy_expanded_snippet_copies_the_render_or_says_why_not() {
    let mut rig = grouped();
    rig.click_key(header::MORE_KEY);
    rig.click_text("Copy expanded snippet");
    wait_for_text(&mut rig, "Copied expanded snippet");
    assert_eq!(
        rig.fixtures.pasteboard.read_text().unwrap().as_deref(),
        Some("Hello")
    );

    *rig.fixtures.pasteboard.failure.lock().unwrap() = Some(PortError::Unavailable);
    rig.click_key(header::MORE_KEY);
    rig.click_text("Copy expanded snippet");
    wait_for_text(&mut rig, "Couldn't copy snippet");
    assert!(
        has_text(&rig, "This feature is unavailable."),
        "the port's sentence"
    );
}

#[test]
fn more_actions_duplicate_move_and_delete() {
    let mut rig = grouped();
    rig.click_key(header::MORE_KEY);
    rig.click_text("Duplicate");
    let copy = rig.rt.state.frame.selected_snippet.unwrap();
    assert_ne!(copy, id(1));
    assert_eq!(
        rig.focused_key().as_deref(),
        Some(crate::main::editor::NAME_KEY)
    );

    open(&mut rig, 1);
    rig.click_key(header::MORE_KEY);
    rig.click_text("Move to group");
    assert!(rig.rt.state.editor.header.move_open);
    let work = rig
        .rt
        .elements()
        .into_iter()
        .rev()
        .find(|e| e.text.as_deref() == Some("Work"))
        .map(|e| e.rect)
        .unwrap();
    let (x, y) = (work[0] + work[2] / 2.0, work[1] + work[3] / 2.0);
    rig.rt.pointer_move(x, y);
    rig.rt.pointer_down(x, y);
    rig.rt.pointer_up(x, y);
    rig.settle();
    assert_eq!(stored(&rig, 1).group_id, Some(id(10)));

    rig.click_key(header::MORE_KEY);
    rig.click_text("Delete");
    assert!(rig.library().snippets.iter().all(|s| s.id != id(1)));
    assert!(has_text(&rig, "Deleted “Sig”"));
}
