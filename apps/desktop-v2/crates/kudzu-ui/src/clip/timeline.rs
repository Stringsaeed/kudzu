//! The copy timeline: every copy of this content as a tick on a track spanning the retention
//! window (or since it was first copied when history is kept forever). `CopyTimeline` in
//! `clip-preview.tsx`.

use chrono::{DateTime, Utc};
use kudzu_core::clipboard::ClipEntry;

const DAY_MS: f64 = 86_400_000.0;

/// Where the dots and separators sit, as fractions of the track.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    /// Each copy, oldest first: its position on the track and its time.
    pub copies: Vec<(f64, DateTime<Utc>)>,
    /// Day separators; only when the window is a month or less.
    pub ticks: Vec<f64>,
    /// "7 days ago" (or the first copy's date when history is kept forever).
    pub start_label: String,
}

fn ms(t: DateTime<Utc>) -> f64 {
    t.timestamp_millis() as f64
}

pub fn timeline(e: &ClipEntry, retention_days: u32, now: DateTime<Utc>) -> Timeline {
    let times: Vec<DateTime<Utc>> = if e.timings.is_empty() {
        vec![e.copied_at]
    } else {
        e.timings.clone()
    };
    let first = ms(times[0]);
    let now_ms = ms(now);
    let span = if retention_days > 0 {
        (f64::from(retention_days) * DAY_MS).max(now_ms - first)
    } else {
        DAY_MS.max(now_ms - first)
    };
    let start = now_ms - span;
    let days = (span / DAY_MS).round().max(1.0) as usize;
    let ticks = if days <= 31 {
        (1..days).map(|i| i as f64 / days as f64).collect()
    } else {
        Vec::new()
    };
    let start_label = if retention_days > 0 {
        format!("{days} day{} ago", if days == 1 { "" } else { "s" })
    } else {
        DateTime::<Utc>::from_timestamp_millis(start as i64)
            .map(|d| d.format("%b %-d").to_string())
            .unwrap_or_default()
    };
    Timeline {
        copies: times
            .iter()
            .map(|&t| (((ms(t) - start) / span).clamp(0.0, 1.0), t))
            .collect(),
        ticks,
        start_label,
    }
}

/// Where a dot is drawn along the track: `calc(p * 96% + 2%)`.
pub fn dot_position(fraction: f64) -> f64 {
    fraction * 0.96 + 0.02
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::model::tests::{clip, now};

    fn utc_now() -> DateTime<Utc> {
        now().with_timezone(&Utc)
    }

    #[test]
    fn the_track_spans_the_retention_window_with_the_latest_copy_at_the_end() {
        let mut e = clip("x", 60, false);
        e.timings = vec![
            utc_now() - chrono::Duration::days(3),
            utc_now() - chrono::Duration::hours(1),
        ];
        let t = timeline(&e, 7, utc_now());
        assert_eq!(t.start_label, "7 days ago");
        assert_eq!(t.ticks.len(), 6);
        assert!((t.copies[0].0 - 4.0 / 7.0).abs() < 1e-3);
        assert!(t.copies[1].0 > 0.99);
    }

    #[test]
    fn a_copy_older_than_the_window_stretches_it_so_nothing_falls_off() {
        let mut e = clip("x", 60, false);
        e.timings = vec![utc_now() - chrono::Duration::days(20)];
        let t = timeline(&e, 7, utc_now());
        assert_eq!(t.start_label, "20 days ago");
        assert!(t.copies[0].0.abs() < 1e-6);
    }

    #[test]
    fn forever_retention_starts_at_the_first_copy_and_long_windows_have_no_day_ticks() {
        let mut e = clip("x", 60, false);
        e.timings = vec![utc_now() - chrono::Duration::days(90)];
        let t = timeline(&e, 0, utc_now());
        assert!(t.ticks.is_empty());
        assert_eq!(t.start_label, "Aug 13");
    }

    #[test]
    fn dots_keep_two_percent_of_margin() {
        assert!((dot_position(0.0) - 0.02).abs() < 1e-9);
        assert!((dot_position(1.0) - 0.98).abs() < 1e-9);
    }
}
