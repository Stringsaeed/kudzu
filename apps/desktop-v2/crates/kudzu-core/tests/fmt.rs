//! Fixture tests for count, duration, relative-date, byte-size and clock labels.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use kudzu_core::fmt;
use serde::Deserialize;

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Deserialize)]
struct StatsFixture {
    counts: Vec<CountCase>,
    durations: Vec<DurationCase>,
    relative: Vec<RelativeCase>,
}

#[derive(Deserialize)]
struct CountCase {
    input: i64,
    output: String,
}

#[derive(Deserialize)]
struct DurationCase {
    input: f64,
    output: String,
}

#[derive(Deserialize)]
struct RelativeCase {
    age: i64,
    output: String,
}

#[derive(Deserialize)]
struct ByteCase {
    n: u64,
    label: String,
}

fn fixture_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap()
}

#[test]
fn counts_match_stats_fixture() {
    let stats: StatsFixture = load("stats.json");
    assert_eq!(stats.counts.len(), 6);
    for case in &stats.counts {
        assert_eq!(
            fmt::format_count(case.input),
            case.output,
            "format_count({})",
            case.input
        );
    }
}

#[test]
fn durations_match_stats_fixture() {
    let stats: StatsFixture = load("stats.json");
    assert_eq!(stats.durations.len(), 6);
    for case in &stats.durations {
        assert_eq!(
            fmt::format_duration(case.input),
            case.output,
            "format_duration({})",
            case.input
        );
    }
}

#[test]
fn relative_dates_match_stats_fixture() {
    let stats: StatsFixture = load("stats.json");
    assert_eq!(stats.relative.len(), 5);
    let now_ms = fixture_now().timestamp_millis();
    for case in &stats.relative {
        let at_ms = now_ms - case.age;
        assert_eq!(
            fmt::relative_time(at_ms, now_ms),
            case.output,
            "relative_time(age={})",
            case.age
        );
    }
}

#[test]
fn byte_sizes_match_clipboard_fixture() {
    let cases: Vec<ByteCase> = load("fmt-bytes.json");
    assert_eq!(cases.len(), 5);
    for case in &cases {
        assert_eq!(
            fmt::format_bytes(case.n),
            case.label,
            "format_bytes({})",
            case.n
        );
    }
}

#[test]
fn clock_is_time_today_and_date_before() {
    let now = fixture_now();
    assert_eq!(fmt::clock(now, now), "12:00 PM");
    assert_eq!(
        fmt::clock(Utc.with_ymd_and_hms(2026, 11, 11, 0, 5, 0).unwrap(), now),
        "12:05 AM"
    );
    assert_eq!(
        fmt::clock(Utc.with_ymd_and_hms(2026, 11, 10, 15, 30, 0).unwrap(), now),
        "Nov 10, 3:30 PM"
    );
}

#[test]
fn relative_time_zero_is_never() {
    assert_eq!(
        fmt::relative_time(0, fixture_now().timestamp_millis()),
        "Never"
    );
}
