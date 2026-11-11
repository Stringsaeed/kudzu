//! Per-keystroke budget for the palette: one cached index, then rank and row assembly.
//!
//! Enforced in release (`cargo test --release -p kudzu-core --test palette_perf`). A debug run
//! prints the timing and still checks that a second query does not rebuild the index.

use std::path::PathBuf;
use std::time::Instant;

use chrono::DateTime;
use kudzu_core::search::{PaletteCache, Scope};
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
    std::fs::read_to_string(path).unwrap()
}

const BUDGET_MS: f64 = 2.0;
const ROUNDS: usize = 20;

#[test]
fn palette_keystroke_over_5000_snippets_stays_within_the_rank_budget() {
    let inputs: Inputs = serde_json::from_str(&fixture("search-inputs.json")).unwrap();
    assert_eq!(inputs.large.len(), 5_000);
    let mut library = inputs.library;
    library.snippets = inputs.large;
    let now = DateTime::from_timestamp_millis(1_794_398_400_000).unwrap();
    let cache = PaletteCache::default();
    let _ = cache.answer(0, &library, "", Scope::All, now);
    assert_eq!(
        cache.index_builds(),
        1,
        "the warm query builds the index once"
    );

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
            let listed = cache.answer(0, &library, query, Scope::All, now);
            samples_ms.push(start.elapsed().as_secs_f64() * 1_000.0);
            assert!(listed.rows.len() <= 60);
        }
    }
    assert_eq!(
        cache.index_builds(),
        1,
        "a keystroke must not rebuild the index"
    );
    samples_ms.sort_by(|a, b| a.total_cmp(b));
    let p95 = samples_ms[(samples_ms.len() * 95 / 100).min(samples_ms.len() - 1)];
    let worst = samples_ms.last().copied().unwrap_or(0.0);
    println!(
        "palette keystroke over 5,000 snippets: p95 {p95:.3} ms, worst {worst:.3} ms ({} samples), index builds {}",
        samples_ms.len(),
        cache.index_builds()
    );
    if cfg!(debug_assertions) {
        eprintln!("debug build: the {BUDGET_MS} ms budget is only enforced in release");
    } else {
        assert!(p95 <= BUDGET_MS, "p95 {p95:.3} ms exceeds {BUDGET_MS} ms");
    }
}
