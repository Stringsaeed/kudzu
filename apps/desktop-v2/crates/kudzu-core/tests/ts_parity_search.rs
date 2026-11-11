//! Search parity with the Tauri app's `lib/search.ts`.
//!
//! The fixtures under `tests/fixtures/ts-parity/` were recorded from that TypeScript at a fixed
//! clock (2026-11-11T12:00:00Z) over the sample library (18 snippets) and a seeded 5,000-snippet
//! library (`search.json`, `search-inputs.json`). Every query must give the same ordered ids,
//! the same field, the same UTF-16 name-hit positions and scores within 1e-6.
//!
//! `plain-summaries.json` is the recorded `plainSummary(content, undefined, 2000)` output.
//! These tests index with `template::summary::plain_summary` at 2,000 units. The recorded file
//! supplies a separate summary comparison; ranking itself always calls the real summariser.

use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use kudzu_core::library::usage::usage_by_id;
use kudzu_core::search::{self, MatchField};
use kudzu_core::template::summary::{plain_summary, SEARCH_UNITS};
use kudzu_core::{KudzuLibrary, SnippetRecord};
use serde::Deserialize;
use uuid::Uuid;

/// The clock the fixtures were recorded at.
fn fixture_now() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(1_794_398_400_000).unwrap()
}

fn read(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[derive(Deserialize)]
struct Inputs {
    library: KudzuLibrary,
    large: Vec<SnippetRecord>,
}

#[derive(Deserialize)]
struct Dataset {
    dataset: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    query: String,
    limit: usize,
    hits: Vec<ExpectedHit>,
}

#[derive(Deserialize)]
struct ExpectedHit {
    id: Uuid,
    score: f64,
    field: Option<String>,
    #[serde(rename = "nameHits")]
    name_hits: Vec<usize>,
}

fn field_name(field: Option<MatchField>) -> Option<&'static str> {
    field.map(|f| match f {
        MatchField::Name => "name",
        MatchField::Trigger => "trigger",
        MatchField::Content => "content",
    })
}

#[test]
fn every_query_matches_the_typescript_ranking() {
    let inputs: Inputs = serde_json::from_str(&read("search-inputs.json")).unwrap();
    let datasets: Vec<Dataset> = serde_json::from_str(&read("search.json")).unwrap();
    let usage = usage_by_id(&inputs.library.usage);
    let summarize = |content: &str| plain_summary(content, None, SEARCH_UNITS);

    let mut compared = 0;
    for dataset in &datasets {
        let snippets = match dataset.dataset.as_str() {
            "preview" => &inputs.library.snippets,
            "large" => &inputs.large,
            other => panic!("unknown dataset {other}"),
        };
        let index = search::index(snippets, &usage, &summarize);
        for case in &dataset.cases {
            let label = format!("{} {:?}", dataset.dataset, case.query);
            let hits = search::rank(&index, &case.query, case.limit, fixture_now());
            let got: Vec<Uuid> = hits.iter().map(|h| snippets[h.position].id).collect();
            let want: Vec<Uuid> = case.hits.iter().map(|h| h.id).collect();
            assert_eq!(got, want, "{label}: ordered ids");
            for (hit, expected) in hits.iter().zip(&case.hits) {
                assert!(
                    (hit.score - expected.score).abs() < 1e-6,
                    "{label}: score of {} was {} want {}",
                    expected.id,
                    hit.score,
                    expected.score
                );
                assert_eq!(
                    field_name(hit.field),
                    expected.field.as_deref(),
                    "{label}: field of {}",
                    expected.id
                );
                assert_eq!(
                    hit.name_hits, expected.name_hits,
                    "{label}: name hits of {}",
                    expected.id
                );
                compared += 1;
            }
        }
    }
    // 26 queries over two datasets; 2,027 recorded hits (ten of the queries fill the 200 limit).
    assert!(compared > 2_000, "only {compared} hits compared");
}

#[test]
fn limit_truncates_after_the_full_ordering() {
    let inputs: Inputs = serde_json::from_str(&read("search-inputs.json")).unwrap();
    let usage = usage_by_id(&inputs.library.usage);
    let index = search::index(&inputs.large, &usage, &|c| {
        plain_summary(c, None, SEARCH_UNITS)
    });
    let all = search::rank(&index, "sig", usize::MAX, fixture_now());
    let few = search::rank(&index, "sig", 7, fixture_now());
    assert!(all.len() > 200);
    assert_eq!(few.len(), 7);
    assert_eq!(
        few.iter().map(|h| h.position).collect::<Vec<_>>(),
        all.iter().take(7).map(|h| h.position).collect::<Vec<_>>()
    );
}
