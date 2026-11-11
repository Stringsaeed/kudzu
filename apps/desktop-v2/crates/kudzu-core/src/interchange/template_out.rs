//! Accumulates Kudzu template source from literal text and ready-made tags.

use crate::template::{serialize, Node};

/// Builds template source. Literal runs are buffered and escaped together, so braces and
/// backslashes in the text can never combine into something the parser reads as a tag.
#[derive(Default)]
pub(crate) struct TemplateOut {
    out: String,
    text: String,
}

impl TemplateOut {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn text(&mut self, text: &str) {
        self.text.push_str(text);
    }

    pub(crate) fn push_char(&mut self, ch: char) {
        self.text.push(ch);
    }

    /// Appends tag source already written by the serializer (see `espanso::tags`).
    pub(crate) fn tag(&mut self, tag_source: &str) {
        self.flush();
        self.out.push_str(tag_source);
    }

    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let node = Node::Text {
            text: std::mem::take(&mut self.text),
            range: 0..0,
        };
        self.out.push_str(&serialize(&[node]));
    }

    pub(crate) fn finish(mut self) -> String {
        self.flush();
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_braces_and_backslashes_in_literal_runs() {
        let mut out = TemplateOut::new();
        out.text("a {");
        out.text("{b} \\");
        out.tag("{{cursor}}");
        out.text("end");
        assert_eq!(out.finish(), "a \\{{b} \\\\{{cursor}}end");
    }
}
