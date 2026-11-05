//! Usage statistics: per-snippet totals across devices, time saved and palette ranking.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{iso8601_opt, SnippetRecord, UsageRecord};

/// Average characters per word for the time-saved estimate (the conventional WPM definition).
pub const CHARACTERS_PER_WORD: f64 = 5.0;
/// Days after which the recency boost in [`usage_rank`] halves.
pub const RECENCY_HALF_LIFE_DAYS: f64 = 7.0;
/// Weight of the recency boost relative to `ln(1 + count)`.
pub const RECENCY_WEIGHT: f64 = 2.0;

/// Usage totals for one snippet, summed across every device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetStats {
    pub snippet_id: Uuid,
    pub count: i64,
    pub chars_saved: i64,
    #[serde(with = "iso8601_opt", default)]
    pub last_used_at: Option<DateTime<Utc>>,
}

impl SnippetStats {
    fn empty(snippet_id: Uuid) -> Self {
        Self {
            snippet_id,
            count: 0,
            chars_saved: 0,
            last_used_at: None,
        }
    }
}

/// Library-wide usage statistics, as shown in the Stats window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSummary {
    /// Every known snippet, sorted by count, then chars saved, then recency (descending), then id.
    pub per_snippet: Vec<SnippetStats>,
    pub total_expansions: i64,
    pub total_chars_saved: i64,
    /// Seconds of typing saved at the configured typing speed.
    pub estimated_time_saved: f64,
}

impl StatsSummary {
    /// The `n` most-used snippets, excluding ones never used.
    pub fn top_snippets(&self, n: usize) -> Vec<SnippetStats> {
        self.per_snippet
            .iter()
            .filter(|s| s.count > 0)
            .take(n)
            .cloned()
            .collect()
    }

    /// Palette ranking score for a snippet; 0 when it is unknown or has never been used.
    pub fn usage_rank_for(&self, snippet_id: Uuid, now: DateTime<Utc>) -> f64 {
        self.per_snippet
            .iter()
            .find(|s| s.snippet_id == snippet_id)
            .map_or(0.0, |s| usage_rank(s.count, s.last_used_at, now))
    }

    /// Palette ranking scores for every snippet, for ranking many results at once.
    pub fn usage_ranks(&self, now: DateTime<Utc>) -> HashMap<Uuid, f64> {
        self.per_snippet
            .iter()
            .map(|s| (s.snippet_id, usage_rank(s.count, s.last_used_at, now)))
            .collect()
    }
}

/// Sums per-device usage into per-snippet stats. Usage of snippets not in `snippets` is ignored.
pub fn summary(
    usage: &[UsageRecord],
    snippets: &[SnippetRecord],
    typing_speed_wpm: u32,
) -> StatsSummary {
    let mut by_snippet: HashMap<Uuid, SnippetStats> = snippets
        .iter()
        .map(|s| (s.id, SnippetStats::empty(s.id)))
        .collect();
    for row in usage {
        if let Some(stats) = by_snippet.get_mut(&row.snippet_id) {
            stats.count += row.count;
            stats.chars_saved += row.chars_saved;
            stats.last_used_at = latest(stats.last_used_at, row.last_used_at);
        }
    }
    let mut per_snippet: Vec<SnippetStats> = by_snippet.into_values().collect();
    per_snippet.sort_by(|l, r| {
        r.count
            .cmp(&l.count)
            .then(r.chars_saved.cmp(&l.chars_saved))
            // `None` sorts before any date, so descending puts never-used last.
            .then(r.last_used_at.cmp(&l.last_used_at))
            .then(l.snippet_id.cmp(&r.snippet_id))
    });
    let total_expansions = per_snippet.iter().map(|s| s.count).sum();
    let total_chars_saved = per_snippet.iter().map(|s| s.chars_saved).sum();
    StatsSummary {
        per_snippet,
        total_expansions,
        total_chars_saved,
        estimated_time_saved: time_saved(total_chars_saved, typing_speed_wpm),
    }
}

/// Seconds it would take to type `chars` characters at `typing_speed_wpm` (chars / 5 = words).
pub fn time_saved(chars: i64, typing_speed_wpm: u32) -> f64 {
    if typing_speed_wpm == 0 || chars <= 0 {
        return 0.0;
    }
    chars as f64 / CHARACTERS_PER_WORD / f64::from(typing_speed_wpm) * 60.0
}

/// Search-palette score: `ln(1 + count) + RECENCY_WEIGHT × 0.5^(ageDays / RECENCY_HALF_LIFE_DAYS)`.
///
/// The log keeps heavy hitters from drowning everything else; the boost (2.0 just now, 1.0 after a
/// week, ~0 after a couple of months) lets something used today outrank something used somewhat
/// more often long ago. Future `last_used_at` (clock skew across devices) counts as "now".
pub fn usage_rank(count: i64, last_used_at: Option<DateTime<Utc>>, now: DateTime<Utc>) -> f64 {
    let frequency = (count.max(0) as f64).ln_1p();
    let Some(last) = last_used_at else {
        return frequency;
    };
    let age_seconds = (now - last).num_milliseconds().max(0) as f64 / 1000.0;
    let age_days = age_seconds / 86_400.0;
    frequency + RECENCY_WEIGHT * 0.5f64.powf(age_days / RECENCY_HALF_LIFE_DAYS)
}

/// The later of two optional dates.
pub(crate) fn latest(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    a.max(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn snippets() -> (SnippetRecord, SnippetRecord, SnippetRecord) {
        (
            SnippetRecord::new("A", vec![";a".into()], "alpha"),
            SnippetRecord::new("B", vec![";b".into()], "beta"),
            SnippetRecord::new("C", vec![";c".into()], "gamma"),
        )
    }

    fn row(
        id: Uuid,
        device: &str,
        count: i64,
        chars: i64,
        last: Option<DateTime<Utc>>,
    ) -> UsageRecord {
        UsageRecord {
            snippet_id: id,
            device_id: device.into(),
            count,
            chars_saved: chars,
            last_used_at: last,
        }
    }

    fn t0() -> DateTime<Utc> {
        Utc.timestamp_opt(1_000_000_000, 0).unwrap()
    }

    #[test]
    fn sums_across_devices_and_sorts_by_count_descending() {
        let (a, b, c) = snippets();
        let usage = [
            row(a.id, "mac1", 2, 20, Some(t0())),
            row(a.id, "mac2", 3, 30, Some(t0() + Duration::seconds(60))),
            row(b.id, "mac1", 10, 5, Some(t0())),
        ];
        let s = summary(&usage, &[a.clone(), b.clone(), c.clone()], 40);
        assert_eq!(
            s.per_snippet
                .iter()
                .map(|x| x.snippet_id)
                .collect::<Vec<_>>(),
            vec![b.id, a.id, c.id]
        );
        assert_eq!(s.per_snippet[1].count, 5);
        assert_eq!(s.per_snippet[1].chars_saved, 50);
        assert_eq!(
            s.per_snippet[1].last_used_at,
            Some(t0() + Duration::seconds(60))
        );
        assert_eq!(s.per_snippet[2].count, 0);
        assert_eq!(s.per_snippet[2].last_used_at, None);
        assert_eq!(s.total_expansions, 15);
        assert_eq!(s.total_chars_saved, 55);
    }

    #[test]
    fn estimates_time_saved_from_wpm() {
        let (a, _, _) = snippets();
        let usage = [row(a.id, "m", 1, 1000, None)];
        assert_eq!(
            summary(&usage, std::slice::from_ref(&a), 40).estimated_time_saved,
            300.0
        );
        assert_eq!(
            summary(&usage, std::slice::from_ref(&a), 0).estimated_time_saved,
            0.0
        );
    }

    #[test]
    fn ignores_usage_for_unknown_snippets() {
        let (a, _, _) = snippets();
        let s = summary(&[row(Uuid::new_v4(), "m", 7, 70, None)], &[a], 40);
        assert_eq!(s.total_expansions, 0);
        assert_eq!(s.per_snippet.len(), 1);
    }

    #[test]
    fn top_snippets_skips_unused_and_limits() {
        let (a, b, c) = snippets();
        let usage = [row(a.id, "m", 1, 0, None), row(b.id, "m", 2, 0, None)];
        let s = summary(&usage, &[a.clone(), b.clone(), c], 40);
        assert_eq!(
            s.top_snippets(1)
                .iter()
                .map(|x| x.snippet_id)
                .collect::<Vec<_>>(),
            vec![b.id]
        );
        assert_eq!(
            s.top_snippets(10)
                .iter()
                .map(|x| x.snippet_id)
                .collect::<Vec<_>>(),
            vec![b.id, a.id]
        );
    }

    #[test]
    fn ties_are_broken_deterministically() {
        let (a, b, _) = snippets();
        let usage = [
            row(a.id, "m", 1, 1, Some(t0())),
            row(b.id, "m", 1, 1, Some(t0())),
        ];
        let reversed = [usage[1].clone(), usage[0].clone()];
        let first = summary(&usage, &[a.clone(), b.clone()], 40);
        let second = summary(&reversed, &[b, a], 40);
        assert_eq!(first.per_snippet, second.per_snippet);
    }

    #[test]
    fn usage_rank_formula() {
        assert_eq!(usage_rank(0, None, t0()), 0.0);
        let fresh = usage_rank(1, Some(t0()), t0());
        assert!((fresh - (2f64.ln() + 2.0)).abs() < 1e-9);
        let week_old = usage_rank(1, Some(t0()), t0() + Duration::days(7));
        assert!((week_old - (2f64.ln() + 1.0)).abs() < 1e-9);
        let future = usage_rank(1, Some(t0() + Duration::hours(1)), t0());
        assert!((future - fresh).abs() < 1e-9);
    }

    #[test]
    fn recent_beats_slightly_more_frequent_but_stale() {
        let (a, b, c) = snippets();
        let now = t0() + Duration::days(60);
        let usage = [
            row(a.id, "m", 12, 0, Some(t0())),
            row(b.id, "m", 8, 0, Some(now)),
        ];
        let s = summary(&usage, &[a.clone(), b.clone()], 40);
        assert!(s.usage_rank_for(b.id, now) > s.usage_rank_for(a.id, now));
        assert_eq!(s.usage_rank_for(c.id, now), 0.0);
        assert_eq!(s.usage_ranks(now).len(), 2);
    }
}
