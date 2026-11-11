//! Clock and timezone-explicit English date, time and expiry labels.
use super::ClipEntry;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

pub(super) fn month(m: u32) -> &'static str {
    [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ][(m - 1) as usize]
}
fn clock<T: TimeZone>(d: &DateTime<T>, seconds: bool) -> String {
    let h = d.hour() % 12;
    let h = if h == 0 { 12 } else { h };
    let am = if d.hour() < 12 { "AM" } else { "PM" };
    if seconds {
        format!("{h}:{:02}:{:02} {am}", d.minute(), d.second())
    } else {
        format!("{h}:{:02} {am}", d.minute())
    }
}
pub fn clip_time<T: TimeZone>(at: DateTime<Utc>, now: DateTime<T>, dated: bool) -> String {
    let diff = (now
        .clone()
        .signed_duration_since(at)
        .num_milliseconds()
        .max(0) as f64)
        / 1000.0;
    if diff < 45.0 {
        return "Just now".into();
    }
    if diff < 3600.0 {
        return format!("{}m ago", (diff / 60.0).round());
    }
    let d = at.with_timezone(&now.timezone());
    let time = clock(&d, false);
    if !dated {
        return time;
    }
    let day = day_label(at, &now);
    match day.as_str() {
        "Today" => time,
        "Yesterday" => day,
        _ if diff < 6.0 * 86400.0 => d.weekday().to_string(),
        _ => format!("{} {}", month(d.month()), d.day()),
    }
}
pub fn exact_time<T: TimeZone>(at: DateTime<Utc>, now: DateTime<T>) -> String {
    let day = day_label(at, &now);
    let d = at.with_timezone(&now.timezone());
    if day == "Today" || day == "Yesterday" {
        format!("{day} at {}", clock(&d, true))
    } else {
        format!(
            "{} {}, {}, {}",
            month(d.month()),
            d.day(),
            d.year(),
            clock(&d, false)
        )
    }
}
pub fn expires_in<T: TimeZone>(e: &ClipEntry, days: u32, now: DateTime<T>) -> String {
    if e.pinned {
        return "Never (pinned)".into();
    }
    if days == 0 {
        return "When history is full".into();
    }
    let hours = (e.copied_at.signed_duration_since(now).num_milliseconds() as f64) / 3_600_000.0
        + days as f64 * 24.0;
    if hours <= 1.0 {
        "Within the hour".into()
    } else if hours < 36.0 {
        format!("In {} hours", hours.round())
    } else {
        format!("In {} days", (hours / 24.0).round())
    }
}

pub fn day_label<T: TimeZone>(at: DateTime<Utc>, now: &DateTime<T>) -> String {
    let d = at.with_timezone(&now.timezone());
    let today = now.date_naive();
    if d.date_naive() == today {
        return "Today".into();
    }
    if Some(d.date_naive()) == today.pred_opt() {
        return "Yesterday".into();
    }
    let start = today.and_hms_opt(0, 0, 0).unwrap();
    let zone = now.timezone();
    let local = zone.from_local_datetime(&start);
    let midnight = local
        .clone()
        .earliest()
        .or_else(|| local.latest())
        .or_else(|| {
            // Chrono returns None for BOTH earliest/latest in a DST gap. Resolve to the
            // first valid local minute that day, as JS normalizes a missing midnight.
            (1..1440).find_map(|minutes| {
                let local = zone.from_local_datetime(&(start + chrono::Duration::minutes(minutes)));
                local.clone().earliest().or_else(|| local.latest())
            })
        });
    if midnight.is_some_and(|m| {
        (m.signed_duration_since(at).num_milliseconds() as f64) / 86_400_000.0 < 6.0
    }) {
        return [
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
            "Sunday",
        ][d.weekday().num_days_from_monday() as usize]
            .into();
    }
    let date = format!("{}, {} {}", d.weekday(), month(d.month()), d.day());
    if d.year() != now.year() {
        format!("{date}, {}", d.year())
    } else {
        date
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, LocalResult, NaiveDate, NaiveDateTime};

    // Deterministic timezone with a one-hour midnight gap on the fixture date.
    #[derive(Clone)]
    struct MidnightGap;
    impl TimeZone for MidnightGap {
        type Offset = FixedOffset;
        fn from_offset(_: &Self::Offset) -> Self {
            Self
        }
        fn offset_from_local_date(&self, _: &NaiveDate) -> LocalResult<Self::Offset> {
            LocalResult::Single(FixedOffset::east_opt(3600).unwrap())
        }
        fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> LocalResult<Self::Offset> {
            if local.date() == NaiveDate::from_ymd_opt(2026, 11, 11).unwrap() && local.hour() == 0 {
                LocalResult::None
            } else {
                self.offset_from_local_date(&local.date())
            }
        }
        fn offset_from_utc_date(&self, _: &NaiveDate) -> Self::Offset {
            FixedOffset::east_opt(3600).unwrap()
        }
        fn offset_from_utc_datetime(&self, _: &NaiveDateTime) -> Self::Offset {
            FixedOffset::east_opt(3600).unwrap()
        }
    }
    #[test]
    fn midnight_gap_keeps_recent_weekday_label() {
        let now: DateTime<Utc> = "2026-11-11T12:00:00Z".parse().unwrap();
        let at = "2026-11-09T12:00:00Z".parse().unwrap();
        let local_now = now.with_timezone(&MidnightGap);
        let midnight =
            MidnightGap.from_local_datetime(&local_now.date_naive().and_hms_opt(0, 0, 0).unwrap());
        assert!(midnight.earliest().is_none());
        assert!(midnight.latest().is_none());
        assert_eq!(day_label(at, &local_now), "Monday");
    }
}
