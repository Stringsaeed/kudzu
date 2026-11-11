//! Fixture tests for the clipboard colour card against `color.json` / `color-inputs.json`.

use std::path::PathBuf;

use kudzu_core::color::{self, ColorCard};
use serde::Deserialize;

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Deserialize)]
struct ColorCase {
    css: String,
    rgba: [f64; 4],
    result: ColorResult,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ColorResult {
    values: Vec<(String, String)>,
    on_white: String,
    on_black: String,
    light: bool,
}

#[derive(Deserialize)]
struct ColorInput {
    css: String,
    rgba: [f64; 4],
}

fn to8(rgba: [f64; 4]) -> [u8; 4] {
    rgba.map(|v| (v * 255.0).round() as u8)
}

#[test]
fn formats_match_every_color_fixture() {
    let cases: Vec<ColorCase> = load("color.json");
    assert_eq!(cases.len(), 9);
    for case in &cases {
        let got = color::formats(case.rgba);
        let want = ColorCard {
            values: case.result.values.clone(),
            on_white: case.result.on_white.clone(),
            on_black: case.result.on_black.clone(),
            light: case.result.light,
        };
        assert_eq!(got, want, "formats({})", case.css);
    }
}

#[test]
fn parse_matches_canvas_inputs() {
    let inputs: Vec<ColorInput> = load("color-inputs.json");
    assert_eq!(inputs.len(), 9);
    let mut mismatches = Vec::new();
    for input in &inputs {
        match color::parse(&input.css) {
            None => {
                if input.css != "invalid-color" {
                    mismatches.push(format!("{}: parse returned None", input.css));
                }
            }
            Some(got) => {
                if input.css == "invalid-color" {
                    mismatches.push(format!(
                        "invalid-color: parse succeeded with {got:?}, canvas fell back to black"
                    ));
                } else if to8(got) != to8(input.rgba) {
                    mismatches.push(format!(
                        "{}: parsed {:?} vs canvas {:?}",
                        input.css,
                        to8(got),
                        to8(input.rgba)
                    ));
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "csscolorparser vs canvas:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn card_matches_fixture_when_parse_agrees() {
    let cases: Vec<ColorCase> = load("color.json");
    for case in &cases {
        let Some(parsed) = color::parse(&case.css) else {
            assert_eq!(case.css, "invalid-color");
            continue;
        };
        if to8(parsed) != to8(case.rgba) {
            continue;
        }
        let got = color::card(&case.css).unwrap();
        assert_eq!(got.values, case.result.values, "card({})", case.css);
        assert_eq!(got.on_white, case.result.on_white, "card({})", case.css);
        assert_eq!(got.on_black, case.result.on_black, "card({})", case.css);
        assert_eq!(got.light, case.result.light, "card({})", case.css);
    }
}
