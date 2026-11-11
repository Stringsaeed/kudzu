//! Formatting annotates the unchanged plain-text template using UTF-16 ranges.
//!
//! [`Formatting`] is stored on a snippet, so its JSON shape is part of the library format.
//! `html` renders spans for the rich pasteboard flavour.

mod html;

use serde::{Deserialize, Serialize};

pub use html::{escape_html, html};

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
        .or_else(|| {
            url.strip_prefix("data:image/jpeg;base64,")
                .filter(|d| d.starts_with("/9j/"))
        });
    url.len() <= 196_608
        && data.is_some_and(|d| {
            !d.is_empty()
                && d.len().is_multiple_of(4)
                && d.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
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
