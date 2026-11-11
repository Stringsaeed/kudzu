//! Fixture tests for count, duration, relative-date, byte-size and clock labels.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use kudzu_core::fmt;
use serde::Deserialize;

fn load<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatsFixture {
    counts: Vec<CountCase>,
    durations: Vec<DurationCase>,
    relative: Vec<RelativeCase>,
    clock: Vec<ClockCase>,
    relative_local: Vec<RelativeCase>,
    clock_local: Vec<ClockCase>,
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
#[serde(rename_all = "camelCase")]
struct RelativeCase {
    at: i64,
    now: i64,
    offset_minutes: i32,
    output: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClockCase {
    at: String,
    now: String,
    offset_minutes: i32,
    output: String,
}

#[derive(Deserialize)]
struct ByteCase {
    n: u64,
    label: String,
}

fn parse_utc(iso: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(iso)
        .unwrap()
        .with_timezone(&Utc)
}

#[test]
fn counts_match_stats_fixture() {
    let stats: StatsFixture = load("stats.json");
    assert!(!stats.counts.is_empty());
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
    assert!(!stats.durations.is_empty());
    for case in &stats.durations {
        assert_eq!(
            fmt::format_duration(case.input),
            case.output,
            "format_duration({})",
            case.input
        );
    }
}

fn assert_relative(cases: &[RelativeCase], label: &str) {
    assert!(!cases.is_empty(), "{label} is empty");
    for case in cases {
        assert_eq!(
            fmt::relative_time(case.at, case.now, case.offset_minutes),
            case.output,
            "{label} at={} offset={}",
            case.at,
            case.offset_minutes
        );
    }
}

fn assert_clock(cases: &[ClockCase], label: &str) {
    assert!(!cases.is_empty(), "{label} is empty");
    for case in cases {
        let got = fmt::clock(
            parse_utc(&case.at),
            parse_utc(&case.now),
            case.offset_minutes,
        );
        assert_eq!(
            got, case.output,
            "{label} at={} offset={}",
            case.at, case.offset_minutes
        );
    }
}

#[test]
fn relative_dates_match_stats_fixture() {
    let stats: StatsFixture = load("stats.json");
    assert_relative(&stats.relative, "relative");
    assert_relative(&stats.relative_local, "relativeLocal");
}

#[test]
fn clock_matches_stats_fixture() {
    let stats: StatsFixture = load("stats.json");
    assert_clock(&stats.clock, "clock");
    assert_clock(&stats.clock_local, "clockLocal");
}

#[test]
fn byte_sizes_match_clipboard_fixture() {
    let cases: Vec<ByteCase> = load("fmt-bytes.json");
    assert!(!cases.is_empty());
    for case in &cases {
        assert_eq!(
            fmt::format_bytes(case.n),
            case.label,
            "format_bytes({})",
            case.n
        );
    }
}
