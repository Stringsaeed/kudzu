//! ICU date-pattern formatting.

use chrono::{DateTime, FixedOffset, TimeZone};
use kudzu_core::template::format_icu;

/// Tuesday 2024-03-05 14:07:09.123456 at UTC+05:30.
fn date() -> DateTime<FixedOffset> {
    FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap()
        .with_ymd_and_hms(2024, 3, 5, 14, 7, 9)
        .unwrap()
        .checked_add_signed(chrono::Duration::microseconds(123_456))
        .unwrap()
}

fn utc(y: i32, m: u32, d: u32, h: u32) -> DateTime<FixedOffset> {
    FixedOffset::east_opt(0)
        .unwrap()
        .with_ymd_and_hms(y, m, d, h, 0, 0)
        .unwrap()
}

#[test]
fn numeric_fields() {
    let d = date();
    assert_eq!(
        format_icu(&d, "yyyy-MM-dd HH:mm:ss", "en_US"),
        "2024-03-05 14:07:09"
    );
    assert_eq!(
        format_icu(&d, "y yy yyyyy M d D DDD", "en_US"),
        "2024 24 02024 3 5 65 065"
    );
    assert_eq!(format_icu(&d, "h hh K k H a", "en_US"), "2 02 2 14 14 PM");
    assert_eq!(
        format_icu(&d, "S SS SSS SSSSSS", "en_US"),
        "1 12 123 123456"
    );
    assert_eq!(
        format_icu(&utc(2024, 1, 1, 0), "h k K a", "en_US"),
        "12 24 0 AM"
    );
}

#[test]
fn names_and_quotes() {
    let d = date();
    assert_eq!(format_icu(&d, "EEEE, MMMM d", "en_US"), "Tuesday, March 5");
    assert_eq!(
        format_icu(&d, "EEE MMM EEEEE MMMMM EEEEEE", "en"),
        "Tue Mar T M Tu"
    );
    assert_eq!(
        format_icu(&d, "'Week' w', Q'Q QQQ QQQQ", "en_US"),
        "Week 10, Q1 Q1 1st quarter"
    );
    assert_eq!(format_icu(&d, "'o''clock' ''", "en_US"), "o'clock '");
    assert_eq!(format_icu(&d, "G GGGG", "en_US"), "AD Anno Domini");
    assert_eq!(
        format_icu(&d, "yyyy.MM.dd' at 'HH:mm", "en_US"),
        "2024.03.05 at 14:07"
    );
}

#[test]
fn zones() {
    let d = date();
    assert_eq!(
        format_icu(&d, "Z ZZZZ ZZZZZ", "en"),
        "+0530 GMT+05:30 +05:30"
    );
    assert_eq!(
        format_icu(&d, "x xx xxx X XXX", "en"),
        "+0530 +0530 +05:30 +0530 +05:30"
    );
    assert_eq!(
        format_icu(&d, "z zzzz O", "en"),
        "GMT+5:30 GMT+05:30 GMT+5:30"
    );
    let u = utc(2024, 1, 1, 0);
    assert_eq!(format_icu(&u, "X XXX x z ZZZZZ", "en"), "Z Z +00 GMT Z");
}

#[test]
fn localized_names() {
    let d = date();
    assert_eq!(format_icu(&d, "EEEE d MMMM", "fr_FR"), "mardi 5 mars");
    assert_eq!(
        format_icu(&d, "EEEE, d. MMMM", "de_DE"),
        "Dienstag, 5. März"
    );
    assert_eq!(
        format_icu(&d, "EEEE d 'de' MMMM", "es"),
        "martes 5 de marzo"
    );
    assert_eq!(format_icu(&d, "MMMM", "it_IT"), "marzo");
    assert_eq!(format_icu(&d, "EEEE", "pt_BR"), "terça-feira");
    assert_eq!(format_icu(&d, "MMM", "nl_NL"), "mrt");
    assert_eq!(format_icu(&d, "MMMM", "ar"), "مارس");
    assert_eq!(format_icu(&d, "MMMM", "xx_YY"), "March");
}

#[test]
fn local_weekday_numbers_follow_the_first_day_of_week() {
    let d = date(); // Tuesday
    assert_eq!(format_icu(&d, "e c", "en_US"), "3 3");
    assert_eq!(format_icu(&d, "e c", "de_DE"), "2 2");
    assert_eq!(format_icu(&d, "eeee ccc", "en_US"), "Tuesday Tue");
}

#[test]
fn week_based_year_and_unknown_letters() {
    let d = utc(2021, 1, 1, 0); // ISO week 53 of 2020
    assert_eq!(format_icu(&d, "YYYY-'W'ww", "en"), "2020-W53");
    assert_eq!(format_icu(&d, "yyyy jj", "en"), "2021 jj");
    assert_eq!(format_icu(&d, "5 / - :", "en"), "5 / - :");
}
