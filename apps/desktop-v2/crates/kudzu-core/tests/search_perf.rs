//! Search speed budget: ranking the seeded 5,000-snippet library must take at most 2 ms at the
//! 95th percentile per keystroke (index already built).
//!
//! The assertion only holds for optimised code, so it is enforced in release builds
//! (`cargo test --release -p kudzu-core --test search_perf`); a debug run just exercises the
//! queries and prints the timing.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use chrono::DateTime;
use kudzu_core::library::usage::usage_by_id;
use kudzu_core::search::{self, DEFAULT_LIMIT};
use kudzu_core::{KudzuLibrary, SnippetRecord};
use serde::Deserialize;

#[derive(Deserialize)]
struct Inputs {
    library: KudzuLibrary,
    large: Vec<SnippetRecord>,
}

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    fs::read_to_string(path).unwrap()
}

const BUDGET_MS: f64 = 2.0;
const ROUNDS: usize = 20;

#[test]
fn ranking_5000_snippets_stays_within_the_keystroke_budget() {
    let inputs: Inputs = serde_json::from_str(&fixture("search-inputs.json")).unwrap();
    let summaries: HashMap<String, String> =
        serde_json::from_str(&fixture("plain-summaries.json")).unwrap();
    assert_eq!(inputs.large.len(), 5_000);
    let usage = usage_by_id(&inputs.library.usage);
    let index = search::index(&inputs.large, &usage, &|c| summaries[c].clone());
    let now = DateTime::from_timestamp_millis(1_794_398_400_000).unwrap();

    // The queries the fixtures record, plus typing "email signature" one key at a time.
    let mut queries: Vec<String> = [
        "",
        ";sig",
        ";s42",
        "sig",
        "em sg",
        "clipboard",
        "thanks",
        "café",
        "العربية",
        "İ",
        "😀",
        "no-such-match",
    ]
    .map(String::from)
    .to_vec();
    let typed = "email signature";
    queries.extend((1..=typed.len()).map(|n| typed[..n].to_string()));

    let mut samples_ms = Vec::new();
    for _ in 0..ROUNDS {
        for query in &queries {
            let start = Instant::now();
            let hits = search::rank(&index, query, DEFAULT_LIMIT, now);
            samples_ms.push(start.elapsed().as_secs_f64() * 1_000.0);
            assert!(hits.len() <= DEFAULT_LIMIT);
        }
    }
    samples_ms.sort_by(|a, b| a.total_cmp(b));
    let p95 = samples_ms[(samples_ms.len() * 95 / 100).min(samples_ms.len() - 1)];
    let worst = samples_ms.last().unwrap();
    println!(
        "rank over 5,000 snippets: p95 {p95:.3} ms, worst {worst:.3} ms ({} samples)",
        samples_ms.len()
    );
    if cfg!(debug_assertions) {
        eprintln!("debug build: the {BUDGET_MS} ms budget is only enforced in release");
    } else {
        assert!(p95 <= BUDGET_MS, "p95 {p95:.3} ms exceeds {BUDGET_MS} ms");
    }
}
