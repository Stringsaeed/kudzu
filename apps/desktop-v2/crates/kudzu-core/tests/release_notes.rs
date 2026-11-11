//! Fixture tests for release-note clean-up against `release-notes.json`.

use std::path::PathBuf;

use kudzu_core::release_notes::{self, NotesSection};
use serde::Deserialize;

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Deserialize)]
struct NotesCase {
    input: Option<String>,
    result: Vec<NotesResult>,
}

#[derive(Deserialize)]
struct NotesResult {
    heading: Option<String>,
    items: Vec<String>,
}

#[test]
fn parse_matches_every_release_notes_fixture() {
    let cases: Vec<NotesCase> = load("release-notes.json");
    assert_eq!(cases.len(), 4);
    for (i, case) in cases.iter().enumerate() {
        let got = release_notes::parse(case.input.as_deref());
        let want: Vec<NotesSection> = case
            .result
            .iter()
            .map(|s| NotesSection {
                heading: s.heading.clone(),
                items: s.items.clone(),
            })
            .collect();
        assert_eq!(got, want, "release-notes case {i}");
    }
}
