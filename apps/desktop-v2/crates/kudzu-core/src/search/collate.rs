//! Locale-aware string ordering, standing in for JavaScript's `a.localeCompare(b)`.
//!
//! The Tauri app breaks ranking ties and orders sibling groups with `localeCompare`, which is
//! ICU's root collation (CLDR defaults: tertiary strength, punctuation not ignorable). The tie
//! order is visible, since in a large library most search results tie on score. This module
//! uses ICU4X's `icu_collator` with compiled data, the root (`und`) locale and tertiary strength.
//! It is crate-private: callers use [`rank`](super::rank) and the group tree.
//!
//! The collator is built once on first use. Sorting many names should compare [`CollationKey`]s,
//! computed once per name, rather than strings.
//!
//! `collation_tests.rs` checks the ordering against 500+ strings recorded from Node's
//! `localeCompare` (`docs/migration/reference/collation.mjs`), and lists where ICU4X's data
//! version differs from Node's.

use std::cmp::Ordering;
use std::sync::LazyLock;

use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};

static COLLATOR: LazyLock<CollatorBorrowed<'static>> = LazyLock::new(|| {
    let mut options = CollatorOptions::default();
    options.strength = Some(Strength::Tertiary);
    Collator::try_new(CollatorPreferences::default(), options)
        .expect("ICU collation data is compiled in")
});

/// A string's sort key: byte-wise comparison of two keys orders their strings like
/// [`locale_compare`], and equal keys mean the strings collate equal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub(crate) struct CollationKey(Vec<u8>);

impl CollationKey {
    pub(crate) fn new(text: &str) -> Self {
        let mut key = Vec::with_capacity(text.len() * 2 + 4);
        let Ok(()) = COLLATOR.write_sort_key_to(text, &mut key);
        CollationKey(key)
    }
}

/// Orders `a` against `b` like `a.localeCompare(b)` in the Tauri app.
pub(crate) fn locale_compare(a: &str, b: &str) -> Ordering {
    COLLATOR.compare(a, b)
}
