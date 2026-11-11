//! Number, duration, date and size formatting helpers for the UI.
//!
//! English, en-US, matching `lib/stats.ts`, `lib/clipboard.ts` (`formatBytes`) and
//! `lib/activity.ts` (`activityTime`). The product locale is not switched.

use chrono::{DateTime, Datelike, Timelike, Utc};

/// Compact count: `999`, `1.0k`, `1000k`, `1.0M`.
pub fn format_count(n: i64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    if n < 1_000_000 {
        let digits = if n < 10_000 { 1 } else { 0 };
        format!("{}k", to_fixed(n as f64 / 1000.0, digits))
    } else {
        format!("{}M", to_fixed(n as f64 / 1_000_000.0, 1))
    }
}

/// Duration from a number of minutes: `6s`, `1 min`, `1.0 h`, `1.0 days`.
pub fn format_duration(minutes: f64) -> String {
    if minutes < 1.0 {
        return format!("{}s", js_round(minutes * 60.0));
    }
    if minutes < 60.0 {
        return format!("{} min", js_round(minutes));
    }
    let h = minutes / 60.0;
    if h < 24.0 {
        let digits = if h < 10.0 { 1 } else { 0 };
        format!("{} h", to_fixed(h, digits))
    } else {
        format!("{} days", to_fixed(h / 24.0, 1))
    }
}

/// Relative time from an epoch-millis timestamp. `0` is `"Never"`.
pub fn relative_time(at_ms: i64, now_ms: i64) -> String {
    if at_ms == 0 {
        return "Never".into();
    }
    let diff = (at_ms - now_ms) as f64 / 1000.0;
    let abs = diff.abs();
    if abs < 60.0 {
        return "Just now".into();
    }
    if abs < 3600.0 {
        return format_rtf(js_round(diff / 60.0), Unit::Minute);
    }
    if abs < 86_400.0 {
        return format_rtf(js_round(diff / 3600.0), Unit::Hour);
    }
    if abs < 86_400.0 * 30.0 {
        return format_rtf(js_round(diff / 86_400.0), Unit::Day);
    }
    format_calendar_date(millis_to_utc(at_ms))
}

/// Byte size: `0 B`, `1.0 KB`, `977 KB`, `37 MB`.
pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let units = ["KB", "MB", "GB"];
    let mut v = n as f64 / 1024.0;
    let mut u = 0usize;
    while v >= 1024.0 && u < units.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    let digits = if v < 10.0 { 1 } else { 0 };
    format!("{} {}", to_fixed(v, digits), units[u])
}

/// Clock: time of day when `at` is today, otherwise `"Mon D, h:mm AM"`.
pub fn clock(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let time = time_of_day(at);
    if at.date_naive() == now.date_naive() {
        time
    } else {
        format!("{}, {time}", short_month_day(at))
    }
}

enum Unit {
    Minute,
    Hour,
    Day,
}

fn format_rtf(value: i64, unit: Unit) -> String {
    match (value, unit) {
        (0, Unit::Day) => "today".into(),
        (1, Unit::Day) => "tomorrow".into(),
        (-1, Unit::Day) => "yesterday".into(),
        (1, Unit::Minute) => "in 1 minute".into(),
        (-1, Unit::Minute) => "1 minute ago".into(),
        (n, Unit::Minute) if n > 1 => format!("in {n} minutes"),
        (n, Unit::Minute) => format!("{} minutes ago", n.abs()),
        (1, Unit::Hour) => "in 1 hour".into(),
        (-1, Unit::Hour) => "1 hour ago".into(),
        (n, Unit::Hour) if n > 1 => format!("in {n} hours"),
        (n, Unit::Hour) => format!("{} hours ago", n.abs()),
        (n, Unit::Day) if n > 1 => format!("in {n} days"),
        (n, Unit::Day) => format!("{} days ago", n.abs()),
    }
}

fn format_calendar_date(at: DateTime<Utc>) -> String {
    format!("{} {}, {}", month_short(at.month()), at.day(), at.year())
}

fn short_month_day(at: DateTime<Utc>) -> String {
    format!("{} {}", month_short(at.month()), at.day())
}

fn month_short(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

fn time_of_day(at: DateTime<Utc>) -> String {
    let h24 = at.hour();
    let (h12, ampm) = match h24 {
        0 => (12, "AM"),
        1..=11 => (h24, "AM"),
        12 => (12, "PM"),
        _ => (h24 - 12, "PM"),
    };
    format!("{h12}:{:02} {ampm}", at.minute())
}

fn millis_to_utc(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

/// JavaScript `Math.round`: nearest integer, ties toward +∞.
fn js_round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}

fn to_fixed(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}
