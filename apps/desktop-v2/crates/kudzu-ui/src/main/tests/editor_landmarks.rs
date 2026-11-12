//! S51 landmarks against the S02 reference (`metrics.json`), within 2 pt, in both themes, plus
//! the pane at the minimum window (900 × 560).
//!
//! `snippet-actions` (and `library`) show the whole window, and the content field's chips,
//! quick toolbar and format bar (S52, S54) are not built, so the scene stays "not yet built" in
//! `cargo xtask parity` (PLAN S73). These tests cover the editor's own region instead. Below the
//! content field every reference landmark is lower by exactly what the stand-in field lacks
//! (the reference's 276 pt content region against the stand-in's 198); the test asserts that
//! height and moves those landmarks up by it, so S52 and S54 drop the shift when they land.

use std::path::PathBuf;

use kudzu_app::settings::Theme as Stored;
use serde_json::Value;

use super::Rig;
use crate::main::editor::{self, behavior, content, header, preview, scenes, view};
use crate::main::scenes::SCENES;
use crate::theme::Theme;

/// "Expands to" (24) + gap (6) + the stand-in field's floor.
const STAND_IN_CONTENT_H: f32 = 24.0 + 6.0 + content::MIN_HEIGHT;
const TOLERANCE: f32 = 2.0;

/// `(role, name, key in the rig, below the content field)`. `None` finds the element by role
/// and name.
type Owned = (&'static str, &'static str, Option<&'static str>, bool);

const OWNED: &[Owned] = &[
    ("button", "Move to group", Some(header::GROUP_KEY), false),
    (
        "button",
        "Remove from favorites",
        Some(header::FAVORITE_KEY),
        false,
    ),
    ("switch", "Enabled", Some(header::ENABLED_KEY), false),
    ("button", "More actions", Some(header::MORE_KEY), false),
    ("textbox", "Snippet name", Some(editor::NAME_KEY), false),
    (
        "button",
        "Remove trigger ;sig",
        Some("editor-trigger-remove-;sig"),
        false,
    ),
    ("textbox", "Add trigger", Some(editor::TRIGGER_KEY), false),
    ("region", "Preview", Some(preview::SECTION_KEY), true),
    ("region", "Behavior", Some(behavior::SECTION_KEY), true),
    ("heading", "Behavior", None, true),
    ("radio", "Instantly", Some("editor-fire:immediate"), true),
    (
        "radio",
        "After the word",
        Some("editor-fire:wholeWord"),
        true,
    ),
];

fn reference(scene: &str, theme: &str) -> Vec<(String, String, [f32; 4])> {
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
        .filter(|l| l[2].as_u64() == Some(1))
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
                [r[0], r[1], r[2], r[3]],
            )
        })
        .collect()
}

/// A rig with the bundled fonts (Paper Mono for the chips), on `build`'s state.
fn rig(stored: Stored, build: fn(&mut crate::shell::MainState)) -> Rig {
    let mut rig = Rig::themed(true, stored);
    rig.rt.text =
        rune::text::TextSystem::new(crate::theme::fonts::FILES, crate::theme::fonts::families());
    rig.settle();
    rig.rt.update(move |s, _| build(s));
    rig.settle();
    rig
}

fn drawn(rig: &Rig, role: &str, name: &str, key: Option<&str>) -> [f32; 4] {
    rig.rt
        .elements()
        .into_iter()
        .find(|e| match key {
            Some(key) => e.key.as_deref() == Some(key) && !e.exiting,
            None => e.role == role && e.label.as_deref() == Some(name) && !e.exiting,
        })
        .unwrap_or_else(|| panic!("no {role} “{name}”"))
        .rect
}

fn check(scene: &str, build: fn(&mut crate::shell::MainState)) {
    for (stored, theme) in [(Stored::Light, "light"), (Stored::Dark, "dark")] {
        let rig = rig(stored, build);
        let refs = reference(scene, theme);
        let find = |role: &str, name: &str| {
            refs.iter()
                .find(|(r, n, _)| r == role && n == name)
                .unwrap_or_else(|| panic!("{scene}: no reference {role} “{name}”"))
                .2
        };
        let content_ref = find("region", "Content");
        let content = drawn(&rig, "region", "Content", Some(view::CONTENT_REGION_KEY));
        for i in 0..3 {
            assert!(
                (content[i] - content_ref[i]).abs() <= TOLERANCE,
                "{scene} ({theme}) Content: want {content_ref:?}, got {content:?}"
            );
        }
        assert_eq!(
            content[3], STAND_IN_CONTENT_H,
            "the stand-in field's height"
        );
        let shift = content_ref[3] - STAND_IN_CONTENT_H;
        for &(role, name, key, below) in OWNED {
            let mut want = find(role, name);
            if below {
                want[1] -= shift;
            }
            let got = drawn(&rig, role, name, key);
            let delta = (0..4).map(|i| (want[i] - got[i]).abs()).fold(0.0, f32::max);
            assert!(
                delta <= TOLERANCE,
                "{scene} ({theme}) {role} “{name}”: want {want:?}, got {got:?}"
            );
        }
    }
}

#[test]
fn the_editor_matches_the_library_reference_in_both_themes() {
    check("library", scenes::sample);
}

#[test]
fn the_editor_matches_the_snippet_actions_reference_in_both_themes() {
    check("snippet-actions", scenes::snippet_actions);
}

#[test]
fn snippet_actions_stays_deferred_until_its_regions_exist() {
    assert!(SCENES.iter().all(|(name, _)| *name != "snippet-actions"));
    let pending = include_str!("../../../../../xtask/src/parity/reference.rs");
    assert!(pending.contains("(\"main\", \"snippet-actions\", \"S51\")"));
}

#[test]
fn the_more_menu_lists_tauris_actions() {
    let rig = rig(Stored::Dark, scenes::snippet_actions);
    let texts: Vec<String> = rig
        .rt
        .elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect();
    for copy in [
        "Copy expanded snippet",
        "Duplicate",
        "Move to group",
        "Delete",
    ] {
        assert!(texts.iter().any(|t| t == copy), "menu item {copy}");
    }
}

#[test]
fn the_cards_use_the_surface_and_depth_tokens_in_both_themes() {
    for (stored, theme) in [
        (Stored::Light, Theme::light()),
        (Stored::Dark, Theme::dark()),
    ] {
        let rig = rig(stored, scenes::sample);
        let behavior = rig
            .rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some(behavior::SECTION_KEY))
            .unwrap();
        assert_eq!(behavior.fill, theme.c.surface);
        assert_eq!(behavior.radius, crate::theme::radius::XL);
        let card = rig
            .rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("editor-content-card"))
            .unwrap();
        assert_eq!(card.fill, theme.c.surface);
    }
}

/// The minimum window: the toolbar's controls stay inside the pane, the body scrolls, and the
/// fields keep a usable width.
#[test]
fn the_pane_fits_the_minimum_window() {
    let mut rig = rig(Stored::Light, scenes::sample);
    rig.rt.set_viewport((900.0, 560.0), 1.0);
    rig.settle();
    let pane = drawn(&rig, "", "", Some(editor::PANE_KEY));
    assert!(
        pane[0] + pane[2] <= 900.0 - 8.0 + 0.5,
        "the pane ends at the gutter: {pane:?}"
    );
    for key in [
        header::FAVORITE_KEY,
        header::ENABLED_KEY,
        header::MORE_KEY,
        header::GROUP_KEY,
    ] {
        let r = drawn(&rig, "", "", Some(key));
        assert!(
            r[0] >= pane[0] && r[0] + r[2] <= pane[0] + pane[2] + 0.5,
            "{key} {r:?} inside {pane:?}"
        );
    }
    let more = drawn(&rig, "", "", Some(header::MORE_KEY));
    assert!(
        (pane[0] + pane[2] - 16.0 - (more[0] + more[2])).abs() <= 1.0,
        "16 pt from the edge: {more:?} in {pane:?}, group {:?}",
        drawn(&rig, "", "", Some(header::GROUP_KEY))
    );
    let name = drawn(&rig, "", "", Some(editor::NAME_KEY));
    assert_eq!(name[0], pane[0] + 32.0);
    assert_eq!(
        name[2],
        pane[2] - 64.0,
        "the body is padded 32, not held at 760"
    );
    let body = drawn(&rig, "", "", Some(view::BODY_KEY));
    assert!(
        body[1] + body[3] <= 560.0 - 8.0 + 0.5,
        "the body ends inside the card"
    );
    let behavior = drawn(&rig, "", "", Some(behavior::SECTION_KEY));
    assert!(
        behavior[1] > body[1] + body[3],
        "behaviour is below the fold, reached by scrolling"
    );
}

/// With nothing selected the empty state is centred in the pane, at both sizes.
#[test]
fn the_empty_state_is_centred_at_both_sizes() {
    for size in [(1180.0, 760.0), (900.0, 560.0)] {
        let mut rig = rig(Stored::Dark, |s| {
            crate::main::library::scenes::library_empty(s);
        });
        rig.rt.set_viewport(size, 1.0);
        rig.settle();
        let pane = drawn(&rig, "", "", Some(editor::PANE_KEY));
        let button = drawn(&rig, "", "", Some(view::EMPTY_NEW_KEY));
        let centre = button[0] + button[2] / 2.0;
        assert!(
            (centre - (pane[0] + pane[2] / 2.0)).abs() <= 1.0,
            "{size:?}: {button:?} in {pane:?}"
        );
    }
}
