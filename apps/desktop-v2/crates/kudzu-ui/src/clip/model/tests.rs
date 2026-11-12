use super::*;
use chrono::TimeZone;
use kudzu_core::clipboard::{ClipKind, History};

pub(crate) fn now() -> DateTime<FixedOffset> {
    Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0)
        .unwrap()
        .fixed_offset()
}

/// A text clip copied `minutes` before [`now`].
pub(crate) fn clip(text: &str, minutes: i64, pinned: bool) -> ClipEntry {
    let at = now().with_timezone(&Utc) - chrono::Duration::minutes(minutes);
    ClipEntry {
        id: Uuid::new_v4(),
        kind: ClipKind::Text,
        text: Some(text.to_owned()),
        text_len: text.chars().count(),
        line_count: 1,
        files: vec![],
        image: None,
        color: None,
        rich: false,
        types: vec![],
        size: text.len() as u64,
        source: None,
        hash: text.to_owned(),
        first_copied_at: at,
        copied_at: at,
        copy_count: 1,
        timings: vec![at],
        last_used_at: None,
        use_count: 0,
        pinned,
        secret: None,
        secret_override: None,
    }
}

pub(crate) fn history(entries: Vec<ClipEntry>) -> Arc<History> {
    let mut h = History::new();
    h.entries = entries;
    Arc::new(h)
}

fn list(entries: Vec<ClipEntry>) -> ClipList {
    let h = history(entries);
    let all: Vec<usize> = (0..h.entries.len()).collect();
    let mut l = ClipList::default();
    l.set(
        h,
        &all,
        RowOptions {
            pinned_first: false,
            sections: true,
        },
        now(),
    );
    l
}

fn ten() -> ClipList {
    list(
        (0..10)
            .map(|i| clip(&format!("clip {i}"), i * 2, false))
            .collect(),
    )
}

#[test]
fn rows_group_by_day_and_only_clips_are_selectable() {
    let l = list(vec![
        clip("today", 5, false),
        clip("yesterday", 24 * 60 + 5, false),
    ]);
    assert_eq!(l.rows().len(), 4);
    assert!(matches!(l.rows()[0].kind, RowKind::Header { .. }));
    assert_eq!(l.len(), 2);
    assert_eq!(l.active(), Some(l.order()[0]));
}

#[test]
fn arrows_clamp_at_both_ends_and_keep_a_single_active_row() {
    let mut l = ten();
    l.move_by(-1, false);
    assert_eq!(l.active(), Some(l.order()[0]));
    l.move_by(1, false);
    assert_eq!(l.active(), Some(l.order()[1]));
    l.move_by(100, false);
    assert_eq!(l.active(), Some(l.order()[9]));
    assert!(!l.is_multi());
}

#[test]
fn page_moves_eight_and_home_end_jump_to_the_ends() {
    let mut l = ten();
    l.move_by(PAGE, false);
    assert_eq!(l.active(), Some(l.order()[8]));
    l.move_by(-(l.len() as isize), false);
    assert_eq!(l.active(), Some(l.order()[0]));
    l.move_by(l.len() as isize, false);
    assert_eq!(l.active(), Some(l.order()[9]));
}

#[test]
fn shift_extends_a_range_from_the_anchor_and_shrinks_it_back() {
    let mut l = ten();
    l.move_by(1, false);
    l.move_by(1, true);
    l.move_by(1, true);
    assert_eq!(l.selection(), l.order()[1..4].to_vec());
    l.move_by(-1, true);
    assert_eq!(l.selection(), l.order()[1..3].to_vec());
    l.move_by(1, false);
    assert!(!l.is_multi());
}

#[test]
fn command_click_toggles_and_includes_the_active_row() {
    let mut l = ten();
    let ids = l.order().to_vec();
    l.click(
        ids[3],
        Click {
            command: true,
            shift: false,
        },
    );
    assert_eq!(l.selection(), vec![ids[0], ids[3]]);
    l.click(
        ids[0],
        Click {
            command: true,
            shift: false,
        },
    );
    // One id left is not a multi-selection: the active row (the one just clicked) is the target.
    assert!(!l.is_multi());
    assert_eq!(l.selection(), vec![ids[0]]);
}

#[test]
fn shift_click_selects_from_the_anchor() {
    let mut l = ten();
    let ids = l.order().to_vec();
    l.click(ids[2], Click::default());
    l.click(
        ids[5],
        Click {
            command: false,
            shift: true,
        },
    );
    assert_eq!(l.selection(), ids[2..=5].to_vec());
    assert_eq!(l.active(), Some(ids[5]));
}

#[test]
fn select_all_then_a_plain_click_drops_the_selection() {
    let mut l = ten();
    l.select_all();
    assert_eq!(l.selection().len(), 10);
    let id = l.order()[4];
    l.click(id, Click::default());
    assert_eq!(l.selection(), vec![id]);
}

#[test]
fn a_vanished_active_row_hands_over_to_its_neighbour() {
    let entries: Vec<ClipEntry> = (0..5).map(|i| clip(&format!("c{i}"), i, false)).collect();
    let mut l = list(entries.clone());
    let ids = l.order().to_vec();
    l.click(ids[2], Click::default());
    let rest: Vec<ClipEntry> = entries.into_iter().filter(|e| e.id != ids[2]).collect();
    let h = history(rest);
    let all: Vec<usize> = (0..h.entries.len()).collect();
    l.set(
        h,
        &all,
        RowOptions {
            pinned_first: false,
            sections: true,
        },
        now(),
    );
    assert_eq!(
        l.active(),
        Some(ids[3]),
        "the row that moved into its place"
    );
}

#[test]
fn the_last_row_going_away_selects_the_new_last_row() {
    let entries: Vec<ClipEntry> = (0..3).map(|i| clip(&format!("c{i}"), i, false)).collect();
    let mut l = list(entries.clone());
    let ids = l.order().to_vec();
    l.move_by(2, false);
    let h = history(entries[..2].to_vec());
    l.set(
        h,
        &[0, 1],
        RowOptions {
            pinned_first: false,
            sections: true,
        },
        now(),
    );
    assert_eq!(l.active(), Some(ids[1]));
}

#[test]
fn multi_selected_ids_that_vanish_drop_out() {
    let entries: Vec<ClipEntry> = (0..4).map(|i| clip(&format!("c{i}"), i, false)).collect();
    let mut l = list(entries.clone());
    let ids = l.order().to_vec();
    l.select_all();
    let h = history(entries[..3].to_vec());
    l.set(
        h,
        &[0, 1, 2],
        RowOptions {
            pinned_first: false,
            sections: true,
        },
        now(),
    );
    assert_eq!(l.selection().len(), 3);
    assert!(!l.in_multi(ids[3]));
}

#[test]
fn pinned_first_puts_a_pinned_section_on_top_without_a_search() {
    let h = history(vec![clip("a", 1, false), clip("b", 100, true)]);
    let mut l = ClipList::default();
    l.set(
        h,
        &[0, 1],
        RowOptions {
            pinned_first: true,
            sections: true,
        },
        now(),
    );
    assert!(matches!(&l.rows()[0].kind, RowKind::Header { label, .. } if label == "Pinned"));
    assert_eq!(l.entry(l.order()[0]).unwrap().text.as_deref(), Some("b"));
}

#[test]
fn the_first_row_of_a_section_scrolls_with_its_header() {
    let l = list(vec![clip("a", 1, false), clip("b", 2, false)]);
    let first = l.order()[0];
    let second = l.order()[1];
    assert!(l.scroll_key(first).unwrap().starts_with("h-"));
    assert_eq!(l.scroll_key(second).unwrap(), second.to_string());
}

#[test]
fn the_version_changes_on_every_rebuild() {
    let mut l = ten();
    let before = l.version();
    let h = l.history().clone();
    l.set(
        h,
        &[0],
        RowOptions {
            pinned_first: false,
            sections: false,
        },
        now(),
    );
    assert_ne!(l.version(), before);
}
