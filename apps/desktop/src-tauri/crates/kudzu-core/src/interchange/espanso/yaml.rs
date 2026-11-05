//! A minimal YAML document tree with line numbers, built from `yaml-rust2` parser events.
//!
//! Every scalar stays a string (Espanso values are interpreted per key), which mirrors how the
//! Swift importer reads Yams nodes. Aliases resolve to a copy of their anchored node.

use std::collections::HashMap;

use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

/// A YAML node and the 1-based line it starts on.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Node {
    pub kind: Kind,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kind {
    Scalar(String),
    Seq(Vec<Node>),
    Map(Vec<(Node, Node)>),
}

impl Node {
    /// The scalar text, if this is a scalar.
    pub fn as_str(&self) -> Option<&str> {
        match &self.kind {
            Kind::Scalar(s) => Some(s),
            _ => None,
        }
    }

    /// The items, if this is a sequence.
    pub fn as_seq(&self) -> Option<&[Node]> {
        match &self.kind {
            Kind::Seq(items) => Some(items),
            _ => None,
        }
    }

    /// The entries, if this is a mapping.
    pub fn as_map(&self) -> Option<&[(Node, Node)]> {
        match &self.kind {
            Kind::Map(entries) => Some(entries),
            _ => None,
        }
    }

    /// The value for a scalar `key` in a mapping.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.as_map()?
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    /// The value for `key` as a string.
    pub fn str_at(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// The value for `key` as a YAML 1.1 boolean (`true`/`yes`/`false`/`no`, any case).
    pub fn bool_at(&self, key: &str) -> Option<bool> {
        match self.str_at(key)?.to_lowercase().as_str() {
            "true" | "yes" => Some(true),
            "false" | "no" => Some(false),
            _ => None,
        }
    }
}

enum Frame {
    Seq {
        items: Vec<Node>,
        anchor: usize,
        line: usize,
    },
    Map {
        entries: Vec<(Node, Node)>,
        key: Option<Node>,
        anchor: usize,
        line: usize,
    },
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    anchors: HashMap<usize, Node>,
    root: Option<Node>,
    done: bool,
}

impl Builder {
    fn finish(&mut self, node: Node, anchor: usize) {
        if anchor > 0 {
            self.anchors.insert(anchor, node.clone());
        }
        match self.stack.last_mut() {
            Some(Frame::Seq { items, .. }) => items.push(node),
            Some(Frame::Map { entries, key, .. }) => match key.take() {
                Some(k) => entries.push((k, node)),
                None => *key = Some(node),
            },
            None => {
                if self.root.is_none() {
                    self.root = Some(node);
                }
                self.done = true;
            }
        }
    }
}

impl MarkedEventReceiver for Builder {
    fn on_event(&mut self, event: Event, mark: Marker) {
        if self.done {
            return; // Only the first document is read.
        }
        let line = mark.line();
        match event {
            Event::Scalar(value, style, anchor, _) => {
                // An empty value (`replace:`) arrives as a plain `~`; Yams reads it as "".
                let value = if style == TScalarStyle::Plain && value == "~" {
                    String::new()
                } else {
                    value
                };
                self.finish(
                    Node {
                        kind: Kind::Scalar(value),
                        line,
                    },
                    anchor,
                );
            }
            Event::Alias(id) => {
                let node = self.anchors.get(&id).cloned().unwrap_or(Node {
                    kind: Kind::Scalar(String::new()),
                    line,
                });
                self.finish(node, 0);
            }
            Event::SequenceStart(anchor, _) => self.stack.push(Frame::Seq {
                items: vec![],
                anchor,
                line,
            }),
            Event::MappingStart(anchor, _) => {
                self.stack.push(Frame::Map {
                    entries: vec![],
                    key: None,
                    anchor,
                    line,
                });
            }
            Event::SequenceEnd | Event::MappingEnd => match self.stack.pop() {
                Some(Frame::Seq {
                    items,
                    anchor,
                    line,
                }) => self.finish(
                    Node {
                        kind: Kind::Seq(items),
                        line,
                    },
                    anchor,
                ),
                Some(Frame::Map {
                    entries,
                    anchor,
                    line,
                    ..
                }) => {
                    self.finish(
                        Node {
                            kind: Kind::Map(entries),
                            line,
                        },
                        anchor,
                    );
                }
                None => {}
            },
            _ => {}
        }
    }
}

/// Parses the first document of `text`; `Ok(None)` for an empty file.
pub(crate) fn parse(text: &str) -> Result<Option<Node>, String> {
    let mut builder = Builder::default();
    Parser::new_from_str(text)
        .load(&mut builder, false)
        .map_err(|e| e.to_string())?;
    Ok(builder.root)
}
