//! Contiguous, case-insensitive clipboard search in history order.
//! The index borrows an immutable snapshot: replacing it also invalidates its cached text.
use super::{
    facts::{basename, kind_label},
    ClipEntry, ClipKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClipFilter {
    All,
    Pinned,
    Text,
    Code,
    Link,
    Color,
    Image,
    File,
    Other,
}
impl ClipFilter {
    pub fn contains(self, e: &ClipEntry) -> bool {
        match self {
            Self::All => true,
            Self::Pinned => e.pinned,
            Self::Text => matches!(e.kind, ClipKind::Text | ClipKind::Other),
            Self::Code => e.kind == ClipKind::Code,
            Self::Link => e.kind == ClipKind::Link,
            Self::Color => e.kind == ClipKind::Color,
            Self::Image => e.kind == ClipKind::Image,
            Self::File => e.kind == ClipKind::File,
            Self::Other => e.kind == ClipKind::Other,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClipQuery {
    pub terms: Vec<String>,
    pub app: Option<String>,
    pub kind: Option<ClipKind>,
}
impl ClipQuery {
    pub fn parse(query: &str) -> Self {
        let mut result = Self {
            terms: Vec::new(),
            app: None,
            kind: None,
        };
        for raw in query
            .to_lowercase()
            .split(is_js_whitespace)
            .filter(|s| !s.is_empty())
        {
            if let Some(app) = raw.strip_prefix("app:") {
                result.app = Some(app.into());
            } else if let Some(kind) = raw.strip_prefix("is:").and_then(kind_alias) {
                result.kind = Some(kind);
            } else {
                result.terms.push(raw.into());
            }
        }
        result
    }
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
// ECMAScript's whitespace differs from Rust's (BOM is whitespace, NEL is not).
pub(super) fn is_js_whitespace(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

pub struct ClipSearch<'a> {
    entries: &'a [ClipEntry],
    haystacks: Vec<String>,
    apps: Vec<String>,
}
impl<'a> ClipSearch<'a> {
    pub fn new(entries: &'a [ClipEntry]) -> Self {
        let haystacks = entries
            .iter()
            .map(|e| {
                format!(
                    "{}\n{}\n{}\n{}",
                    e.text.as_deref().unwrap_or(""),
                    e.files
                        .iter()
                        .map(|s| basename(s))
                        .collect::<Vec<_>>()
                        .join(" "),
                    e.color.as_deref().unwrap_or(""),
                    kind_label(e.kind)
                )
                .to_lowercase()
            })
            .collect();
        let apps = entries
            .iter()
            .map(|e| {
                e.source
                    .as_ref()
                    .map(|s| format!("{}\n{}", s.name, s.bundle_id).to_lowercase())
                    .unwrap_or_default()
            })
            .collect();
        Self {
            entries,
            haystacks,
            apps,
        }
    }
    /// For local UI only: secret entries remain searchable, as in the reference.
    /// Agent callers must apply their own redaction policy before returning results.
    pub fn search(&self, filter: ClipFilter, query: &ClipQuery) -> Vec<&'a ClipEntry> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                filter.contains(e)
                    && query.kind.is_none_or(|k| e.kind == k)
                    && query.app.as_ref().is_none_or(|a| {
                        a.is_empty() || self.apps[*i].split('\n').any(|s| s.contains(a))
                    })
                    && query.terms.iter().all(|t| self.haystacks[*i].contains(t))
            })
            .map(|(_, e)| e)
            .collect()
    }
}
/// UTF-16 [start,end) offsets, like JavaScript string indices. Terms come from `ClipQuery`.
/// Collect at most 64 occurrences before sorting and merging adjacent/overlapping ranges.
pub fn match_ranges(text: &str, terms: &[String]) -> Vec<(usize, usize)> {
    let lower: Vec<u16> = text.to_lowercase().encode_utf16().collect();
    let mut ranges = Vec::new();
    for term in terms {
        let t: Vec<u16> = term.encode_utf16().collect();
        if t.is_empty() {
            continue;
        }
        let mut start = 0;
        while ranges.len() < 64 && start + t.len() <= lower.len() {
            let Some(i) = lower[start..].windows(t.len()).position(|w| w == t) else {
                break;
            };
            start += i;
            ranges.push((start, start + t.len()));
            start += t.len();
        }
    }
    ranges.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in ranges {
        if let Some(last) = merged.last_mut().filter(|last| r.0 <= last.1) {
            last.1 = last.1.max(r.1);
        } else {
            merged.push(r);
        }
    }
    merged
}
