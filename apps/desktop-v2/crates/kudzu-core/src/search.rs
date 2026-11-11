//! Fuzzy, usage-boosted snippet search for the Quick Search palette and the library list.
//!
//! A port of the Tauri app's `lib/search.ts`, pinned to its recorded output. Each
//! whitespace-separated term of the query must match the snippet's name, one of its triggers, or
//! its content summary. Names and triggers match as subsequences (the fzf model, greedy variant:
//! per-character scores, bonuses for word starts and consecutive runs); content only matches
//! contiguous text, because subsequences over long bodies are noise. Matches weigh
//! name 1 > trigger .85 > content .6, a term equal to a trigger scores 10,000 and a trigger
//! prefix 5,000, so typing a trigger always puts its snippet first. Usage nudges ties
//! (`log2(1 + count) × 4` plus 6 / 3 / 1 for use within a day / week / month); a disabled
//! snippet loses 15.
//!
//! [`index`] folds the searchable fields once, [`rank`] runs per keystroke, and [`highlight`]
//! turns a hit's name positions into ranges to draw.
//!
//! # Deliberate parity with the TypeScript
//!
//! - Positions and lengths count UTF-16 code units, as in JavaScript: an emoji is two, a query
//!   term is cut at 64 units (possibly inside a surrogate pair), and the recorded fixtures list
//!   positions in that unit.
//! - Name positions index the *lowercased* name. Characters whose lowercase is longer, such as
//!   `İ` (which lowercases to `i` and a combining dot), shift later positions, so [`highlight`]
//!   is off by that many units for such names, as the TypeScript is.
//! - Ties in score are broken with a locale comparison of the snippet's own name
//!   (ICU collation via `icu_collator`, see `collate`), not the displayed fallback name; equal
//!   names keep library order.
//! - The content a term is matched against is the plain summary of the template (tags shown as
//!   their chip labels, whitespace collapsed, first 2,000 units). Callers pass
//!   [`plain_summary`](crate::template::summary::plain_summary). [`index`] takes that function
//!   so a test can substitute one.

mod collate;
#[cfg(test)]
mod collation_tests;
mod content;
mod fuzzy;
#[cfg(test)]
mod tests;

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use chrono::{DateTime, Utc};
use uuid::Uuid;

pub(crate) use collate::locale_compare;
use collate::CollationKey;

use crate::library::usage::SnippetUsage;
use crate::SnippetRecord;
use content::Content;
use fuzzy::{fuzzy, is_js_space_char, units};

/// Results `rank` returns when a caller has no reason to ask for another number.
pub const DEFAULT_LIMIT: usize = 200;

/// Longest query term, in UTF-16 units; the rest is ignored.
const MAX_TERM_UNITS: usize = 64;
const DAY_MS: f64 = 86_400_000.0;

/// What a hit matched best.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchField {
    Name,
    Trigger,
    Content,
}

/// One ranked snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    /// Index of the snippet in the slice given to [`index`].
    pub position: usize,
    pub score: f64,
    /// The field of the best-matching term; `None` for the empty query.
    pub field: Option<MatchField>,
    /// UTF-16 unit positions in the lowercased display name that matched, ascending.
    pub name_hits: Vec<usize>,
}

/// The pre-folded searchable fields of a snippet list.
#[derive(Debug, Clone, Default)]
pub struct SearchIndex {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
struct Entry {
    name_lower: Vec<u16>,
    triggers_lower: Vec<Vec<u16>>,
    content: Content,
    usage: i64,
    last_used_ms: i64,
    enabled: bool,
    /// Tie-break key of the snippet's own name (empty names sort first).
    sort_key: CollationKey,
}

impl SearchIndex {
    /// The number of indexed snippets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Folds `snippets` for searching. `usage` is per snippet, summed across devices
/// ([`usage_by_id`](crate::library::usage::usage_by_id)). `summarize` returns the plain-text
/// summary of a template body, at most 2,000 units; it runs once per snippet.
pub fn index(
    snippets: &[SnippetRecord],
    usage: &HashMap<Uuid, SnippetUsage>,
    summarize: &dyn Fn(&str) -> String,
) -> SearchIndex {
    let entries = snippets
        .iter()
        .map(|s| {
            let used = usage.get(&s.id);
            let content_lower = units(&summarize(&s.content).to_lowercase());
            Entry {
                name_lower: units(&display_name(s).to_lowercase()),
                triggers_lower: s
                    .triggers
                    .iter()
                    .map(|t| units(&t.to_lowercase()))
                    .collect(),
                content: Content::new(content_lower),
                usage: used.map_or(0, |u| u.count),
                last_used_ms: used.map_or(0, |u| u.last_ms),
                enabled: s.is_enabled,
                sort_key: CollationKey::new(&s.name),
            }
        })
        .collect();
    SearchIndex { entries }
}

/// The name a snippet is searched and listed under: its name, else its first trigger.
fn display_name(snippet: &SnippetRecord) -> &str {
    if !snippet.name.is_empty() {
        return &snippet.name;
    }
    match snippet.triggers.first() {
        Some(t) if !t.is_empty() => t,
        _ => "Untitled",
    }
}

fn usage_boost(entry: &Entry, now_ms: i64) -> f64 {
    let frequency = (1.0 + entry.usage as f64).log2() * 4.0;
    let recency = if entry.last_used_ms == 0 {
        0.0
    } else {
        let age_days = (now_ms - entry.last_used_ms) as f64 / DAY_MS;
        if age_days < 1.0 {
            6.0
        } else if age_days < 7.0 {
            3.0
        } else if age_days < 30.0 {
            1.0
        } else {
            0.0
        }
    };
    frequency + recency
}

/// Ranks the index for `query` at time `now`, best first, at most `limit` hits. The empty
/// query lists snippets by usage, then name.
pub fn rank(index: &SearchIndex, query: &str, limit: usize, now: DateTime<Utc>) -> Vec<SearchHit> {
    if limit == 0 {
        return Vec::new();
    }
    let now_ms = now.timestamp_millis();
    let lowered = query.trim_matches(is_js_space_char).to_lowercase();
    let terms: Vec<Vec<u16>> = lowered
        .split(is_js_space_char)
        .filter(|t| !t.is_empty())
        .map(|t| {
            let mut term = units(t);
            term.truncate(MAX_TERM_UNITS);
            term
        })
        .collect();

    let mut scored: Vec<Scored> = if terms.is_empty() {
        index
            .entries
            .iter()
            .enumerate()
            .map(|(position, e)| Scored {
                position,
                score: usage_boost(e, now_ms),
                field: None,
            })
            .collect()
    } else {
        index
            .entries
            .iter()
            .enumerate()
            .filter_map(|(position, e)| {
                score_entry(e, &terms, now_ms).map(|(score, field)| Scored {
                    position,
                    score,
                    field,
                })
            })
            .collect()
    };

    let order = |a: &Scored, b: &Scored| -> Ordering {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                index.entries[a.position]
                    .sort_key
                    .cmp(&index.entries[b.position].sort_key)
            })
            .then(a.position.cmp(&b.position))
    };
    if scored.len() > limit {
        scored.select_nth_unstable_by(limit - 1, order);
        scored.truncate(limit);
    }
    scored.sort_unstable_by(order);

    scored
        .into_iter()
        .map(|s| SearchHit {
            position: s.position,
            score: s.score,
            field: s.field,
            name_hits: name_hits(&index.entries[s.position], &terms),
        })
        .collect()
}

struct Scored {
    position: usize,
    score: f64,
    field: Option<MatchField>,
}

/// Scores one entry against every term; `None` unless all terms match.
fn score_entry(
    entry: &Entry,
    terms: &[Vec<u16>],
    now_ms: i64,
) -> Option<(f64, Option<MatchField>)> {
    let mut total = 0.0;
    let mut field = None;
    let mut best = f64::NEG_INFINITY;
    for term in terms {
        let mut term_best = f64::NEG_INFINITY;
        let mut term_field = None;
        if let Some(score) = fuzzy(term, &entry.name_lower, None) {
            term_best = score;
            term_field = Some(MatchField::Name);
        }
        for trigger in &entry.triggers_lower {
            if trigger == term {
                term_best = term_best.max(10_000.0);
                term_field = Some(MatchField::Trigger);
                continue;
            }
            if trigger.starts_with(term) {
                if 5_000.0 > term_best {
                    term_best = 5_000.0 - trigger.len() as f64;
                    term_field = Some(MatchField::Trigger);
                }
                continue;
            }
            if let Some(score) = fuzzy(term, trigger, None) {
                if score * 0.85 > term_best {
                    term_best = score * 0.85;
                    term_field = Some(MatchField::Trigger);
                }
            }
        }
        // Content only matches contiguous text: subsequences over long bodies are noise (and the
        // slowest part of a keystroke).
        if term_best < 40.0 && term.len() > 1 {
            if let Some(at_boundary) = entry.content.match_boundary(term) {
                let boundary = if at_boundary { 10.0 } else { 0.0 };
                let content = (term.len() * 16) as f64 + boundary;
                if content * 0.6 > term_best {
                    term_best = content * 0.6;
                    term_field = Some(MatchField::Content);
                }
            }
        }
        if term_best == f64::NEG_INFINITY {
            return None;
        }
        total += term_best;
        if term_best > best {
            best = term_best;
            field = term_field;
        }
    }
    total += usage_boost(entry, now_ms);
    if !entry.enabled {
        total -= 15.0;
    }
    Some((total, field))
}

/// Every unit of the name that any term matched, ascending and without repeats.
fn name_hits(entry: &Entry, terms: &[Vec<u16>]) -> Vec<usize> {
    let mut hits = Vec::new();
    for term in terms {
        fuzzy(term, &entry.name_lower, Some(&mut hits));
    }
    hits.sort_unstable();
    hits.dedup();
    hits
}

/// Byte ranges of `text` to emphasise for `hits` (UTF-16 unit positions, as in
/// [`SearchHit::name_hits`]). A hit on either half of a surrogate pair selects the whole
/// character; adjacent characters merge into one range; positions past the end are ignored.
pub fn highlight(text: &str, hits: &[usize]) -> Vec<Range<usize>> {
    if hits.is_empty() {
        return Vec::new();
    }
    let hit: HashSet<usize> = hits.iter().copied().collect();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut unit = 0;
    for (byte, c) in text.char_indices() {
        let width = c.len_utf16();
        if (unit..unit + width).any(|u| hit.contains(&u)) {
            let end = byte + c.len_utf8();
            match ranges.last_mut() {
                Some(last) if last.end == byte => last.end = end,
                _ => ranges.push(byte..end),
            }
        }
        unit += width;
    }
    ranges
}
