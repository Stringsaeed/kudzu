//! Pure clipboard labels. Timezone and clock are explicit; English matches the en-US reference.
use super::{query::is_js_whitespace, sections::day_label, ClipEntry, ClipKind};
use crate::secrets::SecretKind;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use serde::Serialize;

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
pub fn secret_reason(e: &ClipEntry) -> &'static str {
    match e.secret {
        Some(SecretKind::PrivateKey) => "Looks like a private key",
        Some(SecretKind::ApiKey) => "Looks like an API key",
        Some(SecretKind::Token) => "Looks like an access token",
        Some(SecretKind::Password) => "Looks like a password",
        Some(SecretKind::Credential) => "Looks like a secret",
        None => "Marked as secret",
    }
}
pub fn basename(path: &str) -> &str {
    let p = path.trim_end_matches('/');
    p.rsplit('/').next().unwrap_or(p)
}
pub fn dirname(path: &str) -> &str {
    let p = path.trim_end_matches('/');
    match p.rfind('/') {
        Some(i) if i > 0 => &p[..i],
        _ => "/",
    }
}
pub fn tildify(path: &str) -> String {
    if let Some(rest) = path
        .strip_prefix("/Users/")
        .filter(|s| !s.starts_with('/') && !s.is_empty())
    {
        let end = rest.find('/').unwrap_or(rest.len());
        format!("~{}", &rest[end..])
    } else {
        path.into()
    }
}
pub fn format_bytes(n: u64) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64 / 1024.0;
    let units = ["KB", "MB", "GB"];
    let mut u = 0;
    while v >= 1024.0 && u < 2 {
        v /= 1024.0;
        u += 1;
    }
    if v < 10.0 {
        format!("{v:.1} {}", units[u])
    } else {
        format!("{} {}", v.round(), units[u])
    }
}
pub fn word_count(text: &str) -> usize {
    text.split(is_js_whitespace)
        .filter(|s| !s.is_empty())
        .count()
}
pub fn has_text(e: &ClipEntry) -> bool {
    matches!(
        e.kind,
        ClipKind::Text | ClipKind::Code | ClipKind::Link | ClipKind::Color
    )
}
fn count(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
pub fn text_facts(e: &ClipEntry, full: Option<&str>) -> String {
    let mut s = format!(
        "{} character{}",
        count(e.text_len),
        if e.text_len == 1 { "" } else { "s" }
    );
    if let Some(full) = full {
        let words = word_count(full);
        s.push_str(&format!(
            " · {} word{}",
            count(words),
            if words == 1 { "" } else { "s" }
        ));
    }
    if e.line_count > 1 {
        s.push_str(&format!(" · {} lines", count(e.line_count)));
    }
    s
}
pub fn image_format(e: &ClipEntry) -> &'static str {
    let kinds = [
        ("png", "PNG"),
        ("jpeg", "JPEG"),
        ("tiff", "TIFF"),
        ("gif", "GIF"),
        ("heic", "HEIC"),
        ("webp", "WebP"),
    ];
    for t in &e.types {
        if let Some((_, label)) = kinds.iter().find(|(key, _)| t.contains(key)) {
            return label;
        }
    }
    "Image"
}
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Description {
    pub title: String,
    pub subtitle: String,
}
fn collapse(s: &str) -> String {
    s.split(is_js_whitespace)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn describe(e: &ClipEntry) -> Description {
    let text = e.text.as_deref().unwrap_or("");
    let (title, subtitle) = match e.kind {
        ClipKind::Link => link_parts(text.trim_matches(is_js_whitespace)),
        ClipKind::Color => (
            e.color.as_deref().unwrap_or(text).to_uppercase(),
            "Color".into(),
        ),
        ClipKind::Image => (
            e.image
                .map(|i| format!("Image  {} × {}", i.width, i.height))
                .unwrap_or("Image".into()),
            format!("{} · {}", image_format(e), format_bytes(e.size)),
        ),
        ClipKind::File => {
            let first = e.files.first().map(String::as_str).unwrap_or("");
            (
                if e.files.len() > 1 {
                    format!("{} and {} more", basename(first), e.files.len() - 1)
                } else {
                    basename(first).into()
                },
                tildify(dirname(first)),
            )
        }
        ClipKind::Other => (
            e.source
                .as_ref()
                .map(|s| format!("Content from {}", s.name))
                .unwrap_or("Data".into()),
            format!(
                "{} format{} · {}",
                e.types.len(),
                if e.types.len() == 1 { "" } else { "s" },
                format_bytes(e.size)
            ),
        ),
        _ => {
            let mut lines = text
                .split('\n')
                .map(|l| l.trim_matches(is_js_whitespace))
                .filter(|l| !l.is_empty());
            (
                collapse(lines.next().unwrap_or("")),
                lines.next().map(collapse).unwrap_or_else(|| {
                    format!(
                        "{} character{}",
                        count(e.text_len),
                        if e.text_len == 1 { "" } else { "s" }
                    )
                }),
            )
        }
    };
    Description { title, subtitle }
}
// URL display only, never used for opening/validation. Preserve decodeURI's reserved escapes.
fn decode_uri(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let pair = s.get(i + 1..i + 3)?;
            let n = u8::from_str_radix(pair, 16).ok()?;
            if b";/?:@&=+$,#".contains(&n) {
                out.extend_from_slice(&b[i..i + 3]);
            } else {
                out.push(n);
            }
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
fn link_parts(text: &str) -> (String, String) {
    let url = if text
        .get(..4)
        .is_some_and(|s| s.eq_ignore_ascii_case("www."))
    {
        format!("https://{text}")
    } else {
        text.into()
    };
    let fallback = || (text.into(), String::new());
    let Some((scheme, body)) = url.split_once(':') else {
        return fallback();
    };
    if scheme.eq_ignore_ascii_case("mailto") {
        return (
            body.split(['?', '#']).next().unwrap_or("").into(),
            "Email address".into(),
        );
    }
    let Some(body) = body.strip_prefix("//") else {
        return fallback();
    };
    let end = body.find(['/', '?', '#']).unwrap_or(body.len());
    let host = body[..end].rsplit('@').next().unwrap_or("").to_lowercase();
    if host.is_empty() || host.contains(char::is_whitespace) {
        return fallback();
    }
    let mut host = host.strip_prefix("www.").unwrap_or(&host).to_string();
    let port = match scheme.to_ascii_lowercase().as_str() {
        "https" => ":443",
        "http" => ":80",
        "ftp" => ":21",
        _ => "",
    };
    if !port.is_empty() && host.ends_with(port) {
        host.truncate(host.len() - port.len());
    }
    let rest = body[end..].split('#').next().unwrap_or("");
    let rest = if rest.starts_with('?') {
        format!("/{rest}")
    } else {
        rest.into()
    };
    let Some(mut rest) = decode_uri(&rest) else {
        return fallback();
    };
    let (path, query) = rest.split_once('?').unwrap_or((&rest, ""));
    if let Some(path) = path.strip_prefix('/') {
        let mut segments = Vec::new();
        let mut parts = path.split('/').peekable();
        while let Some(part) = parts.next() {
            match part {
                "." => {}
                ".." => {
                    segments.pop();
                }
                _ => {
                    segments.push(part);
                    continue;
                }
            }
            if parts.peek().is_none() {
                segments.push("");
            }
        }
        let normalized = format!("/{}", segments.join("/"));
        rest = if rest.contains('?') {
            format!("{normalized}?{query}")
        } else {
            normalized
        };
    }
    if rest.ends_with('/') {
        rest.pop();
    }
    (host, if rest.is_empty() { text.into() } else { rest })
}
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
pub fn retention_label(days: u32) -> String {
    if days == 0 {
        return "until you clear it".into();
    }
    for (divisor, unit) in [(365, "year"), (30, "month"), (7, "week"), (1, "day")] {
        if days.is_multiple_of(divisor) {
            let n = days / divisor;
            return format!("for {n} {unit}{}", if n == 1 { "" } else { "s" });
        }
    }
    unreachable!()
}
pub fn spoken_label<T: TimeZone>(e: &ClipEntry, concealed: bool, now: DateTime<T>) -> String {
    let mut parts = if concealed {
        vec!["Hidden secret".into(), secret_reason(e).to_lowercase()]
    } else {
        vec![
            kind_label(e.kind).into(),
            describe(e)
                .title
                .chars()
                .scan(0, |n, c| {
                    *n += c.len_utf16();
                    (*n <= 120).then_some(c)
                })
                .collect(),
        ]
    };
    parts.push(format!("copied {}", exact_time(e.copied_at, now)));
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
