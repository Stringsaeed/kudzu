//! `locale_compare` against Node's ICU `String.prototype.localeCompare` (en-US).
//!
//! `collation.json` holds a corpus sorted by that function: ASCII punctuation, digits, case,
//! Latin accents and expansions, Greek, Cyrillic, Hebrew, Arabic, emoji, and the shapes of the
//! 5,000-snippet fixture names. Search and the group tree break ties with it.

use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;

use kudzu_core::search::locale_compare;
use serde::Deserialize;

#[derive(Deserialize)]
struct Corpus {
    sorted: Vec<String>,
}

fn corpus() -> Vec<String> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ts-parity/collation.json");
    serde_json::from_str::<Corpus>(&fs::read_to_string(path).unwrap())
        .unwrap()
        .sorted
}

#[test]
fn sorting_the_corpus_reproduces_icu_order() {
    let want = corpus();
    let mut got = want.clone();
    got.reverse();
    got.sort_by(|a, b| locale_compare(a, b));
    assert_eq!(got, want);
}

#[test]
fn every_pair_compares_like_icu() {
    let sorted = corpus();
    for (i, a) in sorted.iter().enumerate() {
        for (j, b) in sorted.iter().enumerate() {
            // Strings ICU treats as equal (none in the corpus differ only by ignorables) keep
            // their relative order, so index order is the expectation.
            assert_eq!(
                locale_compare(a, b),
                i.cmp(&j),
                "{a:?} against {b:?} (corpus positions {i} and {j})"
            );
        }
    }
}

#[test]
fn identical_strings_are_equal() {
    assert_eq!(
        locale_compare("Sign-up form", "Sign-up form"),
        Ordering::Equal
    );
    assert_eq!(locale_compare("", ""), Ordering::Equal);
}
