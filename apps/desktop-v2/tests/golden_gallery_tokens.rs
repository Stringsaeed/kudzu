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

fn swatch_centres(theme: &str, images: &[&Image]) {
    use kudzu_ui::theme::Theme;
    use serde_json::Value;
    fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
        if node["key"] == key {
            return Some(node);
        }
        node["children"]
            .as_array()?
            .iter()
            .find_map(|c| find(c, key))
    }
    let tree_path =
        std::env::temp_dir().join(format!("kudzu-s04-{}-{theme}.json", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_kudzu"))
        .args([
            "--window", "gallery", "--scene", "tokens", "--theme", theme, "--tree",
        ])
        .arg(&tree_path)
        .env("KUDZU_FONTS", "bundled")
        .output()
        .unwrap();
    assert!(output.status.success());
    let tree: Value = serde_json::from_slice(&std::fs::read(&tree_path).unwrap()).unwrap();
    std::fs::remove_file(tree_path).unwrap();
    let resolved = if theme == "dark" {
        Theme::dark()
    } else {
        Theme::light()
    };
    let background = resolved.c.background;
    let encode = rune::color::srgb_encode;
    for (index, (name, token)) in resolved.tokens.entries().iter().enumerate() {
        let bounds =
            &find(&tree["accessibility"], &format!("swatch-fill-{index}")).expect(name)["bounds"];
        let number = |i| bounds[i].as_f64().unwrap();
        let centre = (number(0) + number(2) / 2.0, number(1) + number(3) / 2.0);
        let color = token.color();
        let expected = [color.r, color.g, color.b]
            .into_iter()
            .zip([background.r, background.g, background.b])
            .map(|(c, b)| {
                ((encode(c) * color.a + encode(b) * (1.0 - color.a)) * 255.0).round() as u8
            })
            .collect::<Vec<_>>();
        for image in images {
            let scale = image.width as f64 / kudzu_ui::gallery::tokens::SIZE.0 as f64;
            let x = (centre.0 * scale) as usize;
            let y = (centre.1 * scale) as usize;
            let pixel = &image.rgb[(y * image.width as usize + x) * 3..][..3];
            assert!(
                pixel
                    .iter()
                    .zip(&expected)
                    .all(|(a, b)| a.abs_diff(*b) <= 1),
                "{theme} {name} centre ({x},{y}): {pixel:?} != {expected:?}"
            );
        }
    }
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
    swatch_centres(theme, &[&got, &want]);
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
