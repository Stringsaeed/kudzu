//! Bundled-font goldens for `--window form --scene demo` (S57). Regression only;
//! `cargo xtask parity` is the SF Pro comparison against the Tauri reference.
//! `KUDZU_BLESS=1` rewrites them.
use std::path::{Path, PathBuf};
use std::process::Command;

use rune::text::pixel_compare::Image;

const CHANNEL_TOLERANCE: u8 = 2;
const CHANGED_SHARE: f64 = 0.0002;
const SIZE: &str = "440x360";

fn golden(theme: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/form-demo.{theme}.png"))
}

fn check(theme: &str) -> Option<String> {
    let out = std::env::temp_dir().join(format!(
        "kudzu-form-demo-{}-{theme}.png",
        std::process::id()
    ));
    let status = Command::new(env!("CARGO_BIN_EXE_kudzu"))
        .args([
            "--window", "form", "--scene", "demo", "--theme", theme, "--size", SIZE, "--snap",
        ])
        .arg(&out)
        .env("KUDZU_FONTS", "bundled")
        .status()
        .expect("run kudzu");
    assert!(
        status.success(),
        "kudzu --snap failed for form demo {theme}"
    );
    if std::env::var_os("KUDZU_BLESS").is_some() {
        std::fs::copy(&out, golden(theme)).expect("bless golden");
    }
    let got = Image::read_png(&out).unwrap();
    let want = Image::read_png(&golden(theme)).unwrap();
    let _ = std::fs::remove_file(&out);
    if (got.width, got.height) != (want.width, want.height) {
        return Some(format!("form demo {theme}: size"));
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
            "form demo {theme}: {changed} pixels changed ({:.3}%)",
            share * 100.0
        )
    })
}

#[test]
fn form_demo_light_matches_its_golden() {
    assert_eq!(check("light"), None);
}

#[test]
fn form_demo_dark_matches_its_golden() {
    assert_eq!(check("dark"), None);
}
