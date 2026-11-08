//! Rich-text spans as the editor and previews handled them (`lib/formatting.ts`). Offsets are
//! UTF-16 code units, as in [`crate::formatting`]. The checks here are the UI's own, which
//! differ from the paste path's in a few characters (see [`safe_link`]).

use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;

use crate::formatting::{FormatSpan, FormatStyle, Formatting};

use super::js;

/// `/^(https?:\/\/|mailto:)/i` with none of whitespace (JS `\s`), C0 controls, DEL, quotes or
/// angle brackets. Unlike [`crate::formatting::safe_link`], C1 controls pass and U+FEFF fails.
pub fn safe_link(url: &str) -> bool {
    let head: String = url.chars().take(8).collect::<String>().to_ascii_lowercase();
    (head.starts_with("http://") || head.starts_with("https://") || head.starts_with("mailto:"))
        && !url.chars().any(|c| js::is_space(c) || c <= '\u{1f}' || c == '\u{7f}' || matches!(c, '"' | '\'' | '<' | '>'))
}

/// An inline PNG or JPEG data URL of at most 192 KiB with well-formed base64 length.
pub fn safe_image(data_url: &str) -> bool {
    let data = data_url
        .strip_prefix("data:image/png;base64,")
        .filter(|d| d.starts_with("iVBORw0KGgo"))
        .or_else(|| data_url.strip_prefix("data:image/jpeg;base64,").filter(|d| d.starts_with("/9j/")));
    js::len16(data_url) <= 196_608
        && data.is_some_and(|d| d.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=')) && d.len().is_multiple_of(4))
}

fn is_high(u: u16) -> bool {
    (0xD800..=0xDBFF).contains(&u)
}

fn is_low(u: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&u)
}

/// Spans that are non-empty, inside the text, don't split a surrogate pair, and carry a safe
/// link or image (1–512 px wide).
pub fn valid_spans(text: &str, spans: &[FormatSpan]) -> Vec<FormatSpan> {
    let units = js::utf16(text);
    let boundary = |pos: usize| pos <= units.len() && !(pos > 0 && is_high(units[pos - 1]) && units.get(pos).copied().is_some_and(is_low));
    spans
        .iter()
        .filter(|s| {
            s.start < s.end
                && boundary(s.start)
                && boundary(s.end)
                && match &s.style {
                    FormatStyle::Link { url } => safe_link(url),
                    FormatStyle::Image { data_url, width } => safe_image(data_url) && (1..=512).contains(width),
                    _ => true,
                }
        })
        .cloned()
        .collect()
}

/// The valid spans when `formatting` belongs to `content`, else `None` (stale or absent).
pub fn source_spans(content: &str, formatting: Option<&Formatting>) -> Option<Vec<FormatSpan>> {
    formatting.filter(|f| f.source == content).map(|f| valid_spans(content, &f.spans))
}

/// Maps literal source offsets to rendered offsets for `raw` (template text starting at
/// `start` in the source and at `output` in the rendered text), skipping escape backslashes.
/// Steps by grapheme, recording each boundary in `positions`.
pub fn literal_positions(raw: &str, start: usize, output: usize, positions: &mut HashMap<usize, usize>) {
    let graphemes: Vec<&str> = raw.graphemes(true).collect();
    let mut input = start;
    let mut rendered = output;
    positions.insert(input, rendered);
    let mut i = 0;
    while i < graphemes.len() {
        let next = graphemes.get(i + 1).copied();
        if graphemes[i] == "\\" && (next == Some("\\") || (next == Some("{") && graphemes.get(i + 2).copied() == Some("{"))) {
            input += 1;
            i += 1;
        }
        let len = js::len16(graphemes[i]);
        input += len;
        rendered += len;
        positions.insert(input, rendered);
        i += 1;
    }
}

/// Same kind, and for links and images the same target.
pub fn same_style(a: &FormatStyle, b: &FormatStyle) -> bool {
    a == b
}

/// One independently styled piece of text.
#[derive(Debug, Clone, PartialEq)]
pub struct FormattedPart {
    /// UTF-16 offset of the piece.
    pub start: usize,
    pub text: String,
    /// The image span covering the piece's start, if any.
    pub image: Option<FormatSpan>,
    /// Styles of every span covering the whole piece.
    pub styles: Vec<FormatStyle>,
}

/// Splits overlapping spans into independently styled pieces, without interpreting HTML. A
/// `cursor` inside the text adds a boundary (a cursor inside a surrogate pair leaves U+FFFD halves).
pub fn formatted_parts(text: &str, spans: &[FormatSpan], cursor: Option<usize>) -> Vec<FormattedPart> {
    let valid = valid_spans(text, spans);
    let units = js::utf16(text);
    let mut boundaries = vec![0, units.len()];
    for s in &valid {
        boundaries.extend([s.start, s.end]);
    }
    if let Some(c) = cursor.filter(|&c| c <= units.len()) {
        boundaries.push(c);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
        .windows(2)
        .map(|w| {
            let (start, end) = (w[0], w[1]);
            FormattedPart {
                start,
                text: js::from16(&units[start..end]),
                image: valid.iter().find(|s| matches!(s.style, FormatStyle::Image { .. }) && s.start <= start && s.end > start).cloned(),
                styles: valid.iter().filter(|s| s.start <= start && s.end >= end).map(|s| s.style.clone()).collect(),
            }
        })
        .collect()
}

pub use crate::formatting::escape_html;

/// The clipboard HTML flavour for formatted text.
pub fn formatted_html(text: &str, spans: &[FormatSpan]) -> String {
    let mut body = String::new();
    for part in formatted_parts(text, spans, None) {
        if let Some(image) = &part.image {
            if let FormatStyle::Image { data_url, width } = &image.style {
                if image.start == part.start {
                    body.push_str(&format!("<img src=\"{data_url}\" width=\"{width}\" alt=\"{}\">", escape_html(&js::slice16(text, image.start, image.end))));
                }
                continue;
            }
        }
        let mut html = escape_html(&part.text).replace('\n', "<br>");
        let has = |kind: &FormatStyle| part.styles.iter().any(|s| s == kind);
        if has(&FormatStyle::Underline) {
            html = format!("<u>{html}</u>");
        }
        if has(&FormatStyle::Italic) {
            html = format!("<em>{html}</em>");
        }
        if has(&FormatStyle::Bold) {
            html = format!("<strong>{html}</strong>");
        }
        if let Some(url) = part.styles.iter().find_map(|s| match s {
            FormatStyle::Link { url } => Some(url),
            _ => None,
        }) {
            html = format!("<a href=\"{}\">{html}</a>", escape_html(url));
        }
        body.push_str(&html);
    }
    format!("<html><head><meta charset=\"utf-8\"></head><body><span style=\"white-space: pre-wrap; font-family: -apple-system, sans-serif;\">{body}</span></body></html>")
}

/// Whether two span lists (or their absence) are the same.
pub fn same_spans(a: Option<&[FormatSpan]>, b: Option<&[FormatSpan]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.start == y.start && x.end == y.end && same_style(&x.style, &y.style)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg==";

    fn span(start: usize, end: usize, style: FormatStyle) -> FormatSpan {
        FormatSpan { start, end, style }
    }

    fn image() -> FormatStyle {
        FormatStyle::Image { data_url: PNG.into(), width: 120 }
    }

    #[test]
    fn only_inert_links_and_raster_images() {
        assert!(!safe_link("javascript:alert(1)"));
        assert!(safe_link("HTTPS://example.com"));
        assert!(safe_link("mailto:a@b.c"));
        assert!(!safe_link("https://a b"));
        assert!(!safe_link("https://a\u{FEFF}b"));
        assert!(safe_link("https://a\u{85}b"));
        assert!(!safe_link("https://a\"b"));
        assert!(!safe_image("data:image/svg+xml;base64,PHN2Zz4="));
        assert!(!safe_image("https://example.com/a.png"));
        assert!(!safe_image("data:image/png;base64,iVBORw0KGgo"));
        assert!(safe_image(PNG));
        assert!(formatted_html("[Logo]", &[span(0, 6, image())]).contains("width=\"120\" alt=\"[Logo]\""));
    }

    #[test]
    fn validates_spans() {
        let text = "a😀b";
        let spans = [
            span(0, 1, FormatStyle::Bold),
            span(0, 2, FormatStyle::Bold), // splits the pair
            span(1, 3, FormatStyle::Italic),
            span(2, 2, FormatStyle::Bold),
            span(3, 5, FormatStyle::Bold),
            span(0, 1, FormatStyle::Link { url: "ftp://x".into() }),
            span(0, 1, FormatStyle::Image { data_url: PNG.into(), width: 0 }),
            span(0, 4, FormatStyle::Image { data_url: PNG.into(), width: 512 }),
        ];
        let valid = valid_spans(text, &spans);
        assert_eq!(valid, [spans[0].clone(), spans[2].clone(), spans[7].clone()]);
        let f = Formatting { source: text.into(), spans: spans.to_vec() };
        assert_eq!(source_spans(text, Some(&f)).map(|v| v.len()), Some(3));
        assert_eq!(source_spans("other", Some(&f)), None);
        assert_eq!(source_spans(text, None), None);
    }

    #[test]
    fn splits_overlapping_spans() {
        let parts = formatted_parts("Hello world", &[span(0, 5, FormatStyle::Bold), span(3, 8, FormatStyle::Italic)], Some(10));
        let summary: Vec<_> = parts.iter().map(|p| (p.start, p.text.as_str(), p.styles.len())).collect();
        assert_eq!(summary, [(0, "Hel", 1), (3, "lo", 2), (5, " wo", 1), (8, "rl", 0), (10, "d", 0)]);
        assert!(formatted_parts("", &[], None).is_empty());
        assert_eq!(formatted_parts("ab", &[], Some(9)).len(), 1);
    }

    #[test]
    fn html_nests_styles_and_escapes() {
        let html = formatted_html("<a&b>\nc", &[span(0, 7, FormatStyle::Bold), span(0, 3, FormatStyle::Link { url: "https://x.y/?a=1&b".into() }), span(0, 2, FormatStyle::Underline)]);
        assert!(html.contains("<a href=\"https://x.y/?a=1&amp;b\"><strong><u>&lt;a</u></strong></a><a href=\"https://x.y/?a=1&amp;b\"><strong>&amp;</strong></a><strong>b&gt;<br>c</strong>"), "{html}");
        let html = formatted_html("Hello\n[Logo] world", &[span(0, 5, FormatStyle::Bold), span(6, 12, image())]);
        assert!(html.contains("<strong>Hello</strong><br><img src=\"data:image/png;base64,"));
        assert!(html.contains("alt=\"[Logo]\"> world</span>"));
    }

    #[test]
    fn maps_literal_positions() {
        let mut positions = HashMap::new();
        literal_positions(r"😀 \{{x", 3, 10, &mut positions);
        let mut v: Vec<_> = positions.into_iter().collect();
        v.sort();
        assert_eq!(v, [(3, 10), (5, 12), (6, 13), (8, 14), (9, 15), (10, 16)]);
    }

    #[test]
    fn compares_spans() {
        let a = [span(0, 1, FormatStyle::Link { url: "https://a".into() })];
        let b = [span(0, 1, FormatStyle::Link { url: "https://b".into() })];
        assert!(same_spans(None, None));
        assert!(same_spans(Some(&a), Some(&a.clone())));
        assert!(!same_spans(Some(&a), Some(&b)));
        assert!(!same_spans(Some(&a), None));
        assert!(!same_style(&FormatStyle::Bold, &FormatStyle::Italic));
    }
}
