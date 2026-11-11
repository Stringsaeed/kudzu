//! HTML tag scanning: where a tag ends, raw-text elements, attributes, and which elements are
//! void or block-level.

use super::entities::decode_entities;

pub(super) const VOID: &[&str] = &[
    "br", "img", "hr", "input", "meta", "link", "wbr", "col", "area", "base", "embed", "source",
    "track", "param",
];

pub(super) const BLOCKS: &[&str] = &[
    "div",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "main",
    "aside",
    "address",
    "figure",
    "figcaption",
    "center",
    "dl",
    "dt",
    "dd",
    "form",
    "fieldset",
    "blockquote",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "caption",
    "pre",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
];
pub(super) fn tag_end(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// Elements whose content is not text to keep: where their content ends (after the close tag).
pub(super) fn raw_text_end(name: &str, rest: &str) -> Option<usize> {
    if !matches!(
        name,
        "script"
            | "style"
            | "head"
            | "title"
            | "template"
            | "noscript"
            | "svg"
            | "math"
            | "iframe"
            | "object"
            | "video"
            | "audio"
            | "canvas"
            | "select"
            | "textarea"
    ) {
        return None;
    }
    let close = format!("</{name}");
    let bytes = rest.as_bytes();
    let mut from = 0;
    while let Some(p) = rest[from..].find("</") {
        let at = from + p;
        if bytes
            .get(at..at + close.len())
            .is_some_and(|b| b.eq_ignore_ascii_case(close.as_bytes()))
        {
            return Some(rest[at..].find('>').map_or(rest.len(), |e| at + e + 1));
        }
        from = at + 2;
    }
    Some(rest.len())
}

pub(super) struct Tag {
    pub(super) name: String,
    pub(super) closing: bool,
    pub(super) self_closing: bool,
    pub(super) attrs: Vec<(String, String)>,
}

impl Tag {
    pub(super) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

pub(super) fn parse_tag(body: &str) -> Tag {
    let closing = body.starts_with('/');
    let body = body.trim_start_matches('/');
    let self_closing = body.trim_end().ends_with('/');
    let name_len = body
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(body.len());
    let name = body[..name_len].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let chars: Vec<char> = body[name_len..].chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && !matches!(chars[i], '=' | '/' | '>') {
            i += 1;
        }
        let key: String = chars[start..i]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if chars.get(i) == Some(&'=') {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if let Some(&q) = chars.get(i).filter(|c| **c == '"' || **c == '\'') {
                i += 1;
                let v = i;
                while i < chars.len() && chars[i] != q {
                    i += 1;
                }
                value = chars[v..i].iter().collect();
                i += 1;
            } else {
                let v = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = chars[v..i].iter().collect();
            }
        }
        if key.is_empty() {
            i += 1;
        } else {
            attrs.push((key, decode_entities(&value)));
        }
    }
    Tag {
        name,
        closing,
        self_closing,
        attrs,
    }
}
