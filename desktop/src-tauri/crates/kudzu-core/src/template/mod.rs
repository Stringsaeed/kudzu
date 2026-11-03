//! Template syntax (docs/TEMPLATE_SYNTAX.md): parse, serialize, render, analyze.
//!
//! PUBLIC API CONTRACT — other crates compile against these exact signatures.
//! Implementation may be split into submodules; keep these re-exported here.

mod analysis;
mod icu;
mod parser;
mod render;

use std::collections::HashMap;
use std::ops::Range;

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Diagnostic importance for editor presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Warning,
    Error,
}

/// A source-located parser or renderer observation. `range` is half-open UTF-16 offsets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub range: Range<usize>,
}

/// A typed attribute value; quoted tokens always remain strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
pub enum AttributeValue {
    String(String),
    Bool(bool),
    Number(f64),
}

/// An ordered tag attribute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
}

/// A tag with ordered attributes and UTF-16 source range (including delimiters).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tag {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub range: Range<usize>,
    /// Exact source of an unknown or malformed `{{…}}` span; `None` for a recognized tag.
    pub verbatim: Option<String>,
}

/// A literal segment or a tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Node {
    Text { text: String, range: Range<usize> },
    Tag(Tag),
}

/// A nonthrowing parse result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedTemplate {
    pub nodes: Vec<Node>,
    pub diagnostics: Vec<Diagnostic>,
}

/// A signed calendar offset (`+1w2d`, `-3m`, `+30min`) with components applied in written order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateOffset {
    /// (unit, signed amount); unit is one of d, w, m, y, h, min.
    pub parts: Vec<(String, i64)>,
}

/// Typed view of a tag using v1 defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateTag {
    Date { format: String, offset: Option<DateOffset>, locale: Option<String> },
    Time { format: String, offset: Option<DateOffset>, locale: Option<String> },
    Clipboard { trim: bool },
    Cursor,
    Snippet { id: Uuid },
    Field { name: String, default: Option<String>, multiline: bool },
    Choice { name: String, options: Vec<String>, default: Option<String> },
    Unknown,
}

/// Form control kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormFieldKind {
    Text,
    Multiline,
    Choice,
}

/// A form control requested by a template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormFieldSpec {
    pub name: String,
    pub kind: FormFieldKind,
    pub options: Vec<String>,
    pub default_value: Option<String>,
}

/// Static facts about one template source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInfo {
    pub form_fields: Vec<FormFieldSpec>,
    pub referenced_snippet_ids: Vec<Uuid>,
    pub has_cursor: bool,
    pub uses_clipboard: bool,
}

/// Explicit inputs for deterministic rendering.
pub struct RenderContext<'a> {
    /// The instant (with the local UTC offset) used by every date/time tag.
    pub now: DateTime<FixedOffset>,
    /// Default formatting locale, e.g. `en_US`.
    pub locale: String,
    /// Plain-text clipboard snapshot.
    pub clipboard: Option<String>,
    /// Form values keyed by field name.
    pub form_values: HashMap<String, String>,
    /// Resolves a snippet ID to its template **source**.
    pub resolve_snippet: &'a dyn Fn(Uuid) -> Option<String>,
    /// The snippet being rendered; referencing it is a cycle even at the top level.
    pub root_snippet_id: Option<Uuid>,
}

/// Rendered text, caret placement and diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderResult {
    pub text: String,
    /// Caret offset in UTF-16 code units from the start of `text`, if a root `{{cursor}}` exists.
    pub cursor_offset: Option<usize>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses template source. Never fails; problems become diagnostics.
pub fn parse(source: &str) -> ParsedTemplate {
    parser::parse(source)
}

/// Canonical serialization of nodes back to source (see TEMPLATE_SYNTAX.md).
pub fn serialize(nodes: &[Node]) -> String {
    parser::serialize(nodes)
}

/// Typed interpretation of a tag.
pub fn typed(tag: &Tag) -> TemplateTag {
    use parser::{parse_uuid, tag_bool, tag_string};
    if tag.verbatim.is_some() {
        return TemplateTag::Unknown;
    }
    let string = |key: &str| tag_string(tag, key).map(str::to_string);
    match tag.name.as_str() {
        "date" | "time" => {
            let is_date = tag.name == "date";
            let format = string("format").unwrap_or_else(|| if is_date { "yyyy-MM-dd" } else { "HH:mm" }.to_string());
            let offset = tag_string(tag, "offset").and_then(DateOffset::parse);
            let locale = string("locale");
            if is_date {
                TemplateTag::Date { format, offset, locale }
            } else {
                TemplateTag::Time { format, offset, locale }
            }
        }
        "clipboard" => TemplateTag::Clipboard { trim: tag_bool(tag, "trim") },
        "cursor" => TemplateTag::Cursor,
        "snippet" => match tag_string(tag, "id").and_then(parse_uuid) {
            Some(id) => TemplateTag::Snippet { id },
            None => TemplateTag::Unknown,
        },
        "field" => match string("name") {
            Some(name) => TemplateTag::Field { name, default: string("default"), multiline: tag_bool(tag, "multiline") },
            None => TemplateTag::Unknown,
        },
        "choice" => match string("name") {
            Some(name) => TemplateTag::Choice {
                name,
                options: split_options(tag_string(tag, "options").unwrap_or("")),
                default: string("default"),
            },
            None => TemplateTag::Unknown,
        },
        _ => TemplateTag::Unknown,
    }
}

/// Splits a `choice` options string on `|`; `\|` is a literal pipe. Empty input gives no options.
pub fn split_options(source: &str) -> Vec<String> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut item = String::new();
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'|') {
            item.push('|');
            chars.next();
        } else if c == '|' {
            result.push(std::mem::take(&mut item));
        } else {
            item.push(c);
        }
    }
    result.push(item);
    result
}

/// Renders `source` with at most eight nested snippet levels.
pub fn render(source: &str, context: &RenderContext<'_>) -> RenderResult {
    render::render(source, context)
}

/// Analyzes recognized tags of one source; the first definition of each form name wins.
pub fn analyze(parsed: &ParsedTemplate) -> TemplateInfo {
    analysis::analyze(parsed)
}

/// Form fields of `source` including nested snippets (depth-first, cycle-safe, depth ≤ 8).
pub fn form_fields(source: &str, resolve_snippet: &dyn Fn(Uuid) -> Option<String>) -> Vec<FormFieldSpec> {
    analysis::form_fields(source, resolve_snippet)
}

/// Formats `date` with an ICU/`DateFormatter` pattern (`yyyy-MM-dd`, `EEEE, MMMM d`, `h:mm a`…)
/// in `locale`. Unsupported letters are emitted literally.
pub fn format_icu(date: &DateTime<FixedOffset>, pattern: &str, locale: &str) -> String {
    icu::format(date, pattern, locale)
}

/// Last submitted form values per snippet (bounded to 200 snippets).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FormMemory {
    pub values: HashMap<Uuid, HashMap<String, String>>,
}

impl FormMemory {
    /// Valid remembered values, then defaults, then the first choice, else empty text.
    pub fn initial_values(&self, snippet_id: Uuid, fields: &[FormFieldSpec]) -> HashMap<String, String> {
        let remembered = self.values.get(&snippet_id);
        fields
            .iter()
            .map(|field| {
                let is_choice = field.kind == FormFieldKind::Choice;
                let candidates = [remembered.and_then(|r| r.get(&field.name)), field.default_value.as_ref()];
                let value = candidates
                    .into_iter()
                    .flatten()
                    .find(|v| !is_choice || field.options.contains(v))
                    .or(if is_choice { field.options.first() } else { None })
                    .cloned()
                    .unwrap_or_default();
                (field.name.clone(), value)
            })
            .collect()
    }

    /// Records a submission.
    pub fn record(&mut self, submitted: HashMap<String, String>, snippet_id: Uuid) {
        if !self.values.contains_key(&snippet_id) && self.values.len() >= Self::CAPACITY {
            // Deterministic eviction, as in the Swift app: the smallest UUID goes.
            if let Some(victim) = self.values.keys().min().copied() {
                self.values.remove(&victim);
            }
        }
        self.values.insert(snippet_id, submitted);
    }

    /// Maximum number of snippets remembered.
    pub const CAPACITY: usize = 200;

    /// Drops memory for snippets that no longer exist.
    pub fn retain_snippets(&mut self, ids: &std::collections::HashSet<Uuid>) {
        self.values.retain(|k, _| ids.contains(k));
    }
}
