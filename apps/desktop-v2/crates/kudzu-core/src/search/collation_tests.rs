//! The collator against orderings recorded from Node's `String.prototype.localeCompare`.
//!
//! `tests/fixtures/ts-parity/collation.json` is written by
//! `docs/migration/reference/collation.mjs`: 500+ strings (ASCII, digits, punctuation and
//! symbols, emoji, Latin accents and expansions, German, French, Turkish, CJK, other scripts,
//! snippet-name shapes, ignorable characters) with a rank each; equal ranks compare equal.
//! Every pair is checked. Pairs where ICU4X (`icu_collator` 2.4.0, compiled data) legitimately
//! differs from Node's ICU are listed in [`KNOWN_DIFFERENCES`] with the reason, and the test
//! fails if the list and the observed differences ever disagree.

use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

use super::collate::{locale_compare, CollationKey};

#[derive(Deserialize)]
struct Corpus {
    locale: String,
    icu: String,
    entries: Vec<(String, u32)>,
}

fn corpus() -> Corpus {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-parity/collation.json");
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// Pairs (in either order) on which this collator disagrees with the recorded Node ordering.
/// Each is `(a, b, what Node says, why we differ)`.
const KNOWN_DIFFERENCES: &[(&str, &str, &str, &str)] = &[];

fn is_known(a: &str, b: &str) -> bool {
    KNOWN_DIFFERENCES
        .iter()
        .any(|(x, y, ..)| (*x == a && *y == b) || (*x == b && *y == a))
}

#[test]
fn every_pair_orders_like_node_except_the_listed_differences() {
    let corpus = corpus();
    assert_eq!(corpus.locale, "en-US");
    assert!(
        corpus.entries.len() > 500,
        "{} strings",
        corpus.entries.len()
    );
    let mut observed = Vec::new();
    for (a, rank_a) in &corpus.entries {
        for (b, rank_b) in &corpus.entries {
            let want = rank_a.cmp(rank_b);
            let got = locale_compare(a, b);
            let by_key = CollationKey::new(a).cmp(&CollationKey::new(b));
            assert_eq!(got, by_key, "key and compare disagree on {a:?} / {b:?}");
            if got != want && !is_known(a, b) {
                observed.push(format!("{a:?} vs {b:?}: node {want:?}, here {got:?}"));
            }
        }
    }
    assert!(
        observed.is_empty(),
        "{} unlisted differences from Node's ICU {}:\n{}",
        observed.len(),
        corpus.icu,
        observed.join("\n")
    );
    // A listed difference that no longer differs is stale.
    for (a, b, node, why) in KNOWN_DIFFERENCES {
        let rank = |s: &str| corpus.entries.iter().find(|(e, _)| e == s).map(|(_, r)| *r);
        let want = rank(a).unwrap().cmp(&rank(b).unwrap());
        assert_ne!(
            locale_compare(a, b),
            want,
            "{a:?} vs {b:?} ({node}; {why}) now matches Node: remove it from the list"
        );
    }
}

#[test]
fn the_cases_the_review_called_out() {
    let lt = |a: &str, b: &str| assert_eq!(locale_compare(a, b), Ordering::Less, "{a:?} < {b:?}");
    // ß expands to "ss": before t, after sr.
    lt("ß", "t");
    lt("strasse", "straße");
    lt("straße", "strasse 1");
    lt("straße", "strat");
    // Turkish letters sort by the root rules in en-US: dotless ı after i, İ with the dot accent.
    lt("istanbul", "İstanbul");
    lt("İstanbul", "ıstanbul");
    // Digits before letters, emoji before digits, punctuation before everything else.
    lt("😀", "0");
    lt("0", "a");
    lt("-", "😀");
    // Han, kana and Hangul come after Latin.
    lt("z", "한국어");
    lt("z", "ひらがな");
    lt("z", "中文");
}

#[test]
fn equal_and_empty_strings() {
    assert_eq!(locale_compare("", ""), Ordering::Equal);
    assert_eq!(
        locale_compare("Sign-up form", "Sign-up form"),
        Ordering::Equal
    );
    // Canonically equivalent spellings collate equal.
    assert_eq!(locale_compare("e\u{301}", "é"), Ordering::Equal);
    assert_eq!(CollationKey::new("e\u{301}"), CollationKey::new("é"));
}
