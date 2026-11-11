//! Shared helpers for the template tests.
#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, TimeZone};
use kudzu_core::template::{
    self, Attribute, AttributeValue, Node, RenderContext, RenderResult, Tag,
};
use uuid::Uuid;

/// 2024-01-31T00:00:00Z.
pub fn now() -> DateTime<FixedOffset> {
    FixedOffset::east_opt(0)
        .unwrap()
        .timestamp_opt(1_706_659_200, 0)
        .unwrap()
}

/// Renders with the Swift tests' default context.
pub struct Fixture {
    pub snippets: HashMap<Uuid, String>,
    pub clipboard: Option<String>,
    pub form_values: HashMap<String, String>,
    pub locale: String,
    pub root: Option<Uuid>,
    pub selection: Option<String>,
    /// Older clipboard items, most recent first (`history[0]` is `{{clipboard history=1}}`).
    pub history: Vec<String>,
    /// Seeds the UUID and random sources, so a fixture always renders the same thing.
    pub seed: u64,
}

impl Fixture {
    pub fn new() -> Self {
        Fixture {
            snippets: HashMap::new(),
            clipboard: Some("clip".into()),
            form_values: HashMap::new(),
            locale: "en_US_POSIX".into(),
            root: None,
            selection: None,
            history: Vec::new(),
            seed: 1,
        }
    }

    pub fn with_snippets(snippets: HashMap<Uuid, String>) -> Self {
        Fixture {
            snippets,
            ..Fixture::new()
        }
    }

    pub fn render(&self, source: &str) -> RenderResult {
        let resolve = |id: Uuid| self.snippets.get(&id).cloned();
        let history = |n: usize| self.history.get(n.checked_sub(1)?).cloned();
        let rng = std::cell::RefCell::new(SplitMix64(self.seed));
        let new_uuid = || {
            let mut rng = rng.borrow_mut();
            Uuid::from_u64_pair(rng.next(), rng.next())
        };
        let pick = |n: usize| rng.borrow_mut().below(n);
        let context = RenderContext {
            resolve_formatting: None,
            now: now(),
            locale: self.locale.clone(),
            clipboard: self.clipboard.clone(),
            form_values: self.form_values.clone(),
            resolve_snippet: &resolve,
            root_snippet_id: self.root,
            selection: self.selection.clone(),
            clipboard_history: Some(&history),
            new_uuid: Some(&new_uuid),
            pick: Some(&pick),
        };
        template::render(source, &context)
    }
}

/// Renders `source` with the default fixture and returns the text.
pub fn render(source: &str) -> String {
    Fixture::new().render(source).text
}

/// Parse then serialize.
pub fn canonical(source: &str) -> String {
    template::serialize(&template::parse(source).nodes)
}

/// Nodes with ranges cleared, adjacent text merged and empty text dropped.
pub fn semantic(nodes: &[Node]) -> Vec<Node> {
    let mut result: Vec<Node> = Vec::new();
    for node in nodes {
        match node {
            Node::Text { text, .. } => {
                if text.is_empty() {
                    continue;
                }
                if let Some(Node::Text { text: previous, .. }) = result.last_mut() {
                    previous.push_str(text);
                } else {
                    result.push(Node::Text {
                        text: text.clone(),
                        range: 0..0,
                    });
                }
            }
            Node::Tag(tag) => {
                let mut tag = tag.clone();
                tag.range = 0..0;
                result.push(Node::Tag(tag));
            }
        }
    }
    result
}

/// A text node without a source range.
pub fn text(value: &str) -> Node {
    Node::Text {
        text: value.into(),
        range: 0..0,
    }
}

/// A recognized tag node without a source range.
pub fn tag(name: &str, attributes: Vec<template::Attribute>) -> Node {
    Node::Tag(template::Tag {
        name: name.into(),
        attributes,
        range: 0..0,
        verbatim: None,
    })
}

/// A string attribute.
pub fn string_attr(name: &str, value: &str) -> template::Attribute {
    template::Attribute {
        name: name.into(),
        value: template::AttributeValue::String(value.into()),
    }
}

/// Deterministic generator for property tests.
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish value in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    pub fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

/// A short literal for grammar property tests.
pub fn random_text(rng: &mut SplitMix64) -> String {
    const ALPHABET: &[&str] = &[
        "a", "{", "}", "\\", "\"", " ", "😀", "\n", "|", "{{", "}}", "\\{", "\\\\",
    ];
    (0..rng.below(7)).map(|_| *rng.pick(ALPHABET)).collect()
}

pub fn random_tag(rng: &mut SplitMix64) -> Tag {
    let make = |name: &str, attributes: Vec<Attribute>| Tag {
        name: name.into(),
        attributes,
        range: 0..0,
        verbatim: None,
    };
    let string = |rng: &mut SplitMix64| AttributeValue::String(random_text(rng));
    match rng.below(14) {
        8 => make(
            "key",
            vec![
                Attribute {
                    name: "name".into(),
                    value: AttributeValue::String(rng.pick(&template::Key::ALL).name().into()),
                },
                Attribute {
                    name: "count".into(),
                    value: AttributeValue::Number(rng.below(60) as f64),
                },
            ],
        ),
        9 => make(
            "delay",
            vec![Attribute {
                name: "ms".into(),
                value: AttributeValue::Number(rng.below(7000) as f64),
            }],
        ),
        10 => make(
            "selection",
            vec![Attribute {
                name: "trim".into(),
                value: AttributeValue::Bool(rng.bool()),
            }],
        ),
        11 => make(
            "uuid",
            vec![Attribute {
                name: "upper".into(),
                value: AttributeValue::Bool(rng.bool()),
            }],
        ),
        12 => make(
            "random",
            vec![Attribute {
                name: "options".into(),
                value: string(rng),
            }],
        ),
        13 => make(
            "clipboard",
            vec![Attribute {
                name: "history".into(),
                value: AttributeValue::Number(rng.below(120) as f64 - 10.0),
            }],
        ),
        0 => make(
            "date",
            vec![Attribute {
                name: "format".into(),
                value: string(rng),
            }],
        ),
        1 => make("cursor", vec![]),
        2 => make(
            "clipboard",
            vec![Attribute {
                name: "trim".into(),
                value: AttributeValue::Bool(rng.bool()),
            }],
        ),
        3 => make(
            "field",
            vec![
                Attribute {
                    name: "name".into(),
                    value: string(rng),
                },
                Attribute {
                    name: "default".into(),
                    value: string(rng),
                },
            ],
        ),
        4 => make(
            "choice",
            vec![
                Attribute {
                    name: "name".into(),
                    value: AttributeValue::String("c".into()),
                },
                Attribute {
                    name: "options".into(),
                    value: string(rng),
                },
            ],
        ),
        5 => make(
            "snippet",
            vec![Attribute {
                name: "id".into(),
                value: AttributeValue::String(
                    Uuid::new_v4().hyphenated().to_string().to_uppercase(),
                ),
            }],
        ),
        6 => make(
            "time",
            vec![Attribute {
                name: "extra".into(),
                value: AttributeValue::Number(rng.below(1999) as f64 - 999.0),
            }],
        ),
        _ => Tag {
            name: String::new(),
            attributes: vec![],
            range: 0..0,
            verbatim: Some(
                rng.pick(&["{{DATE}}", "{{ nope }}", "{{field}}", "{{x y=1}}"])
                    .to_string(),
            ),
        },
    }
}
