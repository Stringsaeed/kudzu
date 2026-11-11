//! Goldens for the Quick Search palette, rendered headless with the bundled fonts
//! (`KUDZU_FONTS=bundled`) so they do not depend on the Mac's SF Pro. Regression tests
//! for the palette scenes, not parity evidence (PLAN §1.7). `KUDZU_BLESS=1` rewrites them.

use std::path::{Path, PathBuf};
use std::process::Command;

use rune::text::pixel_compare::Image;

/// A pixel counts as changed when a channel moves by more than this.
const CHANNEL_TOLERANCE: u8 = 2;
/// The share of changed pixels allowed (0.02%).
const CHANGED_SHARE: f64 = 0.0002;

fn golden(scene: &str, theme: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/palette-{scene}.{theme}.png"))
}

fn render(scene: &str, theme: &str, out: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_kudzu"))
        .args([
            "--window", "palette", "--scene", scene, "--theme", theme, "--size", "760x480",
            "--snap",
        ])
        .arg(out)
        .env("KUDZU_FONTS", "bundled")
        .status()
        .expect("run kudzu");
    assert!(
        status.success(),
        "kudzu --snap failed for palette/{scene} {theme}"
    );
}

fn check(scene: &str, theme: &str) {
    let out = std::env::temp_dir().join(format!(
        "kudzu-palette-{scene}-{}-{theme}.png",
        std::process::id()
    ));
    render(scene, theme, &out);
    if std::env::var_os("KUDZU_BLESS").is_some() {
        std::fs::copy(&out, golden(scene, theme)).expect("bless golden");
    }
    let (got, want) = (
        Image::read_png(&out).unwrap(),
        Image::read_png(&golden(scene, theme)).unwrap(),
    );
    let _ = std::fs::remove_file(&out);
    assert_eq!(
        (got.width, got.height),
        (want.width, want.height),
        "palette/{scene} {theme}: size"
    );
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
    assert!(
        share <= CHANGED_SHARE,
        "palette/{scene} {theme}: {changed} pixels changed ({:.3}%)",
        share * 100.0
    );
}

#[test]
fn palette_search_dark_matches_its_golden() {
    check("search", "dark");
}

#[test]
fn palette_search_light_matches_its_golden() {
    check("search", "light");
}

#[test]
fn palette_no_match_dark_matches_its_golden() {
    check("no-match", "dark");
}

#[test]
fn palette_no_match_light_matches_its_golden() {
    check("no-match", "light");
}
