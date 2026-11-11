//! Pure clipboard labels. Timezone and clock are explicit; English matches the en-US reference.
pub use super::time::{clip_time, exact_time, expires_in};
use super::{link::link_parts, whitespace::is_js_whitespace, ClipEntry, ClipKind};
use crate::secrets::SecretKind;
use chrono::{DateTime, TimeZone};
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
        // JS toFixed rounds positive ties up; Rust's precision formatter uses ties-to-even.
        let tenths = (v * 10.0).round() as u64;
        format!("{}.{:01} {}", tenths / 10, tenths % 10, units[u])
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
/// The shared visibility rule for history, previews and the picker.
pub fn concealed(entry: &ClipEntry, show_secrets: bool) -> bool {
    entry.is_secret() && !show_secrets
}
pub fn spoken_label<T: TimeZone>(e: &ClipEntry, show_secrets: bool, now: DateTime<T>) -> String {
    let mut parts = if concealed(e, show_secrets) {
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
