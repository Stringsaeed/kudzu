//! The editor's template tokenizer (`lib/template.ts`): tags, their UTF-16 ranges and
//! attributes for chips, chip labels and quick previews. Real rendering stays in
//! [`crate::template`]; this mirrors what the UI showed, including its validity rules.

use crate::template::AttributeValue;

use super::js;

/// Tag attributes with JavaScript object semantics: a repeated key keeps its first position
/// and takes the last value, `__proto__` is dropped, and iteration lists integer-like keys
/// first in numeric order, then the rest in insertion order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attrs(Vec<(String, AttributeValue)>);

fn array_index(key: &str) -> Option<u32> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    key.parse::<u32>().ok().filter(|&n| n != u32::MAX)
}

impl Attrs {
    pub fn new() -> Self {
        Self::default()
    }

    /// `attrs[key] = value`.
    pub fn insert(&mut self, key: impl Into<String>, value: AttributeValue) {
        let key = key.into();
        if key == "__proto__" {
            return;
        }
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.0.push((key, value)),
        }
    }

    pub fn get(&self, key: &str) -> Option<&AttributeValue> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The value when it is a string.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(AttributeValue::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `Object.entries(attrs)` order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &AttributeValue)> {
        let mut indexed: Vec<_> = self.0.iter().filter_map(|(k, v)| array_index(k).map(|n| (n, k, v))).collect();
        indexed.sort_by_key(|(n, _, _)| *n);
        let named = self.0.iter().filter(|(k, _)| array_index(k).is_none()).map(|(k, v)| (k.as_str(), v));
        indexed.into_iter().map(|(_, k, v)| (k.as_str(), v)).collect::<Vec<_>>().into_iter().chain(named)
    }
}

impl<K: Into<String>> FromIterator<(K, AttributeValue)> for Attrs {
    fn from_iter<I: IntoIterator<Item = (K, AttributeValue)>>(iter: I) -> Self {
        let mut attrs = Attrs::new();
        for (k, v) in iter {
            attrs.insert(k, v);
        }
        attrs
    }
}

/// JavaScript truthiness of an optional attribute.
pub fn truthy(value: Option<&AttributeValue>) -> bool {
    match value {
        None => false,
        Some(AttributeValue::String(s)) => !s.is_empty(),
        Some(AttributeValue::Bool(b)) => *b,
        Some(AttributeValue::Number(n)) => *n != 0.0 && !n.is_nan(),
    }
}

/// `String(value)`.
pub fn value_string(value: &AttributeValue) -> String {
    match value {
        AttributeValue::String(s) => s.clone(),
        AttributeValue::Bool(b) => b.to_string(),
        AttributeValue::Number(n) => js::number_to_string(*n),
    }
}

/// A `{{…}}` span. `from`/`to` are UTF-16 offsets into the source.
#[derive(Debug, Clone, PartialEq)]
pub struct TagToken {
    pub from: usize,
    pub to: usize,
    pub source: String,
    /// Empty for verbatim (unknown or malformed) tags.
    pub name: String,
    pub attrs: Attrs,
    /// Unknown or malformed `{{…}}` span, kept as written.
    pub verbatim: bool,
}

/// Literal text, unescaped. `from`/`to` are UTF-16 offsets into the source.
#[derive(Debug, Clone, PartialEq)]
pub struct TextToken {
    pub from: usize,
    pub to: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Tag(TagToken),
    Text(TextToken),
}

impl Token {
    pub fn from(&self) -> usize {
        match self {
            Token::Tag(t) => t.from,
            Token::Text(t) => t.from,
        }
    }

    pub fn to(&self) -> usize {
        match self {
            Token::Tag(t) => t.to,
            Token::Text(t) => t.to,
        }
    }
}

pub const KNOWN_TAGS: [&str; 7] = ["date", "time", "clipboard", "cursor", "snippet", "field", "choice"];
const STRING_ATTRS: [&str; 7] = ["default", "name", "format", "options", "offset", "locale", "id"];

fn is(u: Option<&u16>, f: fn(u8) -> bool) -> bool {
    match u {
        Some(&u) if u < 128 => f(u as u8),
        _ => false,
    }
}

fn name_start(b: u8) -> bool {
    b.is_ascii_lowercase()
}
fn name_char(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'
}
fn key_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}
fn bare_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'+' | b'-')
}

const fn unit(c: u8) -> u16 {
    c as u16
}

/// `/^[+-]?\d+(\.\d+)?$/`.
fn is_number(s: &str) -> bool {
    let s = s.strip_prefix(['+', '-']).unwrap_or(s);
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s, None),
    };
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    digits(int) && frac.is_none_or(digits)
}

fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| if matches!(i, 8 | 13 | 18 | 23) { *c == b'-' } else { c.is_ascii_hexdigit() })
}

/// The `}}` closing a tag that starts at `start` (the `{{`), honouring quotes.
fn find_close(s: &[u16], start: usize) -> Option<usize> {
    let mut i = start + 2;
    let mut quoted = false;
    while i < s.len() {
        let c = s[i];
        if quoted {
            if c == unit(b'\\') && i + 1 < s.len() {
                i += 2;
            } else {
                if c == unit(b'"') {
                    quoted = false;
                }
                i += 1;
            }
            continue;
        }
        if c == unit(b'"') {
            quoted = true;
        } else if c == unit(b'}') && s.get(i + 1) == Some(&unit(b'}')) {
            return Some(i + 2);
        }
        i += 1;
    }
    None
}

/// Parses the inside of a closed tag; `None` when malformed.
fn parse_inner(inner: &[u16]) -> Option<(String, Attrs)> {
    let mut i = 0;
    let ws = |i: &mut usize| {
        while *i < inner.len() && js::is_space_unit(inner[*i]) {
            *i += 1;
        }
    };
    ws(&mut i);
    if !is(inner.get(i), name_start) {
        return None;
    }
    let start = i;
    while is(inner.get(i), name_char) {
        i += 1;
    }
    let name = js::from16(&inner[start..i]);
    let mut attrs = Attrs::new();
    loop {
        let before = i;
        ws(&mut i);
        if i >= inner.len() {
            break;
        }
        if i == before {
            return None; // attributes must be separated by whitespace
        }
        let start = i;
        while is(inner.get(i), key_char) {
            i += 1;
        }
        if i == start {
            return None;
        }
        let key = js::from16(&inner[start..i]);
        ws(&mut i);
        if inner.get(i) != Some(&unit(b'=')) {
            return None;
        }
        i += 1;
        ws(&mut i);
        if inner.get(i) == Some(&unit(b'"')) {
            i += 1;
            let mut value = Vec::new();
            let mut closed = false;
            while i < inner.len() {
                let c = inner[i];
                let next = inner.get(i + 1).copied();
                if c == unit(b'\\') && (next == Some(unit(b'"')) || next == Some(unit(b'\\'))) {
                    value.push(inner[i + 1]);
                    i += 2;
                } else if c == unit(b'"') {
                    closed = true;
                    i += 1;
                    break;
                } else {
                    value.push(c);
                    i += 1;
                }
            }
            if !closed {
                return None;
            }
            attrs.insert(key, AttributeValue::String(js::from16(&value)));
        } else {
            let start = i;
            while is(inner.get(i), bare_char) {
                i += 1;
            }
            if i == start {
                return None;
            }
            let bare = js::from16(&inner[start..i]);
            let value = if STRING_ATTRS.contains(&key.as_str()) {
                AttributeValue::String(bare)
            } else if bare == "true" || bare == "false" {
                AttributeValue::Bool(bare == "true")
            } else if is_number(&bare) {
                AttributeValue::Number(bare.parse().unwrap_or(f64::NAN))
            } else {
                AttributeValue::String(bare)
            };
            attrs.insert(key, value);
        }
    }
    Some((name, attrs))
}

fn is_valid_known(name: &str, attrs: &Attrs) -> bool {
    let str_or_none = |k: &str| matches!(attrs.get(k), None | Some(AttributeValue::String(_)));
    let bool_or_none = |k: &str| matches!(attrs.get(k), None | Some(AttributeValue::Bool(_)));
    match name {
        "date" | "time" => str_or_none("format") && str_or_none("locale"),
        "clipboard" => bool_or_none("trim"),
        "cursor" => true,
        "snippet" => attrs.str("id").is_some_and(is_uuid),
        "field" => attrs.str("name").is_some() && str_or_none("default") && bool_or_none("multiline"),
        "choice" => attrs.str("name").is_some() && str_or_none("options") && str_or_none("default"),
        _ => false,
    }
}

/// Splits template source into text and tag tokens. Never fails.
pub fn tokenize(source: &str) -> Vec<Token> {
    let s = js::utf16(source);
    let mut tokens = Vec::new();
    let mut text: Vec<u16> = Vec::new();
    let mut text_from = 0;
    let flush = |tokens: &mut Vec<Token>, text: &mut Vec<u16>, text_from: &mut usize, at: usize| {
        if !text.is_empty() {
            tokens.push(Token::Text(TextToken { from: *text_from, to: at, text: js::from16(text) }));
        }
        text.clear();
        *text_from = at;
    };
    let at = |i: usize| s.get(i).copied();
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        if c == unit(b'\\') && at(i + 1) == Some(unit(b'\\')) {
            text.push(unit(b'\\'));
            i += 2;
            continue;
        }
        if c == unit(b'\\') && at(i + 1) == Some(unit(b'{')) && at(i + 2) == Some(unit(b'{')) {
            text.extend([unit(b'{'), unit(b'{')]);
            i += 3;
            continue;
        }
        if c == unit(b'{') && at(i + 1) == Some(unit(b'{')) && at(i + 2) != Some(unit(b'{')) {
            let Some(close) = find_close(&s, i) else {
                text.extend([unit(b'{'), unit(b'{')]);
                i += 2;
                continue;
            };
            flush(&mut tokens, &mut text, &mut text_from, i);
            let raw = &s[i..close];
            let parsed = parse_inner(&raw[2..raw.len() - 2]).filter(|(name, attrs)| is_valid_known(name, attrs));
            let verbatim = parsed.is_none();
            let (name, attrs) = parsed.unwrap_or_default();
            tokens.push(Token::Tag(TagToken { from: i, to: close, source: js::from16(raw), name, attrs, verbatim }));
            i = close;
            text_from = i;
            continue;
        }
        text.push(c);
        i += 1;
    }
    flush(&mut tokens, &mut text, &mut text_from, s.len());
    tokens
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Serializes a tag canonically: strings quoted, booleans and numbers bare, attribute order
/// kept (as `Object.entries` lists it). `None` and empty strings are left out.
pub fn build_tag<K: Into<String>>(name: &str, attrs: impl IntoIterator<Item = (K, Option<AttributeValue>)>) -> String {
    // Absent values still claim their key's position, like `{ a: undefined }` in an object literal.
    let mut ordered: Vec<(String, Option<AttributeValue>)> = Vec::new();
    for (k, v) in attrs {
        let k = k.into();
        if k == "__proto__" {
            continue;
        }
        match ordered.iter_mut().find(|(key, _)| *key == k) {
            Some(slot) => slot.1 = v,
            None => ordered.push((k, v)),
        }
    }
    let mut indexed: Vec<_> = ordered.iter().filter(|(k, _)| array_index(k).is_some()).collect();
    indexed.sort_by_key(|(k, _)| array_index(k));
    let named = ordered.iter().filter(|(k, _)| array_index(k).is_none());
    let mut parts = vec![name.to_string()];
    for (k, v) in indexed.into_iter().chain(named) {
        match v {
            None => {}
            Some(AttributeValue::String(s)) if s.is_empty() => {}
            Some(AttributeValue::String(s)) => parts.push(format!("{k}={}", quote(s))),
            Some(other) => parts.push(format!("{k}={}", value_string(other))),
        }
    }
    format!("{{{{{}}}}}", parts.join(" "))
}

/// Escapes literal text so it survives a round trip through the parser.
pub fn escape_text(text: &str) -> String {
    text.replace('\\', "\\\\").replace("{{", "\\{{")
}

/// Splits `options` on unescaped pipes (`\|` is a literal pipe).
pub use crate::template::split_options;

pub fn join_options(options: &[impl AsRef<str>]) -> String {
    options.iter().map(|o| o.as_ref().replace('|', "\\|")).collect::<Vec<_>>().join("|")
}

/// Date format presets and their labels, in menu order.
pub const DATE_FORMAT_PRESETS: [(&str, &str); 11] = [
    ("yyyy-MM-dd", "ISO date"),
    ("MMMM d, yyyy", "Long date"),
    ("MMM d, yyyy", "Medium date"),
    ("M/d/yy", "Short date"),
    ("d MMMM yyyy", "Day month year"),
    ("EEEE", "Weekday"),
    ("EEEE, MMMM d", "Weekday + date"),
    ("HH:mm", "24-hour time"),
    ("h:mm a", "12-hour time"),
    ("HH:mm:ss", "Time with seconds"),
    ("yyyy-MM-dd'T'HH:mm:ssXXX", "ISO timestamp"),
];

fn preset_label(format: &str) -> Option<&'static str> {
    DATE_FORMAT_PRESETS.iter().find(|(f, _)| *f == format).map(|(_, l)| *l)
}

/// Resolves a nested snippet's id, exactly as written in the tag, to its name.
pub type SnippetName<'a> = Option<&'a dyn Fn(&str) -> Option<String>>;

/// Human label for a chip.
pub fn chip_label(tag: &TagToken, snippet_name: SnippetName<'_>) -> String {
    if tag.verbatim {
        return tag.source.clone();
    }
    let a = &tag.attrs;
    let s = |k: &str| a.get(k).map(value_string).unwrap_or_else(|| "undefined".into());
    match tag.name.as_str() {
        "date" | "time" => {
            let format = match a.get("format") {
                Some(v) => value_string(v),
                None => (if tag.name == "date" { "yyyy-MM-dd" } else { "HH:mm" }).to_string(),
            };
            let base = preset_label(&format).map(str::to_string).unwrap_or(format);
            let off = if truthy(a.get("offset")) { format!(" {}", s("offset")) } else { String::new() };
            format!("{} · {base}{off}", if tag.name == "date" { "Date" } else { "Time" })
        }
        "clipboard" => (if truthy(a.get("trim")) { "Clipboard (trimmed)" } else { "Clipboard" }).into(),
        "cursor" => "Cursor".into(),
        "snippet" => {
            let name = snippet_name.and_then(|f| f(&s("id")));
            format!("↳ {}", name.unwrap_or_else(|| "Missing snippet".into()))
        }
        "field" => format!("{}{}", s("name"), if truthy(a.get("multiline")) { " ¶" } else { "" }),
        "choice" => format!("{} ▾", s("name")),
        _ => tag.source.clone(),
    }
}

/// Chip colour family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChipTone {
    Time,
    Clipboard,
    Cursor,
    Snippet,
    Form,
    Error,
}

pub fn chip_tone(tag: &TagToken) -> ChipTone {
    if tag.verbatim {
        return ChipTone::Error;
    }
    match tag.name.as_str() {
        "date" | "time" => ChipTone::Time,
        "clipboard" => ChipTone::Clipboard,
        "cursor" => ChipTone::Cursor,
        "snippet" => ChipTone::Snippet,
        _ => ChipTone::Form,
    }
}

/// What a template does, for badges and the palette.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateFacts {
    pub is_form: bool,
    /// Field and choice names, first appearance order, without repeats.
    pub field_names: Vec<String>,
    pub uses_clipboard: bool,
    pub has_cursor: bool,
    /// Nested snippet ids as written, repeats kept.
    pub snippet_ids: Vec<String>,
    pub has_errors: bool,
}

pub fn analyze(source: &str) -> TemplateFacts {
    let mut facts = TemplateFacts::default();
    for t in tokenize(source) {
        let Token::Tag(t) = t else { continue };
        if t.verbatim {
            facts.has_errors = true;
        } else if t.name == "field" || t.name == "choice" {
            facts.is_form = true;
            let n = t.attrs.str("name").unwrap_or_default().to_string();
            if !facts.field_names.contains(&n) {
                facts.field_names.push(n);
            }
        } else if t.name == "clipboard" {
            facts.uses_clipboard = true;
        } else if t.name == "cursor" {
            facts.has_cursor = true;
        } else if t.name == "snippet" {
            facts.snippet_ids.push(t.attrs.str("id").unwrap_or_default().to_string());
        }
    }
    facts
}

/// Default `max` of [`plain_summary`].
pub const SUMMARY_MAX: usize = 140;

/// [`plain_summary`] as UTF-16 units: the cut at `max` can split a surrogate pair, as in JS.
pub fn plain_summary16(source: &str, snippet_name: SnippetName<'_>, max: usize) -> Vec<u16> {
    let mut out = String::new();
    let mut len = 0;
    for t in tokenize(source) {
        let piece = match &t {
            Token::Text(t) => t.text.clone(),
            Token::Tag(tag) => format!("‹{}›", chip_label(tag, snippet_name)),
        };
        len += js::len16(&piece);
        out.push_str(&piece);
        if len > max {
            break;
        }
    }
    let mut units = js::utf16(js::trim(&js::collapse_spaces(&out)));
    units.truncate(max);
    units
}

/// Plain-text one-line summary of content for list rows (tags shown as their labels), at most
/// `max` UTF-16 units. A surrogate pair cut in half at the end becomes U+FFFD.
pub fn plain_summary(source: &str, snippet_name: SnippetName<'_>, max: usize) -> String {
    js::from16(&plain_summary16(source, snippet_name, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(t: &Token) -> &TagToken {
        match t {
            Token::Tag(t) => t,
            Token::Text(_) => panic!("expected a tag, got {t:?}"),
        }
    }

    fn text(t: &Token) -> (&str, usize, usize) {
        match t {
            Token::Text(t) => (&t.text, t.from, t.to),
            Token::Tag(_) => panic!("expected text, got {t:?}"),
        }
    }

    #[test]
    fn tokenizes_text_and_tags_with_utf16_ranges() {
        let tokens = tokenize("😀 hi {{date format=\"EEEE\"}}!");
        assert_eq!(tokens.len(), 3);
        assert_eq!(text(&tokens[0]), ("😀 hi ", 0, 6));
        let t = tag(&tokens[1]);
        assert_eq!((t.from, t.to, t.name.as_str(), t.verbatim), (6, 28, "date", false));
        assert_eq!(t.attrs.str("format"), Some("EEEE"));
        assert_eq!(text(&tokens[2]), ("!", 28, 29));
    }

    #[test]
    fn escapes_and_unclosed_braces_are_text() {
        let tokens = tokenize(r"a \{{x}} \\ {{b");
        assert_eq!(tokens.len(), 1);
        assert_eq!(text(&tokens[0]), (r"a {{x}} \ {{b", 0, 15));
        // `{{{` starts with a literal brace.
        let tokens = tokenize("{{{cursor}}");
        assert_eq!(text(&tokens[0]), ("{", 0, 1));
        assert_eq!(tag(&tokens[1]).name, "cursor");
        // Text resumes after the tag, even when the gap is empty.
        let tokens = tokenize("{{cursor}}{{cursor}}x");
        assert_eq!(tokens.len(), 3);
        assert_eq!(text(&tokens[2]), ("x", 20, 21));
    }

    #[test]
    fn attribute_types_and_validation() {
        let t = tokenize(r#"{{field name=who default="a \"b\" \\ \c" multiline=true}}"#);
        let t = tag(&t[0]);
        assert!(!t.verbatim);
        assert_eq!(t.attrs.str("default"), Some(r#"a "b" \ \c"#));
        assert_eq!(t.attrs.get("multiline"), Some(&AttributeValue::Bool(true)));
        // Numbers parse unless the attribute is a string attribute.
        let t = tokenize("{{clipboard x=-1.5 y=+2 name=3 z=1e5}}");
        let t = tag(&t[0]);
        assert_eq!(t.attrs.get("x"), Some(&AttributeValue::Number(-1.5)));
        assert_eq!(t.attrs.get("y"), Some(&AttributeValue::Number(2.0)));
        assert_eq!(t.attrs.get("name"), Some(&AttributeValue::String("3".into())));
        assert_eq!(t.attrs.get("z"), Some(&AttributeValue::String("1e5".into())));
        // Invalid: wrong types, unknown names, missing separators, unclosed quotes.
        for bad in [
            "{{clipboard trim=yes}}",
            "{{field}}",
            "{{field name=a multiline=\"true\"}}",
            "{{snippet id=nope}}",
            "{{wat}}",
            "{{Date}}",
            "{{ }}",
            "{{date format=\"x\"locale=\"y\"}}",
            "{{date format=}}",
            "{{date =x}}",
            "{{date format \"x\"}}",
        ] {
            let tokens = tokenize(bad);
            let t = tag(&tokens[0]);
            assert!(t.verbatim, "{bad}");
            assert_eq!((t.name.as_str(), t.attrs.is_empty(), t.source.as_str()), ("", true, bad));
        }
        assert!(!tag(&tokenize("{{snippet id=0A1B2C3D-0000-4000-8000-00000000000F}}")[0]).verbatim);
        // A quoted `}}` doesn't close the tag.
        let tokens = tokenize(r#"{{choice name=x options="a}}|b"}} z"#);
        assert_eq!(tag(&tokens[0]).attrs.str("options"), Some("a}}|b"));
        assert_eq!(text(&tokens[1]).0, " z");
    }

    #[test]
    fn attrs_follow_object_semantics() {
        let tokens = tokenize("{{cursor b=1 2=x a=2 b=3 0=y __proto__=z}}");
        let t = tag(&tokens[0]);
        let keys: Vec<_> = t.attrs.iter().map(|(k, _)| k.to_string()).collect();
        assert_eq!(keys, ["0", "2", "b", "a"]);
        assert_eq!(t.attrs.get("b"), Some(&AttributeValue::Number(3.0)));
    }

    #[test]
    fn builds_tags() {
        let s = |v: &str| Some(AttributeValue::String(v.into()));
        assert_eq!(build_tag("field", [("name", s("Who \"me\"")), ("default", s("")), ("multiline", Some(AttributeValue::Bool(true)))]), r#"{{field name="Who \"me\"" multiline=true}}"#);
        assert_eq!(build_tag::<&str>("cursor", []), "{{cursor}}");
        assert_eq!(build_tag("x", [("n", Some(AttributeValue::Number(2.0))), ("m", None), ("1", s("a"))]), r#"{{x 1="a" n=2}}"#);
        assert_eq!(escape_text(r"a\{{b"), r"a\\\{{b");
        assert_eq!(split_options(r"a|b\|c|"), ["a", "b|c", ""]);
        assert!(split_options("").is_empty());
        assert_eq!(join_options(&["a|b", "c"]), r"a\|b|c");
    }

    #[test]
    fn chip_labels_and_tones() {
        let label = |src: &str| chip_label(tag(&tokenize(src)[0]), None);
        assert_eq!(label("{{date}}"), "Date · ISO date");
        assert_eq!(label("{{time}}"), "Time · 24-hour time");
        assert_eq!(label(r#"{{date format="EEEE" offset=+1d}}"#), "Date · Weekday +1d");
        assert_eq!(label(r#"{{time format="H"}}"#), "Time · H");
        assert_eq!(label(r#"{{date format=""}}"#), "Date · ");
        assert_eq!(label("{{clipboard trim=true}}"), "Clipboard (trimmed)");
        assert_eq!(label("{{clipboard}}"), "Clipboard");
        assert_eq!(label("{{cursor}}"), "Cursor");
        assert_eq!(label("{{field name=who multiline=true}}"), "who ¶");
        assert_eq!(label("{{choice name=size}}"), "size ▾");
        assert_eq!(label("{{nope}}"), "{{nope}}");
        let id = "00000000-0000-0000-0000-000000000001";
        let src = format!("{{{{snippet id=\"{id}\"}}}}");
        assert_eq!(label(&src), "↳ Missing snippet");
        let resolve = |s: &str| (s == id).then(|| "Sig".to_string());
        assert_eq!(chip_label(tag(&tokenize(&src)[0]), Some(&resolve)), "↳ Sig");
        let tone = |src: &str| chip_tone(tag(&tokenize(src)[0]));
        assert_eq!([tone("{{date}}"), tone("{{clipboard}}"), tone("{{cursor}}"), tone(&src), tone("{{field name=a}}"), tone("{{x}}")], [
            ChipTone::Time,
            ChipTone::Clipboard,
            ChipTone::Cursor,
            ChipTone::Snippet,
            ChipTone::Form,
            ChipTone::Error
        ]);
    }

    #[test]
    fn analyzes_templates() {
        let id = "00000000-0000-0000-0000-000000000001";
        let facts = analyze(&format!("{{{{field name=a}}}}{{{{choice name=b}}}}{{{{field name=a}}}}{{{{clipboard}}}}{{{{cursor}}}}{{{{snippet id={id}}}}}{{{{snippet id={id}}}}}{{{{bad}}}}"));
        assert_eq!(facts, TemplateFacts {
            is_form: true,
            field_names: vec!["a".into(), "b".into()],
            uses_clipboard: true,
            has_cursor: true,
            snippet_ids: vec![id.into(), id.into()],
            has_errors: true,
        });
        assert_eq!(analyze("plain"), TemplateFacts::default());
    }

    #[test]
    fn plain_summaries() {
        assert_eq!(plain_summary("  Hello\n\n {{field name=who}}  {{cursor}} ", None, SUMMARY_MAX), "Hello ‹who› ‹Cursor›");
        assert_eq!(plain_summary("abcdef", None, 3), "abc");
        // The cut counts UTF-16 units and can halve an emoji.
        assert_eq!(plain_summary("a😀b", None, 2), "a\u{FFFD}");
        assert_eq!(plain_summary16("a😀b", None, 2), [u16::from(b'a'), 0xD83D]);
    }
}
