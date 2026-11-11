//! Goldens for the gallery scenes: S06's three exact-metric primitives (button, switch, input),
//! one overlay on the kit's palette and elevation (dropdown-menu) and the icon set; S07's D6
//! select and four shared widgets (breadcrumb, selection pill, key hints, settings card), rendered
//! headless with the bundled fonts (`KUDZU_FONTS=bundled`). Regression tests for the kit
//! mapping, `widgets::kit_ext` and `widgets`, not parity evidence: `cargo xtask parity` is (PLAN §1.7).
//! `KUDZU_BLESS=1` rewrites them.

use std::path::{Path, PathBuf};
use std::process::Command;

use rune::text::pixel_compare::Image;

/// Same tolerances as the tokens goldens (`golden_gallery_tokens.rs`).
const CHANNEL_TOLERANCE: u8 = 2;
const CHANGED_SHARE: f64 = 0.0002;

/// Scene and window size in points: only as tall as the scene draws, to keep the PNGs small.
const SCENES: [(&str, &str); 10] = [
    ("button", "1160x540"),
    ("switch", "1160x260"),
    ("input", "1160x220"),
    ("dropdown-menu", "1160x340"),
    ("icons", "1160x600"),
    ("select", "1160x320"),
    ("breadcrumb", "1160x480"),
    ("selection-pill", "1160x260"),
    ("key-hints", "1160x160"),
    ("settings-card", "1160x420"),
];

fn golden(scene: &str, theme: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/gallery-{scene}.{theme}.png"))
}

fn check(scene: &str, size: &str, theme: &str) -> Option<String> {
    let out = std::env::temp_dir().join(format!(
        "kudzu-gallery-{scene}-{}-{theme}.png",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_kudzu"))
        .args([
            "--window", "gallery", "--scene", scene, "--theme", theme, "--size", size, "--snap",
        ])
        .arg(&out)
        .env("KUDZU_FONTS", "bundled")
        .status()
        .expect("run kudzu");
    assert!(status.success(), "kudzu --snap failed for {scene} {theme}");
    if std::env::var_os("KUDZU_BLESS").is_some() {
        std::fs::copy(&out, golden(scene, theme)).expect("bless golden");
    }
    let got = Image::read_png(&out).unwrap();
    let want = Image::read_png(&golden(scene, theme)).unwrap();
    let _ = std::fs::remove_file(&out);
    if (got.width, got.height) != (want.width, want.height) {
        return Some(format!("{scene} {theme}: size"));
    }
    let changed = got
        .rgb
        .chunks(3)
        .zip(want.rgb.chunks(3))
        .filter(|(a, b)| {
            a.iter()
                .zip(*b)
                .any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE)
        })
        .count();
    let share = changed as f64 / (got.width * got.height) as f64;
    (share > CHANGED_SHARE).then(|| {
        format!(
            "{scene} {theme}: {changed} pixels changed ({:.3}%)",
            share * 100.0
        )
    })
}

fn all(theme: &str) {
    let failures: Vec<String> = SCENES
        .iter()
        .filter_map(|(scene, size)| check(scene, size, theme))
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn gallery_components_light_match_their_goldens() {
    all("light");
}

#[test]
fn gallery_components_dark_match_their_goldens() {
    all("dark");
}
