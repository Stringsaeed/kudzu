//! Independent S02 resolved channels, including canvas's quantized alpha samples.
use kudzu_ui::theme::{palette, Theme};
use serde_json::Value;

#[test]
fn palette_matches_chromium_srgb_reference() {
    let metrics: Value =
        serde_json::from_str(include_str!("../docs/migration/reference/metrics.json")).unwrap();
    for (appearance, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
        for (name, token) in theme.tokens.entries() {
            if name == "sidebar_vibrant" {
                continue;
            } // S02 captures the opaque sidebar.
            let entry = &metrics["tokens"][appearance][format!("--{}", name.replace('_', "-"))];
            let reference =
                &metrics["tables"]["colors"][entry["rgba"].as_u64().expect(name) as usize];
            let color = token.color();
            let actual = [color.r, color.g, color.b].map(rune::color::srgb_encode);
            let alpha = reference[3].as_f64().unwrap() as f32;
            // Chromium's mapping differs slightly for light chip-time. Binary-search
            // local MINDE gives (0,99,150), S02 has (0,99,152). The shipped Tauri app
            // renders through WebKit, which may differ again. Owner-approved exception:
            // only this token may differ by two 8-bit channel units; all others allow one.
            let tolerance = if appearance == "light" && name == "chip_time" {
                2.0
            } else {
                1.0
            };
            for (i, channel) in actual.iter().enumerate() {
                // Canvas stores premultiplied bytes. Unpremultiplication amplifies rounding
                // (e.g. dark border's white R becomes 245); compare stored channels instead.
                let expected = reference[i].as_f64().unwrap() as f32;
                assert!(
                    ((channel * 255.0).round() * color.a - expected * alpha).abs() <= tolerance,
                    "{appearance} {name} channel {i}: {} vs {expected}",
                    channel * 255.0
                );
            }
            assert!((color.a - alpha).abs() <= 1.0 / 255.0, "{name} alpha");
        }
    }
    // Ensure the fixture isn't accidentally empty or no longer a source/rgba pair.
    assert!(metrics["tokens"]["light"]["--chip-time"]["source"].is_number());
    assert_eq!(
        palette::LIGHT.entries().len(),
        palette::DARK.entries().len()
    );
}

#[test]
fn every_window_uses_the_css_window_constructor() {
    fn scan(dir: &std::path::Path) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                scan(&path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                if path.ends_with("theme/window.rs") {
                    continue;
                }
                for forbidden in [
                    "WindowSpec::from_app",
                    "WindowSpec::new",
                    "srgb_blend(",
                    "srgb_blend:",
                ] {
                    assert!(
                        !source.contains(forbidden),
                        "{} bypasses the CSS window invariant: {forbidden}",
                        path.display()
                    );
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let constructor =
        std::fs::read_to_string(root.join("crates/kudzu-ui/src/theme/window.rs")).unwrap();
    assert!(constructor.contains(".srgb_blend(true)"));
    assert!(!constructor.contains(".srgb_blend(false)"));
    scan(&root.join("src"));
    scan(&root.join("crates/kudzu-ui/src"));
}
