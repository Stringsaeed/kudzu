//! Usage aggregation for Insights, list badges and search ranking (`lib/stats.ts`). Usage
//! records are per device; everything here sums across devices. Typed text is never stored.

use std::collections::HashMap;

use chrono::{DateTime, TimeZone, Utc};
use uuid::Uuid;

use crate::KudzuLibrary;

use super::js;

/// One snippet's usage summed across devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SnippetUsage {
    pub count: i64,
    /// Characters saved (negative records count as zero).
    pub chars: i64,
    /// Last use in milliseconds since the epoch; 0 means never (as in the UI).
    pub last: i64,
}

impl SnippetUsage {
    /// The last use, if any.
    pub fn last_used(&self) -> Option<DateTime<Utc>> {
        (self.last != 0).then(|| DateTime::from_timestamp_millis(self.last)).flatten()
    }
}

/// Usage per snippet id.
pub fn usage_by_id(library: &KudzuLibrary) -> HashMap<Uuid, SnippetUsage> {
    let mut map: HashMap<Uuid, SnippetUsage> = HashMap::new();
    for u in &library.usage {
        let cur = map.entry(u.snippet_id).or_default();
        cur.count += u.count;
        cur.chars += u.chars_saved.max(0);
        let t = u.last_used_at.map_or(0, |d| d.timestamp_millis());
        if t > cur.last {
            cur.last = t;
        }
    }
    map
}

/// Insights headline numbers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Totals {
    pub expansions: i64,
    pub chars: i64,
    pub minutes_saved: f64,
    pub active_snippets: usize,
    pub used_snippets: usize,
}

/// Average word length used by typing-speed conventions (5 characters per word).
pub const CHARS_PER_WORD: f64 = 5.0;

/// Totals, with time saved at `wpm` words per minute (at least 1).
pub fn totals(library: &KudzuLibrary, usage: &HashMap<Uuid, SnippetUsage>, wpm: f64) -> Totals {
    let expansions = usage.values().map(|u| u.count).sum();
    let chars = usage.values().map(|u| u.chars).sum();
    Totals {
        expansions,
        chars,
        minutes_saved: chars as f64 / CHARS_PER_WORD / wpm.max(1.0),
        active_snippets: library.snippets.iter().filter(|s| s.is_enabled).count(),
        used_snippets: usage.values().filter(|u| u.count > 0).count(),
    }
}

/// "45s", "12 min", "2.5 h", "14 h", "3.2 days".
pub fn format_duration(minutes: f64) -> String {
    if minutes < 1.0 {
        return format!("{}s", js::number_to_string(js::round(minutes * 60.0)));
    }
    if minutes < 60.0 {
        return format!("{} min", js::number_to_string(js::round(minutes)));
    }
    let h = minutes / 60.0;
    if h < 24.0 {
        return format!("{} h", js::to_fixed(h, if h < 10.0 { 1 } else { 0 }));
    }
    format!("{} days", js::to_fixed(h / 24.0, 1))
}

/// "999", "1.2k", "34k", "1.5M".
pub fn format_count(n: i64) -> String {
    let x = n as f64;
    if x < 1000.0 {
        return n.to_string();
    }
    if x < 1_000_000.0 {
        return format!("{}k", js::to_fixed(x / 1000.0, if x < 10_000.0 { 1 } else { 0 }));
    }
    format!("{}M", js::to_fixed(x / 1_000_000.0, 1))
}

fn relative(n: f64, unit: &str) -> String {
    match (unit, n as i64) {
        ("day", -1) => return "yesterday".into(),
        ("day", 0) => return "today".into(),
        ("day", 1) => return "tomorrow".into(),
        (_, 0) => return format!("this {unit}"),
        _ => {}
    }
    let abs = n.abs() as i64;
    let label = format!("{} {unit}{}", js::group_thousands(abs), if abs == 1 { "" } else { "s" });
    if n < 0.0 || (n == 0.0 && n.is_sign_negative()) {
        format!("{label} ago")
    } else {
        format!("in {label}")
    }
}

/// "Never", "Just now", "5 minutes ago", "in 3 hours", "yesterday", "12 days ago", or
/// "Oct 28, 2026" beyond 30 days (`Intl.RelativeTimeFormat` with `numeric: "auto"`, en-US).
/// `ms` and `now_ms` are milliseconds since the epoch; `tz` places the fallback date.
pub fn relative_time<Tz: TimeZone>(ms: i64, now_ms: i64, tz: &Tz) -> String {
    if ms == 0 {
        return "Never".into();
    }
    let diff = (ms - now_ms) as f64 / 1000.0;
    let abs = diff.abs();
    if abs < 60.0 {
        return "Just now".into();
    }
    if abs < 3600.0 {
        return relative(js::round(diff / 60.0), "minute");
    }
    if abs < 86_400.0 {
        return relative(js::round(diff / 3600.0), "hour");
    }
    if abs < 86_400.0 * 30.0 {
        return relative(js::round(diff / 86_400.0), "day");
    }
    js::date_medium(&js::local(ms, tz))
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, TimeZone};

    use super::*;
    use crate::{SnippetRecord, UsageRecord};

    fn record(id: u128, count: i64, chars: i64, last: Option<&str>) -> UsageRecord {
        UsageRecord {
            snippet_id: Uuid::from_u128(id),
            device_id: "d".into(),
            count,
            chars_saved: chars,
            last_used_at: last.map(|s| crate::types::iso8601::parse(s).unwrap()),
        }
    }

    #[test]
    fn sums_usage_across_devices() {
        let mut library = KudzuLibrary {
            usage: vec![
            record(1, 3, 30, Some("2026-01-01T00:00:00Z")),
            record(1, 2, -5, Some("2026-02-01T00:00:00Z")),
            record(1, 1, 10, Some("2025-01-01T00:00:00Z")),
            record(2, 0, 0, None),
            ],
            ..KudzuLibrary::default()
        };
        let mut off = SnippetRecord::new("off", vec![], "");
        off.is_enabled = false;
        library.snippets = vec![SnippetRecord::new("a", vec![], ""), off];
        let usage = usage_by_id(&library);
        let one = usage[&Uuid::from_u128(1)];
        assert_eq!((one.count, one.chars), (6, 40));
        assert_eq!(one.last, crate::types::iso8601::parse("2026-02-01T00:00:00Z").unwrap().timestamp_millis());
        assert_eq!(usage[&Uuid::from_u128(2)], SnippetUsage::default());
        assert_eq!(usage[&Uuid::from_u128(2)].last_used(), None);
        let t = totals(&library, &usage, 40.0);
        assert_eq!((t.expansions, t.chars, t.active_snippets, t.used_snippets), (6, 40, 1, 1));
        assert_eq!(t.minutes_saved, 40.0 / 5.0 / 40.0);
        assert_eq!(totals(&library, &usage, 0.0).minutes_saved, 8.0);
    }

    #[test]
    fn formats_durations_and_counts() {
        assert_eq!(format_duration(0.0), "0s");
        assert_eq!(format_duration(0.5), "30s");
        assert_eq!(format_duration(0.9999), "60s");
        assert_eq!(format_duration(1.5), "2 min");
        assert_eq!(format_duration(59.4), "59 min");
        assert_eq!(format_duration(150.0), "2.5 h");
        assert_eq!(format_duration(9.96 * 60.0), "10.0 h");
        assert_eq!(format_duration(14.0 * 60.0), "14 h");
        assert_eq!(format_duration(77.0 * 60.0), "3.2 days");
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1.0k");
        assert_eq!(format_count(1250), "1.3k");
        assert_eq!(format_count(9999), "10.0k");
        assert_eq!(format_count(34_567), "35k");
        assert_eq!(format_count(999_999), "1000k");
        assert_eq!(format_count(1_500_000), "1.5M");
        assert_eq!(format_count(-5), "-5");
    }

    #[test]
    fn relative_times() {
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        let now = tz.with_ymd_and_hms(2026, 11, 8, 12, 0, 0).unwrap().timestamp_millis();
        let ago = |secs: i64| relative_time(now - secs * 1000, now, &tz);
        assert_eq!(relative_time(0, now, &tz), "Never");
        assert_eq!(ago(59), "Just now");
        assert_eq!(ago(-59), "Just now");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(90), "1 minute ago"); // -1.5 rounds toward +∞
        assert_eq!(ago(150), "2 minutes ago");
        assert_eq!(ago(3590), "60 minutes ago");
        assert_eq!(ago(-120), "in 2 minutes");
        assert_eq!(ago(3600), "1 hour ago");
        assert_eq!(ago(86_399), "24 hours ago");
        assert_eq!(ago(-3 * 3600), "in 3 hours");
        assert_eq!(ago(86_400), "yesterday");
        assert_eq!(ago(-86_400), "tomorrow");
        assert_eq!(ago(5 * 86_400), "5 days ago");
        assert_eq!(ago(31 * 86_400), "Oct 8, 2026");
    }
}
