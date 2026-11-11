//! Template syntax (docs/TEMPLATE_SYNTAX.md): parse, serialize, render, analyze, and the form
//! model.
//!
//! - `parser`, `render`, `analysis`, `icu`: the grammar, the renderer, static analysis and
//!   the ICU date formatter, behind the functions below.
//! - [`forms`]: form fields and form memory.
//! - [`doc`], [`chips`], [`presets`], [`summary`], [`lists`], [`images`]: the editor document,
//!   filled by S23.

mod analysis;
pub mod chips;
pub mod doc;
pub mod forms;
mod icu;
pub mod images;
pub mod lists;
mod parser;
pub mod presets;
mod render;
pub mod summary;

pub use forms::{FormFieldKind, FormFieldSpec, FormMemory};

use std::collections::HashMap;
use std::ops::Range;

use chrono::{DateTime, FixedOffset};
#[cfg(test)]
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Diagnostic importance for editor presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum Severity {
    Warning,
    Error,
}

/// A source-located parser or renderer observation. `range` is half-open UTF-16 offsets.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub range: Range<usize>,
}

/// A typed attribute value; quoted tokens always remain strings.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValue {
    String(String),
    Bool(bool),
    Number(f64),
}

/// An ordered tag attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Attribute {
    pub name: String,
    pub value: AttributeValue,
}

/// A tag with ordered attributes and UTF-16 source range (including delimiters).
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub name: String,
    pub attributes: Vec<Attribute>,
    pub range: Range<usize>,
    /// Exact source of an unknown or malformed `{{…}}` span; `None` for a recognized tag.
    pub verbatim: Option<String>,
}

/// A literal segment or a tag.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Text { text: String, range: Range<usize> },
    Tag(Tag),
}

/// A nonthrowing parse result.
#[derive(Debug, Clone, PartialEq)]
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

/// A key `{{key}}` can press during insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub enum Key {
    Tab,
    Return,
    Escape,
    Left,
    Right,
    Up,
    Down,
    /// Deletes backward (the Mac's Delete key).
    Backspace,
    /// Deletes forward.
    Delete,
    Space,
}

impl Key {
    /// Every key, in the order the editor lists them.
    pub const ALL: [Key; 10] = [
        Key::Tab,
        Key::Return,
        Key::Escape,
        Key::Space,
        Key::Backspace,
        Key::Delete,
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
    ];

    /// Parses a `name` attribute. `enter` and `esc` are accepted spellings of `return` and `escape`.
    pub fn parse(name: &str) -> Option<Key> {
        Some(match name {
            "tab" => Key::Tab,
            "return" | "enter" => Key::Return,
            "escape" | "esc" => Key::Escape,
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "backspace" => Key::Backspace,
            "delete" => Key::Delete,
            "space" => Key::Space,
            _ => return None,
        })
    }

    /// The canonical `name` attribute.
    pub fn name(self) -> &'static str {
        match self {
            Key::Tab => "tab",
            Key::Return => "return",
            Key::Escape => "escape",
            Key::Left => "left",
            Key::Right => "right",
            Key::Up => "up",
            Key::Down => "down",
            Key::Backspace => "backspace",
            Key::Delete => "delete",
            Key::Space => "space",
        }
    }
}

/// Most presses one `{{key}}` tag can ask for.
pub const MAX_KEY_COUNT: u32 = 50;
/// Longest pause of one `{{delay}}` tag, in milliseconds.
pub const MAX_DELAY_MS: u32 = 5_000;
/// Longest total pause of one expansion, in milliseconds; later delays are cut short.
pub const MAX_TOTAL_DELAY_MS: u32 = 10_000;
/// Oldest clipboard-history item `{{clipboard history=N}}` can ask for.
pub const MAX_CLIPBOARD_HISTORY: u32 = 99;

/// Typed view of a tag using v1 defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplateTag {
    Date {
        format: String,
        offset: Option<DateOffset>,
        locale: Option<String>,
    },
    Time {
        format: String,
        offset: Option<DateOffset>,
        locale: Option<String>,
    },
    /// `history` 0 is the current clipboard; N is the Nth item before it.
    Clipboard {
        trim: bool,
        history: u32,
    },
    /// The text selected in the target app (empty for a typed trigger).
    Selection {
        trim: bool,
    },
    /// A random UUID v4.
    Uuid {
        upper: bool,
    },
    /// One of `options`, picked at random.
    Random {
        options: Vec<String>,
    },
    /// Presses `key` `count` times at this point of the insertion.
    Key {
        key: Key,
        count: u32,
    },
    /// Pauses the insertion for `ms` milliseconds.
    Delay {
        ms: u32,
    },
    Cursor,
    Snippet {
        id: Uuid,
    },
    Field {
        name: String,
        default: Option<String>,
        multiline: bool,
    },
    Choice {
        name: String,
        options: Vec<String>,
        default: Option<String>,
    },
    Unknown,
}

/// Static facts about one template source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateInfo {
    pub form_fields: Vec<FormFieldSpec>,
    pub referenced_snippet_ids: Vec<Uuid>,
    pub has_cursor: bool,
    pub uses_clipboard: bool,
    /// Has a `{{selection}}` tag.
    pub uses_selection: bool,
    /// Has a `{{key}}` tag, so the expansion is a sequence rather than one piece of text.
    pub has_key_presses: bool,
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
    /// Optional rich-source resolver. A plain root always renders nested snippets as plain text.
    pub resolve_formatting: Option<&'a dyn Fn(Uuid) -> Option<crate::formatting::Formatting>>,
    /// The snippet being rendered; referencing it is a cycle even at the top level.
    pub root_snippet_id: Option<Uuid>,
    /// Text selected in the target app, captured before insertion. `None` renders empty.
    pub selection: Option<String>,
    /// Plain text of the Nth clipboard-history item before the current clipboard (1 = previous).
    /// Without a resolver, history items render empty.
    pub clipboard_history: Option<&'a dyn Fn(usize) -> Option<String>>,
    /// Source of `{{uuid}}` values. Without one the nil UUID is used, so tests stay deterministic.
    pub new_uuid: Option<&'a dyn Fn() -> Uuid>,
    /// Picks an index below the given option count for `{{random}}` (the result is taken modulo
    /// the count). Without one the first option is used.
    pub pick: Option<&'a dyn Fn(usize) -> usize>,
}

/// Something other than text that happens during insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(tag = "kind", rename_all = "camelCase"))]
pub enum ActionKind {
    /// Press `key` `count` times.
    Key { key: Key, count: u32 },
    /// Wait `ms` milliseconds.
    Pause { ms: u32 },
}

/// An [`ActionKind`] and where it happens: after the first `offset` UTF-16 units of the text.
/// Actions are in insertion order; several can share an offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
pub struct RenderAction {
    pub offset: usize,
    #[cfg_attr(test, serde(flatten))]
    pub action: ActionKind,
}

/// Rendered text, caret placement and diagnostics.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(test, derive(Serialize, Deserialize))]
#[cfg_attr(test, serde(rename_all = "camelCase"))]
pub struct RenderResult {
    pub text: String,
    #[cfg_attr(test, serde(default, skip_serializing_if = "Option::is_none"))]
    pub formatting: Option<Vec<crate::formatting::FormatSpan>>,
    /// Caret offset in UTF-16 code units from the start of `text`, if a root `{{cursor}}` exists.
    pub cursor_offset: Option<usize>,
    pub diagnostics: Vec<Diagnostic>,
    /// Key presses and pauses from `{{key}}` and `{{delay}}`, in order. `text` never contains them:
    /// a consumer that only wants text (copy, previews, agents) can ignore this.
    #[cfg_attr(test, serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub actions: Vec<RenderAction>,
}

impl RenderResult {
    /// Whether inserting this needs key presses, so it can't be delivered as one piece of text.
    pub fn has_key_presses(&self) -> bool {
        self.actions
            .iter()
            .any(|a| matches!(a.action, ActionKind::Key { .. }))
    }
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
    use parser::{parse_uuid, tag_bool, tag_integer, tag_string};
    if tag.verbatim.is_some() {
        return TemplateTag::Unknown;
    }
    let string = |key: &str| tag_string(tag, key).map(str::to_string);
    match tag.name.as_str() {
        "date" | "time" => {
            let is_date = tag.name == "date";
            let format = string("format")
                .unwrap_or_else(|| if is_date { "yyyy-MM-dd" } else { "HH:mm" }.to_string());
            let offset = tag_string(tag, "offset").and_then(DateOffset::parse);
            let locale = string("locale");
            if is_date {
                TemplateTag::Date {
                    format,
                    offset,
                    locale,
                }
            } else {
                TemplateTag::Time {
                    format,
                    offset,
                    locale,
                }
            }
        }
        "clipboard" => TemplateTag::Clipboard {
            trim: tag_bool(tag, "trim"),
            history: tag_integer(tag, "history", 0, MAX_CLIPBOARD_HISTORY, 0).0,
        },
        "selection" => TemplateTag::Selection {
            trim: tag_bool(tag, "trim"),
        },
        "uuid" => TemplateTag::Uuid {
            upper: tag_bool(tag, "upper"),
        },
        "random" => TemplateTag::Random {
            options: split_options(tag_string(tag, "options").unwrap_or("")),
        },
        "key" => match tag_string(tag, "name").and_then(Key::parse) {
            Some(key) => TemplateTag::Key {
                key,
                count: tag_integer(tag, "count", 1, MAX_KEY_COUNT, 1).0,
            },
            None => TemplateTag::Unknown,
        },
        "delay" => match tag.attributes.iter().any(|a| a.name == "ms") {
            true => TemplateTag::Delay {
                ms: tag_integer(tag, "ms", 1, MAX_DELAY_MS, 1).0,
            },
            false => TemplateTag::Unknown,
        },
        "cursor" => TemplateTag::Cursor,
        "snippet" => match tag_string(tag, "id").and_then(parse_uuid) {
            Some(id) => TemplateTag::Snippet { id },
            None => TemplateTag::Unknown,
        },
        "field" => match string("name") {
            Some(name) => TemplateTag::Field {
                name,
                default: string("default"),
                multiline: tag_bool(tag, "multiline"),
            },
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
    render::render(source, None, context)
}

/// Renders formatting only when it belongs to this exact template source.
pub fn render_formatted(
    source: &str,
    formatting: Option<&crate::formatting::Formatting>,
    context: &RenderContext<'_>,
) -> RenderResult {
    render::render(source, formatting.filter(|f| f.source == source), context)
}

/// Analyzes recognized tags of one source; the first definition of each form name wins.
pub fn analyze(parsed: &ParsedTemplate) -> TemplateInfo {
    analysis::analyze(parsed)
}

/// Form fields of `source` including nested snippets (depth-first, cycle-safe, depth ≤ 8).
pub fn form_fields(
    source: &str,
    resolve_snippet: &dyn Fn(Uuid) -> Option<String>,
) -> Vec<FormFieldSpec> {
    analysis::form_fields(source, resolve_snippet)
}

/// A fresh UUID v4 for [`RenderContext::new_uuid`] outside tests.
pub fn random_uuid() -> Uuid {
    Uuid::new_v4()
}

/// A random index below `count` for [`RenderContext::pick`] outside tests (0 when `count` is 0).
pub fn random_index(count: usize) -> usize {
    match count {
        0 => 0,
        // A v4 UUID's low 62 bits are random, which is plenty for a list of options.
        n => (Uuid::new_v4().as_u128() % n as u128) as usize,
    }
}

/// Formats `date` with an ICU/`DateFormatter` pattern (`yyyy-MM-dd`, `EEEE, MMMM d`, `h:mm a`…)
/// in `locale`. Unsupported letters are emitted literally.
pub fn format_icu(date: &DateTime<FixedOffset>, pattern: &str, locale: &str) -> String {
    icu::format(date, pattern, locale)
}
