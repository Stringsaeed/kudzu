//! Shared helpers for the template tests.
#![allow(dead_code)]

use std::collections::HashMap;

use chrono::{DateTime, FixedOffset, TimeZone};
use kudzu_core::template::{self, Node, RenderContext, RenderResult};
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
}

impl Fixture {
    pub fn new() -> Self {
        Fixture {
            snippets: HashMap::new(),
            clipboard: Some("clip".into()),
            form_values: HashMap::new(),
            locale: "en_US_POSIX".into(),
            root: None,
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
        let context = RenderContext {
            now: now(),
            locale: self.locale.clone(),
            clipboard: self.clipboard.clone(),
            form_values: self.form_values.clone(),
            resolve_snippet: &resolve,
            root_snippet_id: self.root,
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
