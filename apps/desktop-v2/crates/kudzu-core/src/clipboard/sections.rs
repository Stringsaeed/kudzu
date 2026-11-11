//! Selectable rows and day sections. Callers choose pinned/section policy per surface.
//! Dates use the caller's timezone; labels use the reference's English presentation.
use super::ClipEntry;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use serde::Serialize;
#[derive(Debug, Clone, Copy)]
pub struct RowOptions {
    pub pinned_first: bool,
    pub sections: bool,
}
impl RowOptions {
    pub fn history(query: &str, filter: super::query::ClipFilter) -> Self {
        Self {
            pinned_first: query
                .trim_matches(super::query::is_js_whitespace)
                .is_empty()
                && filter != super::query::ClipFilter::Pinned,
            sections: true,
        }
    }
    pub fn picker(query: &str) -> Self {
        Self {
            pinned_first: false,
            sections: query
                .trim_matches(super::query::is_js_whitespace)
                .is_empty(),
        }
    }
}
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClipRow<'a> {
    Header {
        key: String,
        label: String,
        count: usize,
    },
    Entry {
        key: uuid::Uuid,
        entry: &'a ClipEntry,
        index: usize,
        dated: bool,
    },
}
#[derive(Debug, Serialize)]
pub struct ClipRows<'a> {
    pub rows: Vec<ClipRow<'a>>,
    pub order: Vec<&'a ClipEntry>,
}
pub fn build_rows<T: TimeZone>(
    entries: &[ClipEntry],
    opts: RowOptions,
    now: DateTime<T>,
) -> ClipRows<'_> {
    let mut result = ClipRows {
        rows: Vec::new(),
        order: Vec::new(),
    };
    let pinned: Vec<_> = entries
        .iter()
        .filter(|e| opts.pinned_first && e.pinned)
        .collect();
    if !pinned.is_empty() && opts.sections {
        result.rows.push(ClipRow::Header {
            key: "h-pinned".into(),
            label: "Pinned".into(),
            count: pinned.len(),
        });
    }
    for e in pinned {
        push(&mut result, e, true);
    }
    let mut current = None;
    let mut header = None;
    for e in entries.iter().filter(|e| !opts.pinned_first || !e.pinned) {
        if opts.sections {
            let d = e.copied_at.with_timezone(&now.timezone()).date_naive();
            if current != Some(d) {
                current = Some(d);
                header = Some(result.rows.len());
                result.rows.push(ClipRow::Header {
                    key: format!("h-{}-{}-{}", d.year(), d.month0(), d.day()),
                    label: day_label(e.copied_at, &now),
                    count: 0,
                });
            }
            if let Some(ClipRow::Header { count, .. }) = header.map(|i| &mut result.rows[i]) {
                *count += 1;
            }
        }
        push(&mut result, e, !opts.sections);
    }
    result
}
fn push<'a>(result: &mut ClipRows<'a>, entry: &'a ClipEntry, dated: bool) {
    result.rows.push(ClipRow::Entry {
        key: entry.id,
        entry,
        index: result.order.len(),
        dated,
    });
    result.order.push(entry);
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
    let midnight = now
        .timezone()
        .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
        .earliest();
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
    let date = format!(
        "{}, {} {}",
        d.weekday(),
        super::facts::month(d.month()),
        d.day()
    );
    if d.year() != now.year() {
        format!("{date}, {}", d.year())
    } else {
        date
    }
}
