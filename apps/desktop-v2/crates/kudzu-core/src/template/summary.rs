//! Plain-text summaries of a template.
//!
//! List rows use 140 UTF-16 units. Search uses 2,000, the same call as
//! `plainSummary(content, undefined, 2000)` in the Tauri app. Tags are shown as their chip
//! labels inside `‹›`. Whitespace collapses the way JavaScript's `/\s+/` does, then the string
//! is trimmed and cut. A cut that would split a surrogate pair stops before the pair.

use super::chips::chip_label;
use super::{parse, Node};

/// Units in a library-list row.
pub const LIST_UNITS: usize = 140;
/// Units indexed for palette and library search.
pub const SEARCH_UNITS: usize = 2_000;

/// Nested-snippet lookup. Search passes `None`.
pub use super::chips::SnippetNames;

/// `plainSummary`. `snippet_name` is the nested-snippet lookup; search passes `None`.
pub fn plain_summary(source: &str, snippet_name: Option<SnippetNames<'_>>, max: usize) -> String {
    match snippet_name {
        Some(names) => summarise(source, names, max),
        None => summarise(source, &|_| None, max),
    }
}

fn summarise(source: &str, names: SnippetNames<'_>, max: usize) -> String {
    let mut out = String::new();
    for node in &parse(source).nodes {
        match node {
            Node::Text { text, .. } => out.push_str(text),
            Node::Tag(tag) => {
                let slice = super::doc::slice_utf16(source, tag.range.clone());
                out.push('\u{2039}');
                out.push_str(&chip_label(tag, &slice, names));
                out.push('\u{203a}');
            }
        }
        if utf16_len(&out) > max {
            break;
        }
    }
    let collapsed = collapse_js_space(&out);
    let trimmed = trim_js(&collapsed);
    utf16_prefix(trimmed, max)
}

fn collapse_js_space(text: &str) -> String {
    let mut out = String::new();
    let mut in_space = false;
    for c in text.chars() {
        if is_js_space(c) {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            in_space = false;
            out.push(c);
        }
    }
    out
}

fn trim_js(text: &str) -> &str {
    let mut start = 0;
    let mut end = text.len();
    for (i, c) in text.char_indices() {
        if is_js_space(c) {
            start = i + c.len_utf8();
        } else {
            break;
        }
    }
    for (i, c) in text.char_indices().rev() {
        if i < start {
            break;
        }
        if is_js_space(c) {
            end = i;
        } else {
            break;
        }
    }
    if start >= end {
        ""
    } else {
        &text[start..end]
    }
}

/// JavaScript `\s` (WhiteSpace and LineTerminator). Every such character is in the BMP.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200a}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202f}'
            | '\u{205f}'
            | '\u{3000}'
            | '\u{feff}'
    )
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn utf16_prefix(text: &str, max: usize) -> String {
    let mut units = 0;
    let mut end = 0;
    for c in text.chars() {
        let n = c.len_utf16();
        if units + n > max {
            break;
        }
        units += n;
        end += c.len_utf8();
    }
    text[..end].to_string()
}
