//! The clipboard screen's own landmarks against the Tauri reference (`reference/metrics.json`),
//! within 2 pt, in both themes. The comparator (`cargo xtask parity`) needs the whole window,
//! sidebar and previews included, so the clipboard scenes stay in its not-yet-built list until
//! S73; these tests cover the regions this step owns until then.

use kudzu_app::settings::Theme;

use super::Rig;
use crate::main::clipboard::scenes;
use crate::shell::MainState;

type Landmark = (&'static str, &'static str, usize, [f32; 4]);

/// Common to `clipboard-history`, `clips-5000`, `clipboard-empty` and `clipboard-no-match`.
const LIST_SIDE: &[Landmark] = &[
    (
        "region",
        "Clipboard history",
        1,
        [224.0, 50.0, 354.0, 702.0],
    ),
    ("heading", "Clipboard", 1, [244.0, 64.0, 198.578, 20.0]),
    (
        "button",
        "Recording copies. Press to pause.",
        1,
        [450.578, 62.0, 82.422, 24.0],
    ),
    ("button", "Clipboard options", 1, [541.0, 60.0, 28.0, 28.0]),
    (
        "textbox",
        "Search clipboard history",
        1,
        [236.0, 98.0, 329.0, 32.0],
    ),
    (
        "contentinfo",
        "Kept for 1 week · 37 MB",
        1,
        [224.0, 716.0, 353.0, 36.0],
    ),
    ("button", "Change", 1, [512.781, 724.5, 48.219, 20.0]),
    ("region", "Preview", 1, [578.0, 50.0, 594.0, 702.0]),
];

const ITEM_ACTIONS: &[Landmark] = &[
    ("button", "Copy", 1, [594.0, 710.5, 95.609, 28.0]),
    ("button", "As plain text", 1, [693.609, 710.5, 157.75, 28.0]),
    ("button", "Save as snippet", 1, [1032.0, 710.5, 28.0, 28.0]),
    ("button", "Hide as a secret", 1, [1064.0, 710.5, 28.0, 28.0]),
    ("button", "Unpin", 1, [1096.0, 710.5, 28.0, 28.0]),
    ("button", "Delete", 1, [1128.0, 710.5, 28.0, 28.0]),
    (
        "navigation",
        "Clipboard item breadcrumb",
        1,
        [602.0, 63.5, 126.688, 20.0],
    ),
];

const NO_MATCH: &[Landmark] = &[
    ("button", "Clear search", 1, [537.0, 104.0, 20.0, 20.0]),
    (
        "button",
        "Show everything",
        1,
        [338.609, 282.0, 123.781, 28.0],
    ),
];

const BULK: &[Landmark] = &[
    ("button", "Unpin", 1, [503.0, 710.0, 28.0, 28.0]),
    ("button", "Delete", 1, [535.0, 710.0, 28.0, 28.0]),
    ("button", "Unpin", 2, [1096.0, 710.5, 28.0, 28.0]),
    ("button", "Delete", 2, [1128.0, 710.5, 28.0, 28.0]),
];

fn check(rig: &Rig, scene: &str, theme: &str, want: &[Landmark]) {
    let els = rig.rt.elements();
    for &(role, name, occurrence, rect) in want {
        let found = els
            .iter()
            .filter(|e| e.role == role && e.label.as_deref() == Some(name))
            .nth(occurrence - 1);
        let Some(e) = found else {
            panic!("{scene}/{theme}: no {role} “{name}” #{occurrence}");
        };
        for (i, side) in ["x", "y", "width", "height"].iter().enumerate() {
            // Positions and heights hold to 2 pt. A width that comes from a label's text also
            // depends on the font (the reference was measured in SF Pro, tests run bundled
            // Figtree), so widths get 4 pt.
            let tolerance = if i == 2 { 4.0 } else { 2.0 };
            assert!(
                (e.rect[i] - rect[i]).abs() <= tolerance,
                "{scene}/{theme}: {role} “{name}” {side} is {} but the reference has {}",
                e.rect[i],
                rect[i]
            );
        }
    }
}

fn scene(theme: Theme, run: fn(&mut MainState)) -> Rig {
    let mut rig = Rig::themed(true, theme);
    rig.rt.update(move |st, _| run(st));
    rig.settle();
    rig
}

fn both(name: &str, run: fn(&mut MainState), groups: &[&[Landmark]]) {
    for (theme, label) in [(Theme::Light, "light"), (Theme::Dark, "dark")] {
        let rig = scene(theme, run);
        for want in groups {
            check(&rig, name, label, want);
        }
    }
}

#[test]
fn clipboard_history_list_side_and_item_actions_match_the_reference() {
    both(
        "clipboard-history",
        scenes::history,
        &[LIST_SIDE, ITEM_ACTIONS],
    );
}

#[test]
fn clips_5000_lays_out_like_the_short_history() {
    both("clips-5000", scenes::clips_5000, &[LIST_SIDE, ITEM_ACTIONS]);
}

#[test]
fn clipboard_options_menu_scene_keeps_the_history_layout() {
    both("clipboard-options-menu", scenes::options_menu, &[LIST_SIDE]);
}

#[test]
fn clipboard_empty_keeps_the_header_search_and_footer() {
    both("clipboard-empty", scenes::empty, &[LIST_SIDE]);
}

#[test]
fn clipboard_no_match_adds_clear_search_and_show_everything() {
    both(
        "clipboard-no-match",
        scenes::no_match,
        &[&LIST_SIDE[..5], NO_MATCH],
    );
}

#[test]
fn clipboard_bulk_replaces_the_footer_with_the_selection_bar() {
    both("clipboard-bulk", scenes::bulk, &[&LIST_SIDE[..5], BULK]);
}
