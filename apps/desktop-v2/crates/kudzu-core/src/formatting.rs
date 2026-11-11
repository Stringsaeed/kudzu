//! Formatting annotates the unchanged plain-text template using UTF-16 ranges.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FormatStyle {
    Bold,
    Italic,
    Underline,
    Link {
        url: String,
    },
    Image {
        #[serde(rename = "dataUrl")]
        data_url: String,
        width: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FormatSpan {
    pub start: usize,
    pub end: usize,
    pub style: FormatStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Formatting {
    /// Detects stale ranges when an older client edits only `content`.
    pub source: String,
    pub spans: Vec<FormatSpan>,
}

pub fn safe_link(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:"))
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>'))
}

/// Raster data only: no remote fetches or active SVG content in imported snippets.
pub fn safe_image(url: &str) -> bool {
    let data = url
        .strip_prefix("data:image/png;base64,")
        .filter(|d| d.starts_with("iVBORw0KGgo"))
        .or_else(|| url.strip_prefix("data:image/jpeg;base64,").filter(|d| d.starts_with("/9j/")));
    url.len() <= 196_608
        && data.is_some_and(|d| {
            !d.is_empty() && d.len().is_multiple_of(4) && d.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
        })
}

/// Text occupied by the rich paste: each image is a single attachment character.
/// This is used for caret movement and undo, while `text` remains the fallback clipboard flavor.
pub fn insertion_text(text: &str, spans: &[FormatSpan]) -> String {
    let mut images: Vec<_> = spans
        .iter()
        .filter(|s| matches!(&s.style, FormatStyle::Image { data_url, .. } if safe_image(data_url)))
        .collect();
    images.sort_by_key(|s| s.start);
    let utf16: Vec<_> = text.encode_utf16().collect();
    let mut output = String::new();
    let mut at = 0;
    for image in images {
        if image.start < at || image.start >= image.end || image.end > utf16.len() {
            continue;
        }
        output.push_str(&String::from_utf16_lossy(&utf16[at..image.start]));
        output.push('\u{fffc}');
        at = image.end;
    }
    output.push_str(&String::from_utf16_lossy(&utf16[at..]));
    output
}

pub fn insertion_offset(offset: usize, spans: &[FormatSpan]) -> usize {
    let mut images: Vec<_> = spans
        .iter()
        .filter(|s| matches!(&s.style, FormatStyle::Image { data_url, .. } if safe_image(data_url)))
        .collect();
    images.sort_by_key(|s| s.start);
    let (mut removed, mut end) = (0, 0);
    for image in images {
        if image.start < end || image.start >= image.end || image.start >= offset {
            continue;
        }
        removed += offset.min(image.end) - image.start - 1;
        end = image.end;
    }
    offset.saturating_sub(removed)
}

pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// HTML preserves the text, replacing image alternatives with their embedded attachments.
/// Only our own tags are emitted; neither template values nor imported URLs can add markup.
pub fn html(text: &str, spans: &[FormatSpan]) -> String {
    let mut offsets = vec![(0, 0)];
    let mut utf16 = 0;
    for (byte, c) in text.char_indices() {
        utf16 += c.len_utf16();
        offsets.push((utf16, byte + c.len_utf8()));
    }
    let byte_at = |offset| offsets.binary_search_by_key(&offset, |(u, _)| *u).ok().map(|i| offsets[i].1);
    let valid: Vec<_> = spans
        .iter()
        .filter(|s| {
            s.start < s.end
                && byte_at(s.start).is_some()
                && byte_at(s.end).is_some()
                && match &s.style {
                    FormatStyle::Link { url } => safe_link(url),
                    FormatStyle::Image { data_url, width } => safe_image(data_url) && (1..=512).contains(width),
                    _ => true,
                }
        })
        .collect();
    let mut boundaries = vec![0, utf16];
    for s in &valid {
        boundaries.extend([s.start, s.end]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut out = String::from(
        "<html><head><meta charset=\"utf-8\"></head><body><span style=\"white-space: pre-wrap; font-family: -apple-system, sans-serif;\">",
    );
    for pair in boundaries.windows(2) {
        let active: Vec<_> = valid.iter().filter(|s| s.start <= pair[0] && s.end >= pair[1]).collect();
        if let Some(image) = active.iter().find(|s| matches!(s.style, FormatStyle::Image { .. })) {
            if image.start == pair[0] {
                if let FormatStyle::Image { data_url, width } = &image.style {
                    let alt = &text[byte_at(image.start).unwrap()..byte_at(image.end).unwrap()];
                    out.push_str(&format!(
                        "<img src=\"{}\" width=\"{width}\" alt=\"{}\">",
                        data_url,
                        escape_html(alt)
                    ));
                }
            }
            continue;
        }
        let bold = active.iter().any(|s| s.style == FormatStyle::Bold);
        let italic = active.iter().any(|s| s.style == FormatStyle::Italic);
        let underline = active.iter().any(|s| s.style == FormatStyle::Underline);
        let link = active.iter().find_map(|s| match &s.style {
            FormatStyle::Link { url } => Some(url),
            _ => None,
        });
        if let Some(url) = link {
            out.push_str(&format!("<a href=\"{}\">", escape_html(url)));
        }
        if bold {
            out.push_str("<strong>");
        }
        if italic {
            out.push_str("<em>");
        }
        if underline {
            out.push_str("<u>");
        }
        if let (Some(start), Some(end)) = (byte_at(pair[0]), byte_at(pair[1])) {
            out.push_str(&escape_html(&text[start..end]).replace('\n', "<br>"));
        }
        if underline {
            out.push_str("</u>");
        }
        if italic {
            out.push_str("</em>");
        }
        if bold {
            out.push_str("</strong>");
        }
        if link.is_some() {
            out.push_str("</a>");
        }
    }
    out.push_str("</span></body></html>");
    out
}
