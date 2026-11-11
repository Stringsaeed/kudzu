//! Builds Kudzu template tag source for the importer.
//!
//! Tags are assembled as [`Tag`] values and written by the template serializer, so quoting and
//! escaping always match what the parser reads back.

use uuid::Uuid;

use crate::template::{serialize, Attribute, AttributeValue, Node, Tag};

fn source(name: &str, attributes: Vec<Attribute>) -> String {
    serialize(&[Node::Tag(Tag {
        name: name.to_string(),
        attributes,
        range: 0..0,
        verbatim: None,
    })])
}

fn string_attr(name: &str, value: &str) -> Attribute {
    Attribute {
        name: name.to_string(),
        value: AttributeValue::String(value.to_string()),
    }
}

fn bool_attr(name: &str, value: bool) -> Attribute {
    Attribute {
        name: name.to_string(),
        value: AttributeValue::Bool(value),
    }
}

/// `{{date format=… offset=… locale=…}}`; missing and empty values are omitted.
pub(crate) fn date_tag(format: Option<&str>, offset: Option<&str>, locale: Option<&str>) -> String {
    let attrs = [("format", format), ("offset", offset), ("locale", locale)]
        .into_iter()
        .filter_map(|(k, v)| v.filter(|v| !v.is_empty()).map(|v| string_attr(k, v)))
        .collect();
    source("date", attrs)
}

/// `{{clipboard}}` or `{{clipboard trim=true}}`.
pub(crate) fn clipboard_tag(trim: bool) -> String {
    source(
        "clipboard",
        if trim {
            vec![bool_attr("trim", true)]
        } else {
            vec![]
        },
    )
}

fn number_attr(name: &str, value: u32) -> Attribute {
    Attribute {
        name: name.to_string(),
        value: AttributeValue::Number(f64::from(value)),
    }
}

/// `{{clipboard history=N}}`: the Nth copy before the current clipboard, optionally trimmed.
pub(crate) fn clipboard_history_tag(history: u32, trim: bool) -> String {
    let mut attrs = vec![number_attr("history", history)];
    if trim {
        attrs.push(bool_attr("trim", true));
    }
    source("clipboard", attrs)
}

/// `{{selection}}`.
pub(crate) fn selection_tag() -> String {
    source("selection", vec![])
}

/// `{{uuid}}` or `{{uuid upper=true}}`.
pub(crate) fn uuid_tag(upper: bool) -> String {
    source(
        "uuid",
        if upper {
            vec![bool_attr("upper", true)]
        } else {
            vec![]
        },
    )
}

/// `{{random options="a|b"}}`; a `|` inside an option is written as `\|`.
pub(crate) fn random_tag(options: &[String]) -> String {
    let encoded = options
        .iter()
        .map(|o| o.replace('|', "\\|"))
        .collect::<Vec<_>>()
        .join("|");
    source("random", vec![string_attr("options", &encoded)])
}

/// `{{key name="tab"}}`, with `count` when it isn't 1.
pub(crate) fn key_tag(key: crate::template::Key, count: u32) -> String {
    let mut attrs = vec![string_attr("name", key.name())];
    if count != 1 {
        attrs.push(number_attr("count", count));
    }
    source("key", attrs)
}

/// `{{cursor}}`.
pub(crate) fn cursor_tag() -> String {
    source("cursor", vec![])
}

/// `{{snippet id="…"}}` with the uppercase UUID the Swift app writes.
pub(crate) fn snippet_tag(id: Uuid) -> String {
    source(
        "snippet",
        vec![string_attr(
            "id",
            &id.hyphenated().to_string().to_uppercase(),
        )],
    )
}

/// `{{field name=… default=… multiline=true}}`.
pub(crate) fn field_tag(name: &str, default: Option<&str>, multiline: bool) -> String {
    let mut attrs = vec![string_attr("name", name)];
    if let Some(d) = default.filter(|d| !d.is_empty()) {
        attrs.push(string_attr("default", d));
    }
    if multiline {
        attrs.push(bool_attr("multiline", true));
    }
    source("field", attrs)
}

/// `{{choice name=… options="a|b" default=…}}`; a `|` inside an option is written as `\|`.
pub(crate) fn choice_tag(name: &str, options: &[String], default: Option<&str>) -> String {
    let encoded = options
        .iter()
        .map(|o| o.replace('|', "\\|"))
        .collect::<Vec<_>>()
        .join("|");
    let mut attrs = vec![string_attr("name", name), string_attr("options", &encoded)];
    if let Some(d) = default.filter(|d| !d.is_empty()) {
        attrs.push(string_attr("default", d));
    }
    source("choice", attrs)
}
