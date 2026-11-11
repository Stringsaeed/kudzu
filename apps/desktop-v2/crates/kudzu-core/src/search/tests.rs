//! Hand-computed scoring cases; the TypeScript fixtures cover the rest (`tests/ts_parity_*.rs`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::fuzzy::{fuzzy, units};
use super::*;
use crate::library::usage::SnippetUsage;
use crate::SnippetRecord;

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(1_794_398_400_000).unwrap()
}

fn plain(content: &str) -> String {
    content.to_string()
}

fn snippet(name: &str, triggers: &[&str], content: &str) -> SnippetRecord {
    SnippetRecord::new(
        name,
        triggers.iter().map(|t| t.to_string()).collect(),
        content,
    )
}

fn rank_one(snippet: &SnippetRecord, query: &str) -> Option<SearchHit> {
    let index = index(std::slice::from_ref(snippet), &HashMap::new(), &plain);
    rank(&index, query, DEFAULT_LIMIT, now()).into_iter().next()
}

fn score(term: &str, text: &str) -> Option<(f64, Vec<usize>)> {
    let mut positions = Vec::new();
    fuzzy(&units(term), &units(text), Some(&mut positions)).map(|s| (s, positions))
}

#[test]
fn contiguous_match_scores_length_boundary_and_start_bonuses() {
    // 3×16 + 2×8, word start +20, text start +20.
    assert_eq!(score("sig", "signature"), Some((104.0, vec![0, 1, 2])));
    // Whole-text match adds +20 and a late start costs 0.5 per unit, capped at 20 units.
    assert_eq!(score("sig", "sig"), Some((124.0, vec![0, 1, 2])));
    assert_eq!(
        score("up", "sign-up"),
        Some((2.0 * 16.0 + 8.0 + 20.0 - 2.5, vec![5, 6]))
    );
    assert_eq!(
        score("z", &format!("{}z", "a".repeat(30))),
        Some((16.0 - 10.0, vec![30]))
    );
}

#[test]
fn scattered_match_uses_the_tightest_window_and_gap_penalties() {
    // s (word start, first term char) 16 + 20, gap -3 after an adjacent hit, g 16.
    assert_eq!(score("sg", "signature"), Some((49.0, vec![0, 2])));
    // The earliest match is a(0)…b(7); the backward pass tightens it to a(5)…b(7). a: 16 + 20,
    // the gap after an adjacent hit -3, b: 16 + 10 for the word start.
    assert_eq!(
        score("ab", "a xx a b"),
        Some((36.0 - 3.0 + 26.0, vec![5, 7]))
    );
    assert_eq!(score("zz", "signature"), None);
    assert_eq!(score("toolong", "ab"), None);
}

#[test]
fn boundaries_are_whitespace_punctuation_and_letter_to_digit() {
    // a - b _ c . d 9 ␠ e
    let text = units("a-b_c.d9 e");
    let want = [
        true, false, true, false, true, false, true, true, false, true,
    ];
    for (i, expected) in want.iter().enumerate() {
        assert_eq!(fuzzy::is_boundary(&text, i), *expected, "index {i}");
    }
    // Unicode whitespace counts as a separator, like JavaScript's `\s`.
    assert!(fuzzy::is_boundary(&units("a\u{a0}b"), 2));
    assert!(fuzzy::is_boundary(&units("a\u{3000}b"), 2));
}

#[test]
fn an_exact_trigger_beats_everything() {
    let s = snippet("Sig", &[";sig"], "");
    let hit = rank_one(&s, ";sig").unwrap();
    assert_eq!(hit.score, 10_000.0);
    assert_eq!(hit.field, Some(MatchField::Trigger));
    // The name does not contain the term, so there is nothing to highlight.
    assert!(hit.name_hits.is_empty());
}

#[test]
fn a_trigger_prefix_scores_5000_minus_its_length() {
    let s = snippet("Other", &[";signature"], "");
    let hit = rank_one(&s, ";sig").unwrap();
    assert_eq!(hit.score, 5_000.0 - 10.0);
    assert_eq!(hit.field, Some(MatchField::Trigger));
}

#[test]
fn fuzzy_trigger_matches_weigh_85_percent() {
    let s = snippet("Other", &["xx-sig"], "");
    let hit = rank_one(&s, "sig").unwrap();
    // "sig" inside "xx-sig": 48 + 16 = 64 + boundary 20 - 1.5 for the offset, ×0.85.
    assert!((hit.score - (3.0 * 16.0 + 16.0 + 20.0 - 1.5) * 0.85).abs() < 1e-9);
    assert_eq!(hit.field, Some(MatchField::Trigger));
}

#[test]
fn content_only_matches_contiguous_text_at_60_percent() {
    let s = snippet("Greeting", &[";hi"], "say hello there");
    let hit = rank_one(&s, "hello").unwrap();
    // 5×16 + 10 for a word start, ×0.6.
    assert!((hit.score - 90.0 * 0.6).abs() < 1e-9);
    assert_eq!(hit.field, Some(MatchField::Content));
    // A subsequence is not enough in the body, and single characters never search it.
    assert!(rank_one(&s, "hlo").is_none());
    assert!(rank_one(&snippet("Greeting", &[";hi"], "xyz"), "z").is_none());
}

#[test]
fn the_summary_is_what_content_search_sees() {
    let s = snippet("Greeting", &[";hi"], "{{clipboard}}");
    let index = index(std::slice::from_ref(&s), &HashMap::new(), &|_| {
        "‹Clipboard›".into()
    });
    let hits = rank(&index, "clipboard", DEFAULT_LIMIT, now());
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].field, Some(MatchField::Content));
}

#[test]
fn every_term_must_match_and_scores_add() {
    let s = snippet("Email signature", &[";sig"], "");
    let both = rank_one(&s, "em sig").unwrap();
    let one = rank_one(&s, "em").unwrap();
    let other = rank_one(&s, "sig").unwrap();
    assert!((both.score - (one.score + other.score)).abs() < 1e-9);
    assert!(rank_one(&s, "em nothere").is_none());
    assert_eq!(both.name_hits, vec![0, 1, 6, 7, 8]);
}

#[test]
fn terms_are_cut_at_64_units_and_the_query_is_trimmed_and_lowercased() {
    let long = "x".repeat(80);
    let s = snippet(&long[..70], &[";x"], "");
    assert!(rank_one(&s, &format!("  {long}  ")).is_some());
    assert!(rank_one(&snippet("Mixed Case", &[";m"], ""), "  MIXED\tcase ").is_some());
}

#[test]
fn usage_and_recency_boost_and_disabled_snippets_lose_15() {
    let mut s = snippet("Alpha", &[";a"], "");
    let usage = HashMap::from([(
        s.id,
        SnippetUsage {
            count: 3,
            chars: 0,
            last_ms: now().timestamp_millis() - 3 * 86_400_000,
        },
    )]);
    let index_ = index(std::slice::from_ref(&s), &usage, &plain);
    let hit = &rank(&index_, "alpha", DEFAULT_LIMIT, now())[0];
    // Name: 5×16 + 4×8 + 20 + 20 + 20 = 172; frequency log2(4)×4 = 8; used 3 days ago: +3.
    assert!((hit.score - (172.0 + 8.0 + 3.0)).abs() < 1e-9);
    s.is_enabled = false;
    let index_ = index(std::slice::from_ref(&s), &usage, &plain);
    let disabled = &rank(&index_, "alpha", DEFAULT_LIMIT, now())[0];
    assert!((disabled.score - (hit.score - 15.0)).abs() < 1e-9);
}

#[test]
fn recency_steps_are_one_day_one_week_and_thirty_days() {
    let s = snippet("Alpha", &[";a"], "");
    let day = 86_400_000;
    for (age_ms, bonus) in [
        (day - 1, 6.0),
        (day, 3.0),
        (7 * day - 1, 3.0),
        (7 * day, 1.0),
        (30 * day - 1, 1.0),
        (30 * day, 0.0),
    ] {
        let usage = HashMap::from([(
            s.id,
            SnippetUsage {
                count: 0,
                chars: 0,
                last_ms: now().timestamp_millis() - age_ms,
            },
        )]);
        let index_ = index(std::slice::from_ref(&s), &usage, &plain);
        let hit = &rank(&index_, "", DEFAULT_LIMIT, now())[0];
        assert_eq!(hit.score, bonus, "age {age_ms} ms");
        assert_eq!(hit.field, None);
        assert!(hit.name_hits.is_empty());
    }
}

#[test]
fn an_empty_query_orders_by_usage_then_name_and_honours_the_limit() {
    let a = snippet("Banana", &[";b"], "");
    let b = snippet("apple", &[";a"], "");
    let c = snippet("Cherry", &[";c"], "");
    let usage = HashMap::from([(
        c.id,
        SnippetUsage {
            count: 5,
            chars: 0,
            last_ms: 0,
        },
    )]);
    let snippets = [a, b, c];
    let index_ = index(&snippets, &usage, &plain);
    let names = |limit| -> Vec<&str> {
        rank(&index_, "   ", limit, now())
            .iter()
            .map(|h| snippets[h.position].name.as_str())
            .collect()
    };
    assert_eq!(names(10), ["Cherry", "apple", "Banana"]);
    assert_eq!(names(2), ["Cherry", "apple"]);
    assert!(names(0).is_empty());
}

#[test]
fn unnamed_snippets_search_and_show_their_first_trigger() {
    let s = snippet("", &[";todo"], "");
    let hit = rank_one(&s, "todo").unwrap();
    // Matched against the fallback name `;todo` (and as a trigger suffix); never "Untitled" once
    // a trigger exists.
    assert_eq!(hit.name_hits, vec![1, 2, 3, 4]);
    let untitled = snippet("", &[], "");
    assert!(rank_one(&untitled, "untitled").is_some());
}

#[test]
fn name_hits_are_utf16_units_of_the_lowercased_name() {
    // The emoji is a surrogate pair: `b` sits at unit 3, not code point 2.
    let s = snippet("a😀b", &[";x"], "");
    assert_eq!(rank_one(&s, "b").unwrap().name_hits, vec![3]);
}

#[test]
fn highlight_turns_unit_hits_into_merged_byte_ranges() {
    assert_eq!(highlight("Signature", &[0, 1, 2, 5]), vec![0..3, 5..6]);
    assert_eq!(
        highlight("Signature", &[]),
        Vec::<std::ops::Range<usize>>::new()
    );
    // Both halves of a surrogate pair select the whole character; out-of-range hits are ignored.
    assert_eq!(highlight("a😀b", &[1]), vec![1..5]);
    assert_eq!(highlight("a😀b", &[2, 3, 99]), vec![1..6]);
    // `é` is two bytes in UTF-8, one UTF-16 unit.
    assert_eq!(highlight("café x", &[3, 5]), vec![3..5, 6..7]);
}
