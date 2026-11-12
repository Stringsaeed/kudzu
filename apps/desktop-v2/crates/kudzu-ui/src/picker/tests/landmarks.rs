//! The picker's landmarks against the Tauri reference (`reference/metrics.json`), full rects,
//! in both themes: the dialog, the field, the six pills, the preview and the footer. The
//! scenes themselves are compared whole by `cargo xtask parity --window picker`.

use kudzu_app::settings::Theme;

use super::Rig;
use crate::picker::scenes;
use crate::shell::windows::PickerState;

type Landmark = (&'static str, &'static str, [f32; 4]);

const LANDMARKS: &[Landmark] = &[
    ("dialog", "Clipboard history", [12.0, 12.0, 776.0, 496.0]),
    (
        "combobox",
        "Search clipboard history",
        [58.0, 12.0, 386.891, 57.0],
    ),
    ("radio", "All", [458.891, 28.5, 34.219, 24.0]),
    ("radio", "Pinned", [497.109, 28.5, 58.297, 24.0]),
    ("radio", "Text", [559.406, 28.5, 43.484, 24.0]),
    ("radio", "Images", [606.891, 28.5, 60.234, 24.0]),
    ("radio", "Links", [671.125, 28.5, 49.094, 24.0]),
    ("radio", "Files", [724.219, 28.5, 45.781, 24.0]),
    ("complementary", "Preview", [400.0, 70.0, 388.0, 398.0]),
    ("contentinfo", "Copy", [12.0, 468.0, 776.0, 40.0]),
];

/// Pills and the field take their width from text: 4 pt. Everything else is structural: 2 pt.
fn tolerance(role: &str, side: usize) -> f32 {
    if side == 2 && matches!(role, "radio" | "combobox") {
        4.0
    } else {
        2.0
    }
}

fn check(scene: fn(&mut PickerState), name: &str) {
    for (theme, label) in [(Theme::Light, "light"), (Theme::Dark, "dark")] {
        let mut rig = Rig::themed(theme);
        rig.rt.update(move |s, _| scene(s));
        rig.settle();
        let els = rig.rt.elements();
        for &(role, label_text, rect) in LANDMARKS {
            let found = els
                .iter()
                .find(|e| e.role == role && e.label.as_deref() == Some(label_text))
                .unwrap_or_else(|| panic!("{name}/{label}: no {role} “{label_text}”"));
            for (i, side) in ["x", "y", "width", "height"].iter().enumerate() {
                assert!(
                    (found.rect[i] - rect[i]).abs() <= tolerance(role, i),
                    "{name}/{label}: {role} “{label_text}” {side} is {} but the reference has {}",
                    found.rect[i],
                    rect[i]
                );
            }
        }
    }
}

#[test]
fn history_matches_the_reference() {
    check(scenes::history, "history");
}

#[test]
fn clips_5000_matches_the_reference() {
    check(scenes::clips_5000, "clips-5000");
}

#[test]
fn empty_matches_the_reference() {
    check(scenes::empty, "empty");
}
