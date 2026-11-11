//! The target app's focused text field: what the fake desktop applies keys and pastes to.
//!
//! Editing works on grapheme clusters, as text fields do. `show` renders the text with `|` at
//! the caret, which is how tests state the document they expect.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Default)]
pub(super) struct Document {
    text: String,
    /// Byte offset, always on a grapheme boundary.
    caret: usize,
    /// What ⌘V inserts instead of the plain-text flavour (an app that pastes rich text).
    pub paste_inserts: Option<String>,
    /// Whether the app reports its caret through Accessibility.
    pub reports_caret: bool,
}

impl Document {
    pub fn new() -> Self {
        Self {
            reports_caret: true,
            ..Self::default()
        }
    }

    /// Replaces the contents with `shown` (text with one `|` for the caret, else caret at end).
    pub fn set(&mut self, shown: &str) {
        match shown.split_once('|') {
            Some((before, after)) => {
                self.text = format!("{before}{after}");
                self.caret = before.len();
            }
            None => {
                self.text = shown.to_string();
                self.caret = shown.len();
            }
        }
    }

    pub fn show(&self) -> String {
        format!("{}|{}", &self.text[..self.caret], &self.text[self.caret..])
    }

    /// Caret position in UTF-16 units, as Accessibility reports it.
    pub fn caret_utf16(&self) -> usize {
        self.text[..self.caret].encode_utf16().count()
    }

    /// Moves the caret to grapheme `index` (a click in the text).
    pub fn place_caret(&mut self, index: usize) {
        self.caret = self
            .text
            .grapheme_indices(true)
            .map(|(at, _)| at)
            .nth(index)
            .unwrap_or(self.text.len());
    }

    pub fn insert(&mut self, text: &str) {
        self.text.insert_str(self.caret, text);
        self.caret += text.len();
    }

    pub fn backspace(&mut self) {
        if let Some((at, _)) = self.text[..self.caret].grapheme_indices(true).next_back() {
            self.text.replace_range(at..self.caret, "");
            self.caret = at;
        }
    }

    pub fn forward_delete(&mut self) {
        if let Some(g) = self.text[self.caret..].graphemes(true).next() {
            let end = self.caret + g.len();
            self.text.replace_range(self.caret..end, "");
        }
    }

    pub fn left(&mut self) {
        if let Some((at, _)) = self.text[..self.caret].grapheme_indices(true).next_back() {
            self.caret = at;
        }
    }

    pub fn right(&mut self) {
        if let Some(g) = self.text[self.caret..].graphemes(true).next() {
            self.caret += g.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_by_grapheme() {
        let mut doc = Document::new();
        doc.set("ab|c");
        doc.insert("e\u{301}");
        assert_eq!(doc.show(), "abe\u{301}|c");
        doc.backspace();
        assert_eq!(doc.show(), "ab|c");
        doc.left();
        doc.forward_delete();
        assert_eq!(doc.show(), "a|c");
        doc.right();
        doc.right();
        assert_eq!(doc.show(), "ac|");
        doc.place_caret(1);
        assert_eq!((doc.show().as_str(), doc.caret_utf16()), ("a|c", 1));
    }
}
