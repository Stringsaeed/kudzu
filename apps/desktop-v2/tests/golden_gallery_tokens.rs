//! Goldens for the gallery's `tokens` scene, rendered headless with the bundled fonts
//! (`KUDZU_FONTS=bundled`) so they do not depend on the Mac's SF Pro. Regression tests for the
//! theme tokens, not parity evidence (PLAN §1.7). `KUDZU_BLESS=1` rewrites them.

use std::path::{Path, PathBuf};
use std::process::Command;

use rune::text::pixel_compare::Image;

/// A pixel counts as changed when a channel moves by more than this. Renders are byte-identical
/// on one Mac; the slack is for GPU rounding on another. Kept tight: a token change moves few
/// pixels by little (a dark shadow's alpha .55 → .95 moves 2,106 pixels by more than 2).
const CHANNEL_TOLERANCE: u8 = 2;
/// The share of changed pixels allowed (0.02%, about 800 of 4.2 million).
const CHANGED_SHARE: f64 = 0.0002;

fn golden(theme: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/gallery-tokens.{theme}.png"))
}

fn render(theme: &str, out: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_kudzu"))
        .args([
            "--window", "gallery", "--scene", "tokens", "--theme", theme, "--snap",
        ])
        .arg(out)
        .env("KUDZU_FONTS", "bundled")
        .status()
        .expect("run kudzu");
    assert!(status.success(), "kudzu --snap failed for {theme}");
}

fn check(theme: &str) {
    let out = std::env::temp_dir().join(format!(
        "kudzu-gallery-tokens-{}-{theme}.png",
        std::process::id()
    ));
    render(theme, &out);
    if std::env::var_os("KUDZU_BLESS").is_some() {
        std::fs::copy(&out, golden(theme)).expect("bless golden");
    }
    let (got, want) = (
        Image::read_png(&out).unwrap(),
        Image::read_png(&golden(theme)).unwrap(),
    );
    let _ = std::fs::remove_file(&out);
    assert_eq!(
        (got.width, got.height),
        (want.width, want.height),
        "{theme}: size"
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
        "{theme}: {changed} pixels changed ({:.3}%)",
        share * 100.0
    );
}

#[test]
fn gallery_tokens_dark_matches_its_golden() {
    check("dark");
}

#[test]
fn gallery_tokens_light_matches_its_golden() {
    check("light");
}
