//! Search speed budget: ranking the seeded 5,000-snippet library must take at most 2 ms at the
//! 95th percentile per keystroke (index already built).
//!
//! The assertion only holds for optimised code, so it is enforced in release builds
//! (`cargo test --release -p kudzu-core --test search_perf`); a debug run just exercises the
//! queries and prints the timing.

mod template_support;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use template_support::SplitMix64;

use chrono::DateTime;
use kudzu_core::library::usage::usage_by_id;
use kudzu_core::search::{self, DEFAULT_LIMIT};
use kudzu_core::template::summary::{plain_summary, SEARCH_UNITS};
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

/// Same ranking budget when every summary is near the 2,000-unit cap. The index is built with
/// the real summariser, not the recorded map.
#[test]
fn ranking_long_summaries_stays_within_the_keystroke_budget() {
    let inputs: Inputs = serde_json::from_str(&fixture("search-inputs.json")).unwrap();
    let mut snippets = inputs.large;
    assert_eq!(snippets.len(), 5_000);
    let mut rng = SplitMix64(0x5323_2000);
    const ENGLISH: &[&str] = &[
        "project",
        "email",
        "signature",
        "schedule",
        "meeting",
        "customer",
        "delivery",
        "thanks",
        "review",
        "clipboard",
        "invoice",
        "quick",
        "brown",
        "fox",
        "jumps",
        "over",
        "lazy",
        "dog",
        "café",
        "summary",
        "budget",
        "next",
        "week",
        "plan",
    ];
    const MIXED: &[&str] = &[
        "العربية",
        "الموعد",
        "شكرا",
        "東京",
        "日本語",
        "café",
        "İstanbul",
        "😀",
        "👩‍💻",
        "résumé",
        "αθήνα",
        "مرحبا",
    ];
    for (i, snippet) in snippets.iter_mut().enumerate() {
        let mut words = Vec::new();
        let mut units = 0;
        while units < 1980 {
            let word = if i % 2 == 1 && rng.below(4) == 0 {
                rng.pick(MIXED)
            } else {
                rng.pick(ENGLISH)
            };
            words.push(*word);
            units += word.encode_utf16().count() + 1;
        }
        snippet.content = format!("{} {{{{date}}}}", words.join(" "));
        let length = plain_summary(&snippet.content, None, SEARCH_UNITS)
            .encode_utf16()
            .count();
        assert!((1980..=2000).contains(&length));
    }
    assert!(
        snippets
            .iter()
            .map(|s| &s.content)
            .collect::<std::collections::HashSet<_>>()
            .len()
            > 4500,
        "long summaries must be varied"
    );
    let usage = usage_by_id(&inputs.library.usage);
    let index = search::index(&snippets, &usage, &|content| {
        plain_summary(content, None, SEARCH_UNITS)
    });
    let now = DateTime::from_timestamp_millis(1_794_398_400_000).unwrap();
    let mut queries: Vec<String> = [
        "project",
        "schedule",
        "review",
        "clipboard",
        "thanks",
        "café",
        "العربية",
        "東京",
        "👩‍💻",
        "invoice summary",
        "customer delivery",
        "meeting next week",
        "zyxwvut",
    ]
    .map(String::from)
    .to_vec();
    let typed = "email signature";
    queries.extend((1..=typed.len()).map(|n| typed[..n].to_string()));
    // Even the absent contiguous term uses letters that are in the bodies: the alphabet
    // shortcut cannot make this workload pass without scanning realistic summaries.
    for query in &queries {
        assert!(
            snippets.iter().any(|s| query
                .to_lowercase()
                .chars()
                .all(|c| s.content.to_lowercase().contains(c))),
            "query alphabet: {query}"
        );
    }

    let mut samples_ms = Vec::new();
    let mut by_query: HashMap<&str, Vec<f64>> = HashMap::new();
    for _ in 0..ROUNDS {
        for query in &queries {
            let start = Instant::now();
            let hits = search::rank(&index, query, DEFAULT_LIMIT, now);
            let elapsed = start.elapsed().as_secs_f64() * 1_000.0;
            samples_ms.push(elapsed);
            by_query.entry(query).or_default().push(elapsed);
            assert!(hits.len() <= DEFAULT_LIMIT);
        }
    }
    let mut per_query: Vec<_> = by_query
        .into_iter()
        .map(|(query, mut times)| {
            times.sort_by(f64::total_cmp);
            (query, times[times.len() * 95 / 100])
        })
        .collect();
    per_query.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("long-summary p95 by query: {per_query:?}");
    samples_ms.sort_by(|a, b| a.total_cmp(b));
    let p95 = samples_ms[(samples_ms.len() * 95 / 100).min(samples_ms.len() - 1)];
    let worst = samples_ms.last().unwrap();
    println!(
        "rank over 5,000 long summaries: p95 {p95:.3} ms, worst {worst:.3} ms ({} samples)",
        samples_ms.len()
    );
    if cfg!(debug_assertions) {
        eprintln!("debug build: the {BUDGET_MS} ms budget is only enforced in release");
    } else {
        assert!(p95 <= BUDGET_MS, "p95 {p95:.3} ms exceeds {BUDGET_MS} ms");
    }
}
