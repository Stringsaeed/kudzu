//! Selectable rows and day sections. Callers choose pinned/section policy per surface.
//! Dates use the caller's timezone; labels use the reference's English presentation.
pub use super::time::day_label;
use super::{whitespace::is_js_whitespace, ClipEntry};
use chrono::{DateTime, Datelike, TimeZone};
use serde::Serialize;
#[derive(Debug, Clone, Copy)]
pub struct RowOptions {
    pub pinned_first: bool,
    pub sections: bool,
}
impl RowOptions {
    pub fn history(query: &str, filter: super::query::ClipFilter) -> Self {
        Self {
            pinned_first: query.trim_matches(is_js_whitespace).is_empty()
                && filter != super::query::ClipFilter::Pinned,
            sections: true,
        }
    }
    pub fn picker(query: &str) -> Self {
        Self {
            pinned_first: false,
            sections: query.trim_matches(is_js_whitespace).is_empty(),
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
pub fn build_rows<'a, T: TimeZone>(
    entries: impl IntoIterator<Item = &'a ClipEntry>,
    opts: RowOptions,
    now: DateTime<T>,
) -> ClipRows<'a> {
    let entries: Vec<_> = entries.into_iter().collect();
    let mut result = ClipRows {
        rows: Vec::new(),
        order: Vec::new(),
    };
    let pinned: Vec<_> = entries
        .iter()
        .filter(|e| opts.pinned_first && e.pinned)
        .copied()
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
    for e in entries
        .into_iter()
        .filter(|e| !opts.pinned_first || !e.pinned)
    {
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
