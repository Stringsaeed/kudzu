//! Nonthrowing template parser and canonical serializer.
//!
//! Like the Swift original, the parser walks extended grapheme clusters, so a brace or quote
//! carrying a combining mark is not a delimiter. Ranges are UTF-16 offsets.

use std::collections::HashMap;

use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use super::{
    Attribute, AttributeValue, DateOffset, Diagnostic, Node, ParsedTemplate, Severity, Tag,
};

/// Parses `source` into nodes plus recoverable diagnostics.
pub(crate) fn parse(source: &str) -> ParsedTemplate {
    let chars: Vec<&str> = source.graphemes(true).collect();
    let n = chars.len();
    let mut offsets = Vec::with_capacity(n + 1);
    let mut acc = 0;
    offsets.push(0);
    for c in &chars {
        acc += utf16_len(c);
        offsets.push(acc);
    }

    let mut out = Output {
        nodes: Vec::new(),
        diagnostics: Vec::new(),
        literal: String::new(),
        text_start: 0,
    };
    let mut has_cursor = false;
    let mut i = 0;
    while i < n {
        if chars[i] == "\\" && i + 1 < n {
            if chars[i + 1] == "\\" {
                out.literal.push('\\');
                i += 2;
                continue;
            }
            if i + 2 < n && chars[i + 1] == "{" && chars[i + 2] == "{" {
                out.literal.push_str("{{");
                i += 3;
                continue;
            }
        }
        if !(i + 1 < n && chars[i] == "{" && chars[i + 1] == "{") {
            out.literal.push_str(chars[i]);
            i += 1;
            continue;
        }
        // A tag cannot start with "{", so in "{{{" the first brace is literal and the tag starts at the next.
        if i + 2 < n && chars[i + 2] == "{" {
            out.literal.push('{');
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + 2;
        let mut quoted = false;
        while end < n {
            if quoted && chars[end] == "\\" && end + 1 < n {
                end += 2;
                continue;
            }
            if chars[end] == "\"" {
                quoted = !quoted;
            }
            if !quoted && end + 1 < n && chars[end] == "}" && chars[end + 1] == "}" {
                break;
            }
            end += 1;
        }
        if end + 1 >= n {
            for c in &chars[start..] {
                out.literal.push_str(c);
            }
            out.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message: "Unterminated tag".into(),
                range: offsets[start]..offsets[n],
            });
            break;
        }
        let range = offsets[start]..offsets[end + 2];
        let mut scanner = TagScanner {
            chars: &chars[start + 2..end],
            i: 0,
            duplicates: Vec::new(),
            bare: HashMap::new(),
        };
        let problem = match scanner.parse() {
            Some(mut tag) => {
                tag.range = range.clone();
                coerce_bare_tokens(&mut tag, &scanner.bare);
                for name in &scanner.duplicates {
                    out.diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        message: format!("Duplicate attribute '{name}'; last value wins"),
                        range: range.clone(),
                    });
                }
                match validate(&tag) {
                    Some(problem) => Some(problem),
                    None => {
                        out.flush(&offsets, start);
                        if tag.name == "cursor" {
                            if has_cursor {
                                out.diagnostics.push(Diagnostic {
                                    severity: Severity::Warning,
                                    message: "Only the first top-level cursor counts".into(),
                                    range: range.clone(),
                                });
                            }
                            has_cursor = true;
                        }
                        if (tag.name == "date" || tag.name == "time")
                            && tag.attributes.iter().any(|a| a.name == "offset")
                            && tag_string(&tag, "offset")
                                .and_then(DateOffset::parse)
                                .is_none()
                        {
                            out.diagnostics.push(Diagnostic {
                                severity: Severity::Warning,
                                message: "Invalid date offset; ignored".into(),
                                range: range.clone(),
                            });
                        }
                        out.nodes.push(Node::Tag(tag));
                        None
                    }
                }
            }
            None => Some("Malformed tag".to_string()),
        };
        if let Some(message) = problem {
            // Unknown and malformed tags are kept as written so a newer version can still read them.
            out.flush(&offsets, start);
            out.nodes.push(Node::Tag(Tag {
                name: String::new(),
                attributes: Vec::new(),
                range: range.clone(),
                verbatim: Some(chars[start..end + 2].concat()),
            }));
            out.diagnostics.push(Diagnostic {
                severity: Severity::Error,
                message,
                range,
            });
        }
        out.text_start = end + 2;
        i = end + 2;
    }
    out.flush(&offsets, n);
    ParsedTemplate {
        nodes: out.nodes,
        diagnostics: out.diagnostics,
    }
}

struct Output {
    nodes: Vec<Node>,
    diagnostics: Vec<Diagnostic>,
    literal: String,
    text_start: usize,
}

impl Output {
    fn flush(&mut self, offsets: &[usize], end: usize) {
        if !self.literal.is_empty() {
            let text = std::mem::take(&mut self.literal);
            self.nodes.push(Node::Text {
                text,
                range: offsets[self.text_start]..offsets[end],
            });
        }
    }
}

/// UTF-16 length of `s`.
pub(crate) fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// The last string value of attribute `key`.
pub(crate) fn tag_string<'t>(tag: &'t Tag, key: &str) -> Option<&'t str> {
    match tag
        .attributes
        .iter()
        .rev()
        .find(|a| a.name == key)
        .map(|a| &a.value)
    {
        Some(AttributeValue::String(s)) => Some(s),
        _ => None,
    }
}

/// The last boolean value of attribute `key`, `false` otherwise.
pub(crate) fn tag_bool(tag: &Tag, key: &str) -> bool {
    matches!(
        tag.attributes
            .iter()
            .rev()
            .find(|a| a.name == key)
            .map(|a| &a.value),
        Some(AttributeValue::Bool(true))
    )
}

/// Swift `UUID(uuidString:)` only accepts the hyphenated 36-character form.
pub(crate) fn parse_uuid(s: &str) -> Option<Uuid> {
    if s.len() != 36 {
        return None;
    }
    Uuid::try_parse(s).ok()
}

/// The string and boolean attributes of a known tag, or `None` for an unknown name.
fn schema(name: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match name {
        "date" | "time" => (&["format", "offset", "locale"], &[]),
        "clipboard" => (&[], &["trim"]),
        "cursor" => (&[], &[]),
        "snippet" => (&["id"], &[]),
        "field" => (&["name", "default"], &["multiline"]),
        "choice" => (&["name", "options", "default"], &[]),
        _ => return None,
    })
}

/// A bare token given to a string attribute is that attribute's text as written (`default=007` is "007").
fn coerce_bare_tokens(tag: &mut Tag, bare: &HashMap<String, String>) {
    let Some((strings, _)) = schema(&tag.name) else {
        return;
    };
    for attribute in &mut tag.attributes {
        if strings.contains(&attribute.name.as_str()) {
            if let Some(token) = bare.get(&attribute.name) {
                attribute.value = AttributeValue::String(token.clone());
            }
        }
    }
}

fn validate(tag: &Tag) -> Option<String> {
    let Some((strings, bools)) = schema(&tag.name) else {
        return Some(format!("Unknown tag '{}'", tag.name));
    };
    for attribute in &tag.attributes {
        let name = attribute.name.as_str();
        let is_string = strings.contains(&name);
        let is_bool = bools.contains(&name);
        match attribute.value {
            AttributeValue::String(_) if is_string => {}
            AttributeValue::Bool(_) if is_bool => {}
            _ if is_string || is_bool => return Some(format!("Invalid type for '{name}'")),
            _ => {}
        }
    }
    if tag.name == "snippet" && tag_string(tag, "id").and_then(parse_uuid).is_none() {
        return Some("snippet requires a UUID id".into());
    }
    if (tag.name == "field" || tag.name == "choice") && tag_string(tag, "name").is_none() {
        return Some(format!("{} requires name", tag.name));
    }
    None
}

fn is_whitespace(g: &str) -> bool {
    g.chars().next().is_some_and(char::is_whitespace)
}

/// The single ASCII byte of `g`, if it is one.
fn ascii(g: &str) -> Option<u8> {
    (g.len() == 1).then(|| g.as_bytes()[0])
}

struct TagScanner<'s> {
    chars: &'s [&'s str],
    i: usize,
    duplicates: Vec<String>,
    /// Source text of attributes whose (last) value was written without quotes.
    bare: HashMap<String, String>,
}

impl TagScanner<'_> {
    fn whitespace(&mut self) {
        while self.i < self.chars.len() && is_whitespace(self.chars[self.i]) {
            self.i += 1;
        }
    }

    fn identifier(&mut self) -> Option<String> {
        let start = self.i;
        if !ascii(self.chars.get(self.i)?).is_some_and(|b| b.is_ascii_lowercase()) {
            return None;
        }
        self.i += 1;
        while self.i < self.chars.len()
            && ascii(self.chars[self.i])
                .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            self.i += 1;
        }
        Some(self.chars[start..self.i].concat())
    }

    fn parse(&mut self) -> Option<Tag> {
        self.whitespace();
        let name = self.identifier()?;
        let mut attributes: Vec<Attribute> = Vec::new();
        while self.i < self.chars.len() {
            if !is_whitespace(self.chars[self.i]) {
                return None;
            }
            self.whitespace();
            if self.i == self.chars.len() {
                break;
            }
            let key = self.identifier()?;
            self.whitespace();
            if self.chars.get(self.i) != Some(&"=") {
                return None;
            }
            self.i += 1;
            self.whitespace();
            let value_start = self.i;
            let value = self.value()?;
            if attributes.iter().any(|a| a.name == key) {
                self.duplicates.push(key.clone());
                attributes.retain(|a| a.name != key);
            }
            if self.chars[value_start] == "\"" {
                self.bare.remove(&key);
            } else {
                self.bare
                    .insert(key.clone(), self.chars[value_start..self.i].concat());
            }
            attributes.push(Attribute { name: key, value });
        }
        Some(Tag {
            name,
            attributes,
            range: 0..0,
            verbatim: None,
        })
    }

    fn value(&mut self) -> Option<AttributeValue> {
        let first = *self.chars.get(self.i)?;
        if first == "\"" {
            self.i += 1;
            let mut result = String::new();
            while self.i < self.chars.len() {
                let c = self.chars[self.i];
                if c == "\"" {
                    self.i += 1;
                    return Some(AttributeValue::String(result));
                }
                if c == "\\" {
                    let next = *self.chars.get(self.i + 1)?;
                    if !matches!(next, "\\" | "\"" | "|") {
                        return None;
                    }
                    if next == "|" {
                        // Kept escaped so `options` splitting can tell it from a separator.
                        result.push('\\');
                    }
                    result.push_str(next);
                    self.i += 2;
                } else {
                    result.push_str(c);
                    self.i += 1;
                }
            }
            return None;
        }
        let start = self.i;
        while self.i < self.chars.len()
            && ascii(self.chars[self.i])
                .is_some_and(|b| b.is_ascii_alphanumeric() || b"_.+-".contains(&b))
        {
            self.i += 1;
        }
        if self.i == start {
            return None;
        }
        let token = self.chars[start..self.i].concat();
        Some(match token.as_str() {
            "true" => AttributeValue::Bool(true),
            "false" => AttributeValue::Bool(false),
            _ => match is_plain_decimal(&token)
                .then(|| token.parse::<f64>().ok())
                .flatten()
            {
                Some(number) if number.is_finite() => AttributeValue::Number(number),
                _ => AttributeValue::String(token),
            },
        })
    }
}

/// An optional sign, digits, and an optional fraction: `5`, `-3`, `+2.50`. No exponent, hex or bare `.5`.
fn is_plain_decimal(token: &str) -> bool {
    let digits = token.strip_prefix(['+', '-']).unwrap_or(token);
    let parts: Vec<&str> = digits.splitn(2, '.').collect();
    parts
        .iter()
        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Canonical serialization; adjacent text is merged first because `{` + `{` is the two-brace text `{{`.
pub(crate) fn serialize(nodes: &[Node]) -> String {
    let mut out = String::new();
    let mut pending = String::new();
    for node in nodes {
        match node {
            Node::Text { text, .. } => pending.push_str(text),
            Node::Tag(tag) => {
                escape_into(&pending, &mut out);
                pending.clear();
                write_tag(tag, &mut out);
            }
        }
    }
    escape_into(&pending, &mut out);
    out
}

/// Writes one tag: its verbatim source, or the canonical form.
pub(crate) fn write_tag(tag: &Tag, out: &mut String) {
    if let Some(verbatim) = &tag.verbatim {
        out.push_str(verbatim);
        return;
    }
    out.push_str("{{");
    out.push_str(&tag.name);
    for attribute in &tag.attributes {
        out.push(' ');
        out.push_str(&attribute.name);
        out.push('=');
        match &attribute.value {
            AttributeValue::String(s) => {
                out.push('"');
                for c in s.chars() {
                    if c == '\\' || c == '"' {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('"');
            }
            AttributeValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            // Rust's `Display` for f64 is the shortest round-tripping decimal and never uses an exponent.
            AttributeValue::Number(n) => out.push_str(&n.to_string()),
        }
    }
    out.push_str("}}");
}

/// Literal text: `\` becomes `\\` and each `{{` pair becomes `\{{`; a lone `{` needs no escape.
fn escape_into(text: &str, out: &mut String) {
    let chars: Vec<&str> = text.graphemes(true).collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == "\\" {
            out.push_str("\\\\");
        } else if chars[i] == "{" && chars.get(i + 1) == Some(&"{") {
            out.push_str("\\{{");
            i += 2;
            continue;
        } else {
            out.push_str(chars[i]);
        }
        i += 1;
    }
}
