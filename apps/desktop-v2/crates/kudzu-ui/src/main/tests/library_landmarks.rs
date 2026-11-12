//! The library scenes' own landmarks against the S02 reference (`metrics.json`), within 2 pt,
//! in both themes. The four scenes stay "not yet built" in `cargo xtask parity` until S73,
//! because their references also hold the S42 sidebar and the S51 editor (orchestrator rule);
//! these tests cover the list column and the breadcrumb trail it decides meanwhile.
//!
//! A reference landmark is named by role, name and its occurrence on the whole page; the rig's
//! sidebar and editor are stubs, so each is found here by the key the list gives it.

use std::path::PathBuf;

use kudzu_app::settings::Theme;
use serde_json::Value;

use super::Rig;
use crate::main::library::{self, bulk, empty, scenes};
use crate::shell::MainState;

/// `(role, name, occurrence in the reference, key in the rig)`. A `None` key is a breadcrumb,
/// found by its label inside the trail.
type Owned = (&'static str, &'static str, usize, Option<&'static str>);

const COLUMN: &[Owned] = &[
    ("region", "Snippets", 2, Some("library-pane")),
    ("heading", "All snippets", 1, Some("library-heading")),
    ("button", "Sort", 1, Some(library::SORT_KEY)),
    ("button", "New snippet", 1, Some(library::NEW_KEY)),
    ("textbox", "Filter snippets", 1, Some(library::FILTER_KEY)),
    ("button", "Snippets", 1, None),
];

fn reference(scene: &str, theme: &str) -> Vec<(String, String, usize, [f32; 4])> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/migration/reference/metrics.json");
    let m: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let state = m["states"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["window"] == "main" && s["scene"] == scene && s["theme"] == theme)
        .unwrap_or_else(|| panic!("no reference for {scene} ({theme})"));
    state["landmarks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            let r: Vec<f32> = l[3]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            (
                l[0].as_str().unwrap().to_owned(),
                l[1].as_str().unwrap().to_owned(),
                l[2].as_u64().unwrap() as usize,
                [r[0], r[1], r[2], r[3]],
            )
        })
        .collect()
}

fn drawn(rig: &Rig, role: &str, name: &str, key: Option<&str>) -> [f32; 4] {
    let els = rig.rt.elements();
    match key {
        Some(key) => {
            els.iter()
                .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
                .unwrap_or_else(|| panic!("nothing keyed {key}"))
                .rect
        }
        None => {
            let trail = els
                .iter()
                .find(|e| e.key.as_deref() == Some("breadcrumb-trail"))
                .expect("the breadcrumb trail");
            els.iter()
                .find(|e| {
                    e.role == role
                        && e.label.as_deref() == Some(name)
                        && e.path.starts_with(&format!("{}/", trail.path))
                })
                .unwrap_or_else(|| panic!("no crumb {name}"))
                .rect
        }
    }
}

fn check(scene: &str, build: fn(&mut MainState), owned: &[Owned]) {
    for (theme, stored) in [("light", Theme::Light), ("dark", Theme::Dark)] {
        let mut rig = Rig::themed(true, stored);
        rig.rt.update(move |st, _| build(st));
        rig.settle();
        let refs = reference(scene, theme);
        // Owned crumbs are listed in trail order, from "Snippets".
        let mut trail_drift = 0.0;
        for &(role, name, occurrence, key) in owned {
            let want = refs
                .iter()
                .find(|(r, n, o, _)| r == role && n == name && *o == occurrence)
                .unwrap_or_else(|| panic!("{scene}: no reference {role} “{name}” #{occurrence}"))
                .3;
            let got = drawn(&rig, role, name, key);
            // A crumb starts where the one before it ends. Rune rounds a crumb's text width up
            // and snaps its box (up to 1.7 pt wider than WebKit's fractions), so a crumb's x is
            // the reference's plus the width differences of the crumbs before it, exactly.
            let crumb = key.is_none();
            let sides: &[usize] = if crumb { &[1, 2, 3] } else { &[0, 1, 2, 3] };
            let delta = sides
                .iter()
                .map(|&i| (want[i] - got[i]).abs())
                .fold(0.0, f32::max);
            if crumb {
                let drift = got[0] - want[0];
                assert!(
                    (drift - trail_drift).abs() <= 0.5,
                    "{scene} ({theme}) crumb “{name}” x: want {} + {trail_drift} from the \
                     crumbs before it, got {}",
                    want[0],
                    got[0]
                );
                trail_drift += got[2] - want[2];
            }
            assert!(
                delta <= 2.0,
                "{scene} ({theme}) {role} “{name}” #{occurrence}: want {want:?}, got {got:?}"
            );
        }
    }
}

#[test]
fn the_library_column_and_trail_match_the_reference() {
    let mut owned = COLUMN.to_vec();
    owned.push(("button", "Work", 1, None));
    owned.push(("button", "Email", 1, None));
    check("library", scenes::library, &owned);
}

#[test]
fn the_empty_library_matches_the_reference() {
    let mut owned = COLUMN.to_vec();
    owned.push(("button", "New snippet", 2, Some(empty::NEW_KEY)));
    check("library-empty", scenes::library_empty, &owned);
}

#[test]
fn the_bulk_bar_matches_the_reference() {
    let mut owned = COLUMN.to_vec();
    owned.extend([
        ("button", "Work", 1, None),
        ("button", "Favorite all", 1, Some(bulk::FAVORITE_KEY)),
        ("button", "Turn off all", 1, Some(bulk::TOGGLE_KEY)),
        ("button", "Move to group", 1, Some(bulk::MOVE_KEY)),
        ("button", "Delete selected", 1, Some(bulk::DELETE_KEY)),
    ]);
    check("snippet-bulk", scenes::snippet_bulk, &owned);
}

#[test]
fn the_deleted_toast_state_matches_the_reference() {
    let mut owned = COLUMN.to_vec();
    owned.push(("button", "Developer", 1, None));
    check(
        "snippet-deleted-toast",
        scenes::snippet_deleted_toast,
        &owned,
    );
}
