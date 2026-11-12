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
fn to_fixed_rounds_exact_ties_up() {
    assert_eq!(fmt::format_count(1_250), "1.3k");
    assert_eq!(fmt::format_count(12_500), "13k");
    assert_eq!(fmt::format_duration(2.25 * 60.0), "2.3 h");
    assert_eq!(fmt::to_fixed(1.25, 1), "1.3");
    assert_eq!(fmt::to_fixed(12.5, 0), "13");
    assert_eq!(fmt::to_fixed(2.25, 1), "2.3");
}

/// `Number#toFixed` rounds the double's exact decimal value, with exact ties going to the
/// larger magnitude, and keeps the sign. Expected values are what JavaScript prints.
#[test]
fn to_fixed_matches_javascript_on_exact_values_and_signs() {
    for (value, digits, js) in [
        (8.345, 2, "8.35"), // 8.345 is 8.3450000000000006394884621840901672840118408203125
        (1.005, 2, "1.00"), // 1.00499999999999989341858963598497211933135986328125
        (2.675, 2, "2.67"), // 2.67499999999999982236431605997495353221893310546875
        (1.45, 1, "1.4"),   // 1.4499999999999999555910790149937383830547332763671875
        (0.15, 1, "0.1"),   // 0.1499999999999999944488848768742172978818416595458984375
        (1.95, 1, "1.9"),   // 1.9499999999999999555910790149937383830547332763671875
        (0.5, 0, "1"),
        (2.5, 0, "3"),
        (-1.25, 1, "-1.3"),
        (-2.5, 0, "-3"),
        (-0.04, 1, "-0.0"),
        (0.0, 2, "0.00"),
        (9.995, 2, "9.99"),  // 9.9949999999999992184029906638897955417633056640625
        (99.95, 1, "100.0"), // 99.9500000000000028421709430404007434844970703125
        (1234.5678, 3, "1234.568"),
    ] {
        assert_eq!(
            fmt::to_fixed(value, digits),
            js,
            "toFixed({value}, {digits})"
        );
    }
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
