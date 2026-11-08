//! Clipboard history on the client (`lib/clipboard.ts`), pure parts: filters, search with
//! `app:` and `is:` operators, match highlighting, day sections, titles and formatting, and
//! the newest-first list upkeep the store did. Times take `now` (ms since the epoch) and a
//! time zone; text is en-US as the web UI showed it.

use std::collections::HashMap;

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use uuid::Uuid;

use crate::clipboard::{ClipEntry, ClipKind};

use super::{js, url};

// MARK: Secrets

/// Whether an entry counts as a secret: the user's correction, else Kudzu's detection.
pub fn is_secret(e: &ClipEntry) -> bool {
    e.is_secret()
}

/// "Looks like a password", "Looks like an API key", or "Marked as secret" for one the user hid.
pub fn secret_reason(e: &ClipEntry) -> String {
    match e.secret {
        Some(kind) => {
            let label = kind.label();
            let an = label.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) || label.get(..3).is_some_and(|p| p.eq_ignore_ascii_case("api"));
            format!("Looks like {} {label}", if an { "an" } else { "a" })
        }
        None => "Marked as secret".into(),
    }
}

// MARK: Kinds and filters

/// A sidebar / toolbar filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipFilter {
    All,
    Pinned,
    Kind(ClipKind),
}

pub fn kind_label(kind: ClipKind) -> &'static str {
    match kind {
        ClipKind::Text => "Text",
        ClipKind::Code => "Code",
        ClipKind::Link => "Link",
        ClipKind::Color => "Color",
        ClipKind::Image => "Image",
        ClipKind::File => "File",
        ClipKind::Other => "Other",
    }
}

/// One filter choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilterOption {
    pub value: ClipFilter,
    pub label: &'static str,
    pub short: &'static str,
}

/// Filters in menu order.
pub const FILTERS: [FilterOption; 8] = [
    FilterOption { value: ClipFilter::All, label: "All", short: "All" },
    FilterOption { value: ClipFilter::Pinned, label: "Pinned", short: "Pinned" },
    FilterOption { value: ClipFilter::Kind(ClipKind::Text), label: "Text", short: "Text" },
    FilterOption { value: ClipFilter::Kind(ClipKind::Image), label: "Images", short: "Images" },
    FilterOption { value: ClipFilter::Kind(ClipKind::Link), label: "Links", short: "Links" },
    FilterOption { value: ClipFilter::Kind(ClipKind::File), label: "Files", short: "Files" },
    FilterOption { value: ClipFilter::Kind(ClipKind::Code), label: "Code", short: "Code" },
    FilterOption { value: ClipFilter::Kind(ClipKind::Color), label: "Colors", short: "Colors" },
];

/// Whether an entry belongs under a filter ("Text" also covers other data).
pub fn in_filter(e: &ClipEntry, f: ClipFilter) -> bool {
    match f {
        ClipFilter::All => true,
        ClipFilter::Pinned => e.pinned,
        ClipFilter::Kind(ClipKind::Text) => matches!(e.kind, ClipKind::Text | ClipKind::Other),
        ClipFilter::Kind(k) => e.kind == k,
    }
}

// MARK: Search

/// Lowercased text, file names, colour and kind label: what a query term must appear in.
pub fn haystack(e: &ClipEntry) -> String {
    let files = e.files.iter().map(|f| basename(f)).collect::<Vec<_>>().join(" ");
    js::lower(&[e.text.as_deref().unwrap_or(""), &files, e.color.as_deref().unwrap_or(""), kind_label(e.kind)].join("\n"))
}

/// Haystacks cached per entry id and content hash (the UI cached by object identity).
#[derive(Debug, Clone, Default)]
pub struct HaystackCache {
    map: HashMap<Uuid, (String, String)>,
}

impl HaystackCache {
    pub fn get(&mut self, e: &ClipEntry) -> &str {
        let slot = self.map.entry(e.id).or_insert_with(|| (e.hash.clone(), haystack(e)));
        if slot.0 != e.hash {
            *slot = (e.hash.clone(), haystack(e));
        }
        &slot.1
    }

    /// Drops entries no longer in the history.
    pub fn retain(&mut self, keep: impl Fn(&Uuid) -> bool) {
        self.map.retain(|id, _| keep(id));
    }
}

/// A clipboard query split into terms and operators.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedQuery {
    pub terms: Vec<String>,
    /// `app:…` (the last one wins); an empty `app:` filters nothing.
    pub app: Option<String>,
    /// `is:…` with a known kind alias (the last one wins).
    pub kind: Option<ClipKind>,
}

fn kind_alias(s: &str) -> Option<ClipKind> {
    Some(match s {
        "text" => ClipKind::Text,
        "code" => ClipKind::Code,
        "link" | "links" | "url" => ClipKind::Link,
        "color" | "colour" => ClipKind::Color,
        "image" | "images" | "img" => ClipKind::Image,
        "file" | "files" => ClipKind::File,
        "other" => ClipKind::Other,
        _ => return None,
    })
}

/// `"invoice app:mail is:link"` → terms + an app filter + a kind filter. Lowercases
/// everything; unknown `is:` values stay terms.
pub fn parse_query(q: &str) -> ParsedQuery {
    let mut parsed = ParsedQuery::default();
    let lower = js::lower(q);
    for raw in js::split_spaces(&lower) {
        if raw.is_empty() {
            continue;
        }
        if let Some(app) = raw.strip_prefix("app:") {
            parsed.app = Some(app.to_string());
        } else if let Some(kind) = raw.strip_prefix("is:").and_then(kind_alias) {
            parsed.kind = Some(kind);
        } else {
            parsed.terms.push(raw.to_string());
        }
    }
    parsed
}

fn matches_query(e: &ClipEntry, filter: ClipFilter, q: &ParsedQuery, hay: impl FnOnce() -> String) -> bool {
    if !in_filter(e, filter) {
        return false;
    }
    if q.kind.is_some_and(|k| e.kind != k) {
        return false;
    }
    if let Some(app) = q.app.as_deref().filter(|a| !a.is_empty()) {
        let Some(source) = &e.source else { return false };
        if !js::lower(&source.name).contains(app) && !js::lower(&source.bundle_id).contains(app) {
            return false;
        }
    }
    if q.terms.is_empty() {
        return true;
    }
    let h = hay();
    q.terms.iter().all(|t| h.contains(t.as_str()))
}

/// Entries matching a filter and query, in history order.
pub fn search_clips<'a>(entries: &'a [ClipEntry], filter: ClipFilter, query: &str) -> Vec<&'a ClipEntry> {
    let q = parse_query(query);
    entries.iter().filter(|e| matches_query(e, filter, &q, || haystack(e))).collect()
}

/// [`search_clips`] reusing cached haystacks.
pub fn search_clips_cached<'a>(entries: &'a [ClipEntry], filter: ClipFilter, query: &str, cache: &mut HaystackCache) -> Vec<&'a ClipEntry> {
    let q = parse_query(query);
    entries.iter().filter(|e| matches_query(e, filter, &q, || cache.get(e).to_string())).collect()
}

/// `[start, end)` UTF-16 ranges of `terms` in the lowercased `text`, merged, for
/// highlighting. At most 64 hits are collected.
pub fn match_ranges(text: &str, terms: &[String]) -> Vec<(usize, usize)> {
    if terms.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let lower = js::utf16(&js::lower(text));
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for t in terms {
        let t = js::utf16(t);
        let mut i = js::index_of16(&lower, &t, 0);
        while let Some(at) = i {
            if ranges.len() >= 64 {
                break;
            }
            ranges.push((at, at + t.len()));
            i = js::index_of16(&lower, &t, at + t.len());
        }
    }
    ranges.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in ranges {
        match merged.last_mut() {
            Some(last) if r.0 <= last.1 => last.1 = last.1.max(r.1),
            _ => merged.push(r),
        }
    }
    merged
}

// MARK: Rows and sections

/// A virtual-list row.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipRow<'a> {
    Header { key: String, label: String, count: usize },
    /// `dated`: the row isn't under a day heading, so its time needs the day too.
    Entry { key: String, entry: &'a ClipEntry, index: usize, dated: bool },
}

fn ms(d: &DateTime<Utc>) -> i64 {
    d.timestamp_millis()
}

/// `${year}-${month0}-${date}` in local time, as the UI keyed days.
fn day_key<Tz: TimeZone>(d: &DateTime<Tz>) -> String {
    format!("{}-{}-{}", d.year(), d.month0(), d.day())
}

fn local_midnight<Tz: TimeZone>(date: NaiveDate, tz: &Tz) -> i64 {
    let midnight = date.and_hms_opt(0, 0, 0).unwrap_or_default();
    tz.from_local_datetime(&midnight)
        .earliest()
        .or_else(|| tz.from_local_datetime(&(midnight + Duration::hours(1))).earliest())
        .map_or_else(|| midnight.and_utc().timestamp_millis(), |d| d.timestamp_millis())
}

/// Section label for a copy time: Today, Yesterday, a weekday within the last 6 days, or a
/// date ("Wed, Oct 28", with the year when it isn't this year).
pub fn day_label<Tz: TimeZone>(ms: i64, now_ms: i64, tz: &Tz) -> String {
    let d = js::local(ms, tz);
    let today = js::local(now_ms, tz);
    if day_key(&d) == day_key(&today) {
        return "Today".into();
    }
    let yesterday = today.date_naive().pred_opt().unwrap_or(today.date_naive());
    if d.date_naive() == yesterday {
        return "Yesterday".into();
    }
    let days = (local_midnight(today.date_naive(), tz) - ms) as f64 / 86_400_000.0;
    if days < 6.0 {
        return js::weekday_long(&d).into();
    }
    let base = format!("{}, {}", js::weekday_short(&d), js::month_day(&d));
    if d.year() != today.year() {
        format!("{base}, {}", d.year())
    } else {
        base
    }
}

/// Options for [`build_rows`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowOptions {
    /// Pinned entries first (when not searching).
    pub pinned_first: bool,
    /// Day headers.
    pub sections: bool,
}

/// Flattens entries into virtual-list rows: pinned first (when asked), then one section per
/// local day. `index` is the entry's position among selectable rows; `order` lists them.
pub fn build_rows<'a, Tz: TimeZone>(entries: &[&'a ClipEntry], opts: RowOptions, now_ms: i64, tz: &Tz) -> (Vec<ClipRow<'a>>, Vec<&'a ClipEntry>) {
    let pinned: Vec<&ClipEntry> = if opts.pinned_first { entries.iter().copied().filter(|e| e.pinned).collect() } else { Vec::new() };
    let rest: Vec<&ClipEntry> = if opts.pinned_first { entries.iter().copied().filter(|e| !e.pinned).collect() } else { entries.to_vec() };
    let mut rows = Vec::new();
    let mut order: Vec<&ClipEntry> = Vec::new();
    let mut push = |rows: &mut Vec<ClipRow<'a>>, e: &'a ClipEntry, dated: bool| {
        rows.push(ClipRow::Entry { key: e.id.to_string(), entry: e, index: order.len(), dated });
        order.push(e);
    };
    if !pinned.is_empty() {
        if opts.sections {
            rows.push(ClipRow::Header { key: "h-pinned".into(), label: "Pinned".into(), count: pinned.len() });
        }
        for e in pinned {
            push(&mut rows, e, true);
        }
    }
    let mut current = String::new();
    let mut header: Option<usize> = None;
    for e in rest {
        if opts.sections {
            let t = ms(&e.copied_at);
            let k = day_key(&js::local(t, tz));
            if k != current {
                rows.push(ClipRow::Header { key: format!("h-{k}"), label: day_label(t, now_ms, tz), count: 0 });
                current = k;
                header = Some(rows.len() - 1);
            }
            if let Some(ClipRow::Header { count, .. }) = header.and_then(|h| rows.get_mut(h)) {
                *count += 1;
            }
        }
        push(&mut rows, e, !opts.sections);
    }
    (rows, order)
}

// MARK: Formatting

fn strip_trailing_slashes(path: &str) -> &str {
    path.trim_end_matches('/')
}

/// Last path component (`/a/b/` → `b`).
pub fn basename(path: &str) -> String {
    let p = strip_trailing_slashes(path);
    let name = p.rfind('/').map_or(p, |i| &p[i + 1..]);
    (if name.is_empty() { p } else { name }).to_string()
}

/// Parent folder (`/a/b` → `/a`, `/a` → `/`).
pub fn dirname(path: &str) -> String {
    let p = strip_trailing_slashes(path);
    match p.rfind('/') {
        Some(i) if i > 0 => p[..i].to_string(),
        _ => "/".into(),
    }
}

/// `/Users/me/Documents` → `~/Documents`.
pub fn tildify(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("/Users/") {
        let user_len = rest.find('/').unwrap_or(rest.len());
        if user_len > 0 {
            return format!("~{}", &rest[user_len..]);
        }
    }
    path.to_string()
}

/// "512 B", "1.5 KB", "12 MB", "3.0 GB".
pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    const UNITS: [&str; 3] = ["KB", "MB", "GB"];
    let mut v = n as f64 / 1024.0;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{} {}", js::to_fixed(v, if v < 10.0 { 1 } else { 0 }), UNITS[u])
}

/// Time of day ("2:04 PM"), or "Just now" / "5m ago" within the last hour. With `dated`,
/// other days get their day instead ("Yesterday", "Mon", "Oct 28").
pub fn clip_time<Tz: TimeZone>(at: &DateTime<Utc>, now_ms: i64, dated: bool, tz: &Tz) -> String {
    let t = ms(at);
    let diff = (now_ms - t).max(0) as f64 / 1000.0;
    if diff < 45.0 {
        return "Just now".into();
    }
    if diff < 3600.0 {
        return format!("{}m ago", js::number_to_string(js::round(diff / 60.0)));
    }
    let local = js::local(t, tz);
    let time = js::time_short(&local);
    if !dated {
        return time;
    }
    match day_label(t, now_ms, tz).as_str() {
        "Today" => time,
        "Yesterday" => "Yesterday".into(),
        _ if diff < 6.0 * 86_400.0 => js::weekday_short(&local).into(),
        _ => js::month_day(&local),
    }
}

/// "Today at 2:04:11 PM", "Yesterday at …", or "Oct 28, 2026, 2:04 PM".
pub fn exact_time<Tz: TimeZone>(at: &DateTime<Utc>, now_ms: i64, tz: &Tz) -> String {
    let t = ms(at);
    let local = js::local(t, tz);
    let day = day_label(t, now_ms, tz);
    if day == "Today" || day == "Yesterday" {
        return format!("{day} at {}", js::time_seconds(&local));
    }
    format!("{}, {}", js::date_medium(&local), js::time_short(&local))
}

/// "Never (pinned)", "When history is full", "Within the hour", "In 5 hours", "In 6 days".
pub fn expires_in(e: &ClipEntry, retention_days: u32, now_ms: i64) -> String {
    if e.pinned {
        return "Never (pinned)".into();
    }
    if retention_days == 0 {
        return "When history is full".into();
    }
    let left = ms(&e.copied_at) + i64::from(retention_days) * 86_400_000 - now_ms;
    if left <= 3_600_000 {
        return "Within the hour".into();
    }
    let hours = left as f64 / 3_600_000.0;
    if hours < 36.0 {
        return format!("In {} hours", js::number_to_string(js::round(hours)));
    }
    format!("In {} days", js::number_to_string(js::round(hours / 24.0)))
}

/// "for 1 week", "for 3 months", "until you clear it".
pub fn retention_label(days: u32) -> String {
    match days {
        0 => "until you clear it".into(),
        1 => "for 1 day".into(),
        365 => "for 1 year".into(),
        d if d % 365 == 0 => format!("for {} years", d / 365),
        30 => "for 1 month".into(),
        d if d % 30 == 0 => format!("for {} months", d / 30),
        7 => "for 1 week".into(),
        d if d % 7 == 0 => format!("for {} weeks", d / 7),
        d => format!("for {d} days"),
    }
}

/// Retention choices in menu order (0 = forever).
pub const RETENTION_OPTIONS: [(u32, &str); 6] = [(1, "1 day"), (7, "1 week"), (30, "1 month"), (90, "3 months"), (365, "1 year"), (0, "Forever")];

/// The first non-blank line, whitespace collapsed.
fn first_line(text: &str) -> String {
    for line in text.split('\n') {
        let t = js::trim(line);
        if !t.is_empty() {
            return js::collapse_spaces(t);
        }
    }
    js::trim(text).to_string()
}

fn link_parts(link: &str) -> (String, String) {
    let www = link.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("www."));
    let input = if www { format!("https://{link}") } else { link.to_string() };
    let parsed = url::parse(&input).and_then(|u| {
        if u.scheme == "mailto" {
            return Some((u.pathname, "Email address".to_string()));
        }
        let host = u.host.strip_prefix("www.").unwrap_or(&u.host).to_string();
        let decoded = url::decode_uri(&format!("{}{}", u.pathname, u.search))?;
        Some((host, decoded.strip_suffix('/').unwrap_or(&decoded).to_string()))
    });
    parsed.unwrap_or_else(|| (link.to_string(), String::new()))
}

/// Two short lines describing an entry, for list rows and accessible names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipDescription {
    pub title: String,
    pub subtitle: String,
}

pub fn describe(e: &ClipEntry) -> ClipDescription {
    let text = e.text.as_deref().unwrap_or("");
    let (title, subtitle) = match e.kind {
        ClipKind::Link => {
            let link = js::trim(text);
            let (host, rest) = link_parts(link);
            let subtitle = if rest.is_empty() { link.to_string() } else { rest };
            (host, subtitle)
        }
        ClipKind::Color => (e.color.as_deref().or(e.text.as_deref()).unwrap_or("").to_uppercase(), "Color".into()),
        ClipKind::Image => (
            e.image.map_or_else(|| "Image".into(), |i| format!("Image  {} × {}", i.width, i.height)),
            format!("{} · {}", image_format(e), format_bytes(e.size)),
        ),
        ClipKind::File => {
            let first = e.files.first().map(String::as_str).unwrap_or("");
            let title = if e.files.len() > 1 { format!("{} and {} more", basename(first), e.files.len() - 1) } else { basename(first) };
            (title, tildify(&dirname(first)))
        }
        ClipKind::Other => (
            e.source.as_ref().map_or_else(|| "Data".into(), |s| format!("Content from {}", s.name)),
            format!("{} format{} · {}", e.types.len(), if e.types.len() == 1 { "" } else { "s" }, format_bytes(e.size)),
        ),
        ClipKind::Text | ClipKind::Code => {
            let lines: Vec<&str> = text.split('\n').map(js::trim).filter(|l| !l.is_empty()).collect();
            let subtitle = if lines.len() > 1 {
                js::collapse_spaces(lines[1])
            } else {
                format!("{} character{}", js::group_thousands(e.text_len as i64), if e.text_len == 1 { "" } else { "s" })
            };
            (first_line(text), subtitle)
        }
    };
    ClipDescription { title, subtitle }
}

/// "PNG", "JPEG", "TIFF", "GIF", "HEIC", "WebP", or "Image", from the first image type.
pub fn image_format(e: &ClipEntry) -> &'static str {
    const NAMES: [&str; 6] = ["png", "jpeg", "tiff", "gif", "heic", "webp"];
    let t = e.types.iter().find(|x| NAMES.iter().any(|n| x.contains(n))).map(String::as_str).unwrap_or("");
    if t.contains("png") {
        "PNG"
    } else if t.contains("jpeg") {
        "JPEG"
    } else if t.contains("tiff") {
        "TIFF"
    } else if t.contains("gif") {
        "GIF"
    } else if t.contains("heic") {
        "HEIC"
    } else if t.contains("webp") {
        "WebP"
    } else {
        "Image"
    }
}

/// Accessible name for a row: "Link, github.com, copied Today at 2:04:11 PM, from Safari, pinned".
pub fn spoken_label<Tz: TimeZone>(e: &ClipEntry, concealed: bool, now_ms: i64, tz: &Tz) -> String {
    let copied = format!("copied {}", exact_time(&e.copied_at, now_ms, tz));
    let mut parts = if concealed {
        vec!["Hidden secret".to_string(), js::lower(&secret_reason(e)), copied]
    } else {
        vec![kind_label(e.kind).to_string(), js::slice16(&describe(e).title, 0, 120), copied]
    };
    if let Some(s) = &e.source {
        parts.push(format!("from {}", s.name));
    }
    if e.copy_count > 1 {
        parts.push(format!("{} times", e.copy_count));
    }
    if e.pinned {
        parts.push("pinned".into());
    }
    parts.join(", ")
}

/// Runs of non-whitespace.
pub fn word_count(text: &str) -> usize {
    js::split_spaces(text).into_iter().filter(|w| !w.is_empty()).count()
}

/// Whether an entry can be turned into a snippet / copied as plain text.
pub fn has_text(e: &ClipEntry) -> bool {
    matches!(e.kind, ClipKind::Text | ClipKind::Code | ClipKind::Link | ClipKind::Color)
}

/// The full text when the excerpt already holds it (`None` means fetch it from the backend).
/// Compares `text_len` with the excerpt's UTF-16 length, as the UI did.
pub fn inline_full_text(e: &ClipEntry) -> Option<&str> {
    e.text.as_deref().filter(|t| e.text_len <= js::len16(t))
}

// MARK: List upkeep

/// Sorts newest first by `copied_at` (stable for equal times).
pub fn sort_newest(list: &mut [ClipEntry]) {
    list.sort_by_key(|e| std::cmp::Reverse(e.copied_at));
}

/// Inserts or replaces `entry`, keeping newest-first order (after entries copied later, before
/// those copied at the same time or earlier).
pub fn upsert(list: &mut Vec<ClipEntry>, entry: ClipEntry) {
    list.retain(|e| e.id != entry.id);
    let i = list.iter().position(|e| e.copied_at <= entry.copied_at).unwrap_or(list.len());
    list.insert(i, entry);
}

#[cfg(test)]
mod tests {
    use chrono::FixedOffset;

    use super::*;
    use crate::clipboard::{ClipSource, ImageSize};
    use crate::secrets::SecretKind;

    fn at(s: &str) -> DateTime<Utc> {
        crate::types::iso8601::parse(s).unwrap()
    }

    fn entry(n: u128, kind: ClipKind, text: Option<&str>, copied: &str) -> ClipEntry {
        ClipEntry {
            id: Uuid::from_u128(n),
            kind,
            text: text.map(str::to_string),
            text_len: text.map_or(0, |t| t.chars().count()),
            line_count: 1,
            files: vec![],
            image: None,
            color: None,
            rich: false,
            types: vec![],
            size: 0,
            source: None,
            hash: format!("h{n}"),
            first_copied_at: at(copied),
            copied_at: at(copied),
            copy_count: 1,
            timings: vec![],
            last_used_at: None,
            use_count: 0,
            pinned: false,
            secret: None,
            secret_override: None,
        }
    }

    fn source(name: &str, bundle: &str) -> Option<ClipSource> {
        Some(ClipSource { bundle_id: bundle.into(), name: name.into(), path: None })
    }

    #[test]
    fn secrets() {
        let mut e = entry(1, ClipKind::Text, Some("x"), "2026-01-01T00:00:00Z");
        assert_eq!(secret_reason(&e), "Marked as secret");
        for (kind, reason) in [
            (SecretKind::PrivateKey, "Looks like a private key"),
            (SecretKind::ApiKey, "Looks like an API key"),
            (SecretKind::Token, "Looks like an access token"),
            (SecretKind::Password, "Looks like a password"),
            (SecretKind::Credential, "Looks like a secret"),
        ] {
            e.secret = Some(kind);
            assert_eq!(secret_reason(&e), reason);
        }
        assert!(is_secret(&e));
        e.secret_override = Some(false);
        assert!(!is_secret(&e));
    }

    #[test]
    fn filters() {
        let other = entry(1, ClipKind::Other, None, "2026-01-01T00:00:00Z");
        let code = entry(2, ClipKind::Code, Some("x"), "2026-01-01T00:00:00Z");
        assert!(in_filter(&other, ClipFilter::Kind(ClipKind::Text)));
        assert!(!in_filter(&code, ClipFilter::Kind(ClipKind::Text)));
        assert!(in_filter(&code, ClipFilter::Kind(ClipKind::Code)));
        assert!(!in_filter(&code, ClipFilter::Pinned));
        assert!(in_filter(&code, ClipFilter::All));
        assert_eq!(FILTERS.map(|f| f.label), ["All", "Pinned", "Text", "Images", "Links", "Files", "Code", "Colors"]);
    }

    #[test]
    fn parses_operators() {
        assert_eq!(parse_query("  Invoice APP:Mail is:LINKS is:bogus app: x"), ParsedQuery {
            terms: vec!["invoice".into(), "is:bogus".into(), "x".into()],
            app: Some(String::new()),
            kind: Some(ClipKind::Link),
        });
        assert_eq!(parse_query(""), ParsedQuery::default());
        assert_eq!(parse_query("is:img is:url").kind, Some(ClipKind::Link));
        assert_eq!(parse_query("is:colour").kind, Some(ClipKind::Color));
    }

    #[test]
    fn searches_with_operators() {
        let mut slack = entry(1, ClipKind::Text, Some("Quarterly Invoice"), "2026-01-03T00:00:00Z");
        slack.source = source("Slack", "com.tinyspeck.slackmacgap");
        let mut image = entry(2, ClipKind::Image, None, "2026-01-02T00:00:00Z");
        image.source = source("Preview", "com.apple.Preview");
        let mut file = entry(3, ClipKind::File, None, "2026-01-01T00:00:00Z");
        file.files = vec!["/Users/me/Docs/Report.PDF".into(), "/tmp/other.txt/".into()];
        let mut color = entry(4, ClipKind::Color, Some("#FF0000"), "2025-12-31T00:00:00Z");
        color.color = Some("#ff0000".into());
        color.pinned = true;
        let entries = vec![slack, image, file, color];
        let ids = |v: Vec<&ClipEntry>| v.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>();
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "")), [1, 2, 3, 4]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "app:slack")), [1]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "app:tinyspeck invoice")), [1]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "app:slack nope")), Vec::<u128>::new());
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "is:image")), [2]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "app: is:image")), [2]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "image")), [2]); // kind label
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "report.pdf other")), [3]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "docs")), Vec::<u128>::new()); // only basenames
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "#ff00 color")), [4]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::Pinned, "")), [4]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::Kind(ClipKind::Text), "invoice")), [1]);
        assert_eq!(ids(search_clips(&entries, ClipFilter::All, "app:preview")), [2]);
        let mut cache = HaystackCache::default();
        assert_eq!(ids(search_clips_cached(&entries, ClipFilter::All, "QUARTERLY", &mut cache)), [1]);
        assert_eq!(ids(search_clips_cached(&entries, ClipFilter::All, "is:file txt", &mut cache)), [3]);
    }

    #[test]
    fn highlights_matches() {
        assert_eq!(match_ranges("Hello hello", &["ell".into()]), [(1, 4), (7, 10)]);
        assert_eq!(match_ranges("abcdef", &["bc".into(), "cde".into(), "f".into()]), [(1, 6)]);
        assert_eq!(match_ranges("😀ab", &["ab".into()]), [(2, 4)]);
        assert!(match_ranges("", &["a".into()]).is_empty());
        assert!(match_ranges("a", &[]).is_empty());
        assert_eq!(match_ranges(&"a".repeat(100), &["a".into()]).len(), 1);
        assert_eq!(match_ranges(&"ab".repeat(100), &["a".into()]).len(), 64);
    }

    #[test]
    fn day_labels_and_rows() {
        let tz = FixedOffset::west_opt(5 * 3600).unwrap(); // 2026-11-08 09:00 local
        let now = at("2026-11-08T14:00:00Z").timestamp_millis();
        let label = |s: &str| day_label(at(s).timestamp_millis(), now, &tz);
        assert_eq!(label("2026-11-08T05:00:00Z"), "Today");
        assert_eq!(label("2026-11-08T04:59:59Z"), "Yesterday");
        assert_eq!(label("2026-11-07T05:00:00Z"), "Yesterday");
        assert_eq!(label("2026-11-03T12:00:00Z"), "Tuesday");
        assert_eq!(label("2026-11-02T12:00:00Z"), "Monday");
        assert_eq!(label("2026-11-01T12:00:00Z"), "Sun, Nov 1");
        assert_eq!(label("2025-12-31T12:00:00Z"), "Wed, Dec 31, 2025");
        assert_eq!(label("2026-11-10T12:00:00Z"), "Tuesday");

        let mut a = entry(1, ClipKind::Text, Some("a"), "2026-11-08T13:00:00Z");
        a.pinned = true;
        let b = entry(2, ClipKind::Text, Some("b"), "2026-11-08T12:00:00Z");
        let c = entry(3, ClipKind::Text, Some("c"), "2026-11-07T12:00:00Z");
        let d = entry(4, ClipKind::Text, Some("d"), "2026-11-07T11:00:00Z");
        let list = [&a, &b, &c, &d];
        let (rows, order) = build_rows(&list, RowOptions { pinned_first: true, sections: true }, now, &tz);
        let shape: Vec<String> = rows
            .iter()
            .map(|r| match r {
                ClipRow::Header { key, label, count } => format!("{key}|{label}|{count}"),
                ClipRow::Entry { entry, index, dated, .. } => format!("{}#{index}{}", entry.text.as_deref().unwrap(), if *dated { "d" } else { "" }),
            })
            .collect();
        assert_eq!(shape, ["h-pinned|Pinned|1", "a#0d", "h-2026-10-8|Today|1", "b#1", "h-2026-10-7|Yesterday|2", "c#2", "d#3"]);
        assert_eq!(order.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>(), [1, 2, 3, 4]);
        let (rows, _) = build_rows(&list, RowOptions { pinned_first: false, sections: false }, now, &tz);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| matches!(r, ClipRow::Entry { dated: true, .. })));
    }

    #[test]
    fn paths_and_sizes() {
        assert_eq!(basename("/a/b/"), "b");
        assert_eq!(basename("/"), "");
        assert_eq!(basename("file"), "file");
        assert_eq!(dirname("/a/b"), "/a");
        assert_eq!(dirname("/a"), "/");
        assert_eq!(dirname("a"), "/");
        assert_eq!(dirname(""), "/");
        assert_eq!(tildify("/Users/me/Documents"), "~/Documents");
        assert_eq!(tildify("/Users/me"), "~");
        assert_eq!(tildify("/Users/"), "/Users/");
        assert_eq!(tildify("/tmp/x"), "/tmp/x");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(10 * 1024 + 900), "11 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_bytes(3 * 1024 * 1024 * 1024 * 1024), "3072 GB");
    }

    #[test]
    fn times() {
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        let now = at("2026-11-08T19:30:00Z").timestamp_millis(); // 2:30 PM local
        let t = |s: &str, dated: bool| clip_time(&at(s), now, dated, &tz);
        assert_eq!(t("2026-11-08T19:29:20Z", false), "Just now");
        assert_eq!(t("2026-11-08T19:40:00Z", false), "Just now");
        assert_eq!(t("2026-11-08T19:27:00Z", false), "3m ago");
        assert_eq!(t("2026-11-08T19:04:00Z", true), "26m ago");
        assert_eq!(t("2026-11-08T18:04:00Z", true), "1:04 PM");
        assert_eq!(t("2026-11-08T05:00:00Z", true), "12:00 AM");
        assert_eq!(t("2026-11-07T12:00:00Z", true), "Yesterday");
        assert_eq!(t("2026-11-07T12:00:00Z", false), "7:00 AM");
        assert_eq!(t("2026-11-04T12:00:00Z", true), "Wed");
        assert_eq!(t("2026-10-28T12:00:00Z", true), "Oct 28");
        let x = |s: &str| exact_time(&at(s), now, &tz);
        assert_eq!(x("2026-11-08T19:04:11Z"), "Today at 2:04:11 PM");
        assert_eq!(x("2026-11-07T23:59:59Z"), "Yesterday at 6:59:59 PM");
        assert_eq!(x("2026-10-28T18:04:11Z"), "Oct 28, 2026, 1:04 PM");
    }

    #[test]
    fn expiry_and_retention() {
        let now = at("2026-11-08T12:00:00Z").timestamp_millis();
        let mut e = entry(1, ClipKind::Text, Some("x"), "2026-11-08T12:00:00Z");
        assert_eq!(expires_in(&e, 0, now), "When history is full");
        assert_eq!(expires_in(&e, 7, now), "In 7 days");
        assert_eq!(expires_in(&e, 1, now), "In 24 hours");
        e.copied_at = at("2026-11-01T12:30:00Z");
        assert_eq!(expires_in(&e, 7, now), "Within the hour");
        e.copied_at = at("2026-11-02T00:00:00Z");
        assert_eq!(expires_in(&e, 7, now), "In 12 hours");
        e.copied_at = at("2026-11-03T18:00:00Z");
        assert_eq!(expires_in(&e, 7, now), "In 2 days"); // 54 h → 2.25 days
        e.pinned = true;
        assert_eq!(expires_in(&e, 7, now), "Never (pinned)");
        let labels: Vec<_> = [0, 1, 2, 7, 14, 30, 60, 90, 365, 730, 10].into_iter().map(retention_label).collect();
        assert_eq!(labels, [
            "until you clear it",
            "for 1 day",
            "for 2 days",
            "for 1 week",
            "for 2 weeks",
            "for 1 month",
            "for 2 months",
            "for 3 months",
            "for 1 year",
            "for 2 years",
            "for 10 days"
        ]);
        assert_eq!(retention_label(210), "for 7 months"); // months before weeks
    }

    #[test]
    fn describes_entries() {
        let d = |e: &ClipEntry| {
            let d = describe(e);
            (d.title, d.subtitle)
        };
        let link = |s: &str| d(&entry(1, ClipKind::Link, Some(s), "2026-01-01T00:00:00Z"));
        assert_eq!(link(" https://www.github.com/kudzu/app/ "), ("github.com".into(), "/kudzu/app".into()));
        assert_eq!(link("https://github.com"), ("github.com".into(), "https://github.com".into()));
        assert_eq!(link("www.example.com/a%20b?q=%2F"), ("example.com".into(), "/a b?q=%2F".into()));
        assert_eq!(link("mailto:me@example.com"), ("me@example.com".into(), "Email address".into()));
        assert_eq!(link("https://x.com/%E0%A4%A"), ("https://x.com/%E0%A4%A".into(), "https://x.com/%E0%A4%A".into()));
        assert_eq!(link("http://host:8080/x"), ("host:8080".into(), "/x".into()));

        let mut color = entry(2, ClipKind::Color, Some("#ff0000"), "2026-01-01T00:00:00Z");
        assert_eq!(d(&color), ("#FF0000".into(), "Color".into()));
        color.color = Some("rgb(1 2 3)".into());
        assert_eq!(d(&color).0, "RGB(1 2 3)");

        let mut image = entry(3, ClipKind::Image, None, "2026-01-01T00:00:00Z");
        image.types = vec!["public.tiff".into(), "public.png".into()];
        image.size = 2048;
        assert_eq!(d(&image), ("Image".into(), "TIFF · 2.0 KB".into()));
        image.image = Some(ImageSize { width: 640, height: 480 });
        assert_eq!(d(&image).0, "Image  640 × 480");
        image.types = vec!["org.webmproject.webp".into()];
        assert_eq!(image_format(&image), "WebP");
        image.types = vec![];
        assert_eq!(image_format(&image), "Image");

        let mut file = entry(4, ClipKind::File, None, "2026-01-01T00:00:00Z");
        assert_eq!(d(&file), ("".into(), "/".into()));
        file.files = vec!["/Users/me/Docs/a.txt".into(), "/b".into(), "/c".into()];
        assert_eq!(d(&file), ("a.txt and 2 more".into(), "~/Docs".into()));

        let mut other = entry(5, ClipKind::Other, None, "2026-01-01T00:00:00Z");
        other.types = vec!["x".into()];
        other.size = 10;
        assert_eq!(d(&other), ("Data".into(), "1 format · 10 B".into()));
        other.source = source("Figma", "com.figma.Desktop");
        other.types.push("y".into());
        assert_eq!(d(&other), ("Content from Figma".into(), "2 formats · 10 B".into()));

        let mut text = entry(6, ClipKind::Text, Some("\n  First   line \n\n second\tline \nthird"), "2026-01-01T00:00:00Z");
        assert_eq!(d(&text), ("First line".into(), "second line".into()));
        text.text = Some("one".into());
        text.text_len = 2048;
        assert_eq!(d(&text), ("one".into(), "2,048 characters".into()));
        text.text_len = 1;
        assert_eq!(d(&text).1, "1 character");
        text.text = Some("  ".into());
        assert_eq!(d(&text).0, "");
    }

    #[test]
    fn spoken_labels() {
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        let now = at("2026-11-08T19:30:00Z").timestamp_millis();
        let mut e = entry(1, ClipKind::Link, Some("https://github.com/x"), "2026-11-08T19:04:11Z");
        e.source = source("Safari", "com.apple.Safari");
        e.pinned = true;
        e.copy_count = 3;
        assert_eq!(spoken_label(&e, false, now, &tz), "Link, github.com, copied Today at 2:04:11 PM, from Safari, 3 times, pinned");
        e.secret = Some(SecretKind::ApiKey);
        assert_eq!(spoken_label(&e, true, now, &tz), "Hidden secret, looks like an api key, copied Today at 2:04:11 PM, from Safari, 3 times, pinned");
    }

    #[test]
    fn small_helpers() {
        assert_eq!(word_count("  one two\n three\u{A0}four "), 4);
        assert_eq!(word_count(""), 0);
        let mut e = entry(1, ClipKind::Text, Some("😀"), "2026-01-01T00:00:00Z");
        assert!(has_text(&e));
        assert_eq!(inline_full_text(&e), Some("😀"));
        e.text_len = 3; // longer than the excerpt's 2 UTF-16 units
        assert_eq!(inline_full_text(&e), None);
        e.text = None;
        assert_eq!(inline_full_text(&e), None);
        assert!(!has_text(&entry(2, ClipKind::Image, None, "2026-01-01T00:00:00Z")));
    }

    #[test]
    fn upserts_newest_first() {
        let e = |n: u128, s: &str| entry(n, ClipKind::Text, Some("x"), s);
        let mut list = vec![e(1, "2026-01-03T00:00:00Z"), e(2, "2026-01-02T00:00:00Z"), e(3, "2026-01-01T00:00:00Z")];
        upsert(&mut list, e(4, "2026-01-02T00:00:00Z"));
        assert_eq!(list.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>(), [1, 4, 2, 3]);
        upsert(&mut list, e(3, "2026-01-04T00:00:00Z"));
        assert_eq!(list.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>(), [3, 1, 4, 2]);
        let mut shuffled = vec![e(1, "2026-01-01T00:00:00Z"), e(2, "2026-01-03T00:00:00Z"), e(3, "2026-01-02T00:00:00Z")];
        sort_newest(&mut shuffled);
        assert_eq!(shuffled.iter().map(|e| e.id.as_u128()).collect::<Vec<_>>(), [2, 3, 1]);
    }
}
