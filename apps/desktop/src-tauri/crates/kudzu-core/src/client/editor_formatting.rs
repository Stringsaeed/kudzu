//! The template editor's formatting commands (`lib/editor-formatting.ts`) without CodeMirror:
//! mapping spans through edits, widening a selection over chips and images, toggling a style,
//! and toggling list prefixes. Offsets are UTF-16 code units of the editor document, whose
//! line breaks are single `\n`s.

use std::mem::discriminant;

use crate::formatting::{FormatSpan, FormatStyle};

use super::formatting::{same_style, valid_spans};
use super::js;
use super::template::{tokenize, Token};

/// One replacement in document coordinates: `from..to` becomes `insert_len` units of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    pub from: usize,
    pub to: usize,
    pub insert_len: usize,
}

/// The shape of a set of changes, as CodeMirror's `ChangeDesc` stores it: `(len, ins)`
/// sections where `ins == None` keeps `len` units and `Some(n)` replaces them with `n`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangeDesc {
    sections: Vec<(usize, Option<usize>)>,
}

impl ChangeDesc {
    /// Builds the description of `changes` (sorted, non-overlapping, as an editor produces)
    /// over a document of `doc_len` units. `None` when they are out of order or out of range.
    pub fn of(changes: &[Change], doc_len: usize) -> Option<Self> {
        let mut desc = ChangeDesc::default();
        let mut pos = 0;
        for c in changes {
            if c.from > c.to || c.to > doc_len || c.from < pos {
                return None;
            }
            if c.from == c.to && c.insert_len == 0 {
                continue;
            }
            if c.from > pos {
                desc.add(c.from - pos, None);
            }
            desc.add(c.to - c.from, Some(c.insert_len));
            pos = c.to;
        }
        if pos < doc_len {
            desc.add(doc_len - pos, None);
        }
        Some(desc)
    }

    // CodeMirror's `addSection`.
    fn add(&mut self, len: usize, ins: Option<usize>) {
        let weight = |ins: Option<usize>| ins.map_or(-1, |n| n as i64);
        if len == 0 && weight(ins) <= 0 {
            return;
        }
        if let Some(last) = self.sections.last_mut() {
            if weight(ins) <= 0 && weight(ins) == weight(last.1) {
                last.0 += len;
                return;
            }
            if len == 0 && last.0 == 0 {
                last.1 = Some(last.1.unwrap_or(0) + ins.unwrap_or(0));
                return;
            }
        }
        self.sections.push((len, ins));
    }

    /// Length of the document before the changes.
    pub fn len(&self) -> usize {
        self.sections.iter().map(|s| s.0).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(|s| s.1.is_none())
    }

    /// CodeMirror's `mapPos` (simple mode): where `pos` ends up. `assoc < 0` keeps a position
    /// before text inserted at it, `assoc > 0` moves it after; positions inside a replaced
    /// range go to its start (`assoc < 0`) or end.
    pub fn map_pos(&self, pos: usize, assoc: i32) -> usize {
        let (mut pos_a, mut pos_b) = (0, 0);
        for &(len, ins) in &self.sections {
            let end_a = pos_a + len;
            match ins {
                None => {
                    if end_a > pos {
                        return pos_b + (pos - pos_a);
                    }
                    pos_b += len;
                }
                Some(ins) => {
                    if end_a > pos || (end_a == pos && assoc < 0 && len == 0) {
                        return if pos == pos_a || assoc < 0 { pos_b } else { pos_b + ins };
                    }
                    pos_b += ins;
                }
            }
            pos_a = end_a;
        }
        pos_b
    }
}

/// Maps spans through an edit: image spans shrink inward (typing beside an image stays
/// outside it), other spans grow outward; spans that collapse are dropped.
pub fn map_spans(spans: Option<&[FormatSpan]>, changes: &ChangeDesc) -> Option<Vec<FormatSpan>> {
    spans.map(|spans| {
        spans
            .iter()
            .map(|s| {
                let image = matches!(s.style, FormatStyle::Image { .. });
                FormatSpan { start: changes.map_pos(s.start, if image { 1 } else { -1 }), end: changes.map_pos(s.end, if image { -1 } else { 1 }), style: s.style.clone() }
            })
            .filter(|s| s.start < s.end)
            .collect()
    })
}

/// The editor's formatting state after a transaction: spans mapped through `changes` (when
/// the document changed), replaced by any `set` effects (the last wins; `Some(None)` switches
/// to plain text), and validated against the new document whenever they changed.
pub fn update_formatting(value: Option<&[FormatSpan]>, changes: Option<&ChangeDesc>, set: &[Option<Vec<FormatSpan>>], new_doc: &str) -> Option<Vec<FormatSpan>> {
    let mut next = match changes {
        Some(c) => map_spans(value, c),
        None => value.map(<[FormatSpan]>::to_vec),
    };
    let mut changed = changes.is_some() && value.is_some();
    for effect in set {
        next = effect.clone();
        changed = true;
    }
    if !changed {
        return next;
    }
    next.map(|spans| valid_spans(new_doc, &spans))
}

/// Image spans with their alternative text, for widgets and atomic ranges.
pub fn image_spans(spans: Option<&[FormatSpan]>, text: &str) -> Vec<(FormatSpan, String)> {
    spans
        .unwrap_or_default()
        .iter()
        .filter(|s| matches!(s.style, FormatStyle::Image { .. }))
        .map(|s| (s.clone(), js::slice16(text, s.start, s.end)))
        .collect()
}

/// The selection widened to cover every chip and image it touches. An empty selection stays put.
pub fn selected_range(doc: &str, from: usize, to: usize, spans: Option<&[FormatSpan]>) -> (usize, usize) {
    let (mut from, mut to) = (from, to);
    if from == to {
        return (from, to);
    }
    for token in tokenize(doc) {
        if let Token::Tag(t) = token {
            if t.from < to && t.to > from {
                from = from.min(t.from);
                to = to.max(t.to);
            }
        }
    }
    for span in spans.unwrap_or_default() {
        if matches!(span.style, FormatStyle::Image { .. }) && span.start < to && span.end > from {
            from = from.min(span.start);
            to = to.max(span.end);
        }
    }
    (from, to)
}

/// What a formatting command applies.
#[derive(Debug, Clone, PartialEq)]
pub enum FormatChange {
    Style(FormatStyle),
    /// Removes every style from the selection.
    Clear,
}

/// Toggles `change` over the selection: the new span list (already validated), or `None` when
/// nothing applies (an empty selection or plain-text mode). A style that already covers the
/// selection is removed; otherwise same-kind spans are cut out of it and the style added.
/// The caller still skips this while an input method is composing.
pub fn change_format(doc: &str, selection: (usize, usize), spans: Option<&[FormatSpan]>, change: &FormatChange) -> Option<Vec<FormatSpan>> {
    let (from, to) = selected_range(doc, selection.0, selection.1, spans);
    let spans = spans?;
    if from == to {
        return None;
    }
    let remove = match change {
        FormatChange::Clear => true,
        FormatChange::Style(style) => spans.iter().any(|s| s.start <= from && s.end >= to && same_style(&s.style, style)),
    };
    let mut next: Vec<FormatSpan> = spans
        .iter()
        .flat_map(|s| {
            let other_kind = matches!(change, FormatChange::Style(style) if discriminant(&s.style) != discriminant(style));
            if s.end <= from || s.start >= to || other_kind {
                return vec![s.clone()];
            }
            let mut pieces = Vec::new();
            if s.start < from {
                pieces.push(FormatSpan { end: from, ..s.clone() });
            }
            if s.end > to {
                pieces.push(FormatSpan { start: to, ..s.clone() });
            }
            pieces
        })
        .collect();
    if let (false, FormatChange::Style(style)) = (remove, change) {
        next.push(FormatSpan { start: from, end: to, style: style.clone() });
    }
    Some(valid_spans(doc, &next))
}

/// A text edit: replace `from..to` (UTF-16) with `insert`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub from: usize,
    pub to: usize,
    pub insert: String,
}

fn ordered_prefix(line: &[u16]) -> Option<usize> {
    let digits = line.iter().take_while(|&&u| (u16::from(b'0')..=u16::from(b'9')).contains(&u)).count();
    (digits > 0 && line.get(digits) == Some(&u16::from(b'.')) && line.get(digits + 1) == Some(&u16::from(b' '))).then_some(digits + 2)
}

fn bullet_prefix(line: &[u16]) -> Option<usize> {
    line.starts_with(&[0x2022, u16::from(b' ')]).then_some(2)
}

/// Adds or removes list prefixes on every line the selection touches. Prefixes are real text
/// (`• ` or `1. `), so lists keep their numbering in plain-text apps and exports. When every
/// line already has this kind of prefix they are all removed; otherwise each line's existing
/// prefix (of either kind) is replaced.
pub fn toggle_list(doc: &str, from: usize, to: usize, ordered: bool) -> Vec<TextEdit> {
    let units = js::utf16(doc);
    let mut lines: Vec<(usize, &[u16])> = Vec::new();
    let mut start = 0;
    for (i, &u) in units.iter().enumerate() {
        if u == u16::from(b'\n') {
            lines.push((start, &units[start..i]));
            start = i + 1;
        }
    }
    lines.push((start, &units[start..]));
    let line_at = |pos: usize| lines.iter().rposition(|(s, _)| *s <= pos).unwrap_or(0);
    let first = line_at(from);
    let last = line_at(if to > from { to - 1 } else { to });
    let selected = &lines[first..=last.max(first)];
    let remove = selected.iter().all(|(_, text)| if ordered { ordered_prefix(text).is_some() } else { bullet_prefix(text).is_some() });
    selected
        .iter()
        .enumerate()
        .map(|(i, (line_from, text))| TextEdit {
            from: *line_from,
            to: line_from + bullet_prefix(text).or_else(|| ordered_prefix(text)).unwrap_or(0),
            insert: if remove {
                String::new()
            } else if ordered {
                format!("{}. ", i + 1)
            } else {
                "• ".into()
            },
        })
        .collect()
}

/// Applies non-overlapping, sorted edits to `doc`.
pub fn apply_edits(doc: &str, edits: &[TextEdit]) -> String {
    let units = js::utf16(doc);
    let mut out = Vec::with_capacity(units.len());
    let mut at = 0;
    for e in edits {
        out.extend_from_slice(&units[at..e.from]);
        out.extend(e.insert.encode_utf16());
        at = e.to;
    }
    out.extend_from_slice(&units[at..]);
    js::from16(&out)
}

/// The [`ChangeDesc`] of `edits` over `doc`.
pub fn edits_desc(doc: &str, edits: &[TextEdit]) -> Option<ChangeDesc> {
    let changes: Vec<Change> = edits.iter().map(|e| Change { from: e.from, to: e.to, insert_len: js::len16(&e.insert) }).collect();
    ChangeDesc::of(&changes, js::len16(doc))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg==";

    fn bold(start: usize, end: usize) -> FormatSpan {
        FormatSpan { start, end, style: FormatStyle::Bold }
    }

    /// A tiny editor: document plus formatting, edited like the CodeMirror field.
    struct Editor {
        doc: String,
        spans: Option<Vec<FormatSpan>>,
    }

    impl Editor {
        fn edit(&mut self, edits: &[TextEdit]) {
            let desc = edits_desc(&self.doc, edits).unwrap();
            self.doc = apply_edits(&self.doc, edits);
            self.spans = update_formatting(self.spans.as_deref(), Some(&desc), &[], &self.doc);
        }
        fn set(&mut self, spans: Option<Vec<FormatSpan>>) {
            self.spans = update_formatting(self.spans.as_deref(), None, &[spans], &self.doc);
        }
    }

    #[test]
    fn format_then_edit_keeps_formatting_with_the_text() {
        let mut e = Editor { doc: "Hello world".into(), spans: Some(vec![]) };
        let next = change_format(&e.doc, (0, 5), e.spans.as_deref(), &FormatChange::Style(FormatStyle::Bold));
        e.set(next);
        assert_eq!(e.spans, Some(vec![bold(0, 5)]));
        e.edit(&[TextEdit { from: 0, to: 5, insert: "Hi".into() }]);
        assert_eq!(e.doc, "Hi world");
        assert!(super::super::formatting::formatted_html(&e.doc, e.spans.as_deref().unwrap()).contains("<strong>Hi</strong> world"));
        // Toggling again over the same range removes it.
        let next = change_format(&e.doc, (0, 2), e.spans.as_deref(), &FormatChange::Style(FormatStyle::Bold));
        assert_eq!(next, Some(vec![]));
        // Plain-text mode and empty selections do nothing.
        assert_eq!(change_format("Hi", (0, 2), None, &FormatChange::Clear), None);
        assert_eq!(change_format("Hi", (1, 1), Some(&[]), &FormatChange::Clear), None);
    }

    #[test]
    fn changing_format_cuts_same_kind_spans_and_keeps_others() {
        let link = |url: &str| FormatStyle::Link { url: url.into() };
        let spans = vec![bold(0, 10), FormatSpan { start: 0, end: 10, style: link("https://a") }, FormatSpan { start: 0, end: 10, style: FormatStyle::Italic }];
        let next = change_format("0123456789", (3, 6), Some(&spans), &FormatChange::Style(link("https://b"))).unwrap();
        assert_eq!(next, vec![
            bold(0, 10),
            FormatSpan { start: 0, end: 3, style: link("https://a") },
            FormatSpan { start: 6, end: 10, style: link("https://a") },
            FormatSpan { start: 0, end: 10, style: FormatStyle::Italic },
            FormatSpan { start: 3, end: 6, style: link("https://b") },
        ]);
        let cleared = change_format("0123456789", (3, 6), Some(&spans), &FormatChange::Clear).unwrap();
        assert_eq!(cleared.len(), 6);
        assert!(cleared.iter().all(|s| s.end <= 3 || s.start >= 6));
    }

    #[test]
    fn selection_widens_over_chips_and_images() {
        let doc = "ab {{cursor}} cd";
        assert_eq!(selected_range(doc, 1, 5, None), (1, 13));
        assert_eq!(selected_range(doc, 5, 5, None), (5, 5));
        let image = FormatSpan { start: 14, end: 16, style: FormatStyle::Image { data_url: PNG.into(), width: 10 } };
        assert_eq!(selected_range(doc, 15, 16, Some(std::slice::from_ref(&image))), (14, 16));
        assert_eq!(image_spans(Some(&[image.clone(), bold(0, 1)]), doc), vec![(image, "cd".to_string())]);
    }

    #[test]
    fn typing_beside_an_image_leaves_it_intact_and_deleting_drops_it() {
        let image = FormatSpan { start: 0, end: 6, style: FormatStyle::Image { data_url: PNG.into(), width: 120 } };
        let mut e = Editor { doc: "[Logo]".into(), spans: Some(vec![]) };
        e.set(Some(vec![image.clone()]));
        e.edit(&[TextEdit { from: 6, to: 6, insert: "!".into() }]);
        assert_eq!(e.spans, Some(vec![image.clone()]));
        e.edit(&[TextEdit { from: 0, to: 0, insert: "x".into() }]);
        assert_eq!(e.spans.as_ref().unwrap()[0].start, 1);
        e.edit(&[TextEdit { from: 1, to: 7, insert: String::new() }]);
        assert_eq!(e.doc, "x!");
        assert_eq!(e.spans, Some(vec![]));
        e.set(None);
        assert_eq!(e.spans, None);
    }

    #[test]
    fn non_image_spans_grow_with_typing_at_their_edges() {
        let mut e = Editor { doc: "ab".into(), spans: Some(vec![bold(0, 2)]) };
        e.edit(&[TextEdit { from: 2, to: 2, insert: "c".into() }, ]);
        e.edit(&[TextEdit { from: 0, to: 0, insert: "z".into() }]);
        assert_eq!(e.spans, Some(vec![bold(0, 4)]));
    }

    #[test]
    fn maps_positions_like_codemirror() {
        let d = ChangeDesc::of(&[Change { from: 2, to: 4, insert_len: 3 }], 6).unwrap();
        assert_eq!(d.len(), 6);
        assert_eq!([d.map_pos(1, -1), d.map_pos(2, -1), d.map_pos(2, 1), d.map_pos(3, -1), d.map_pos(3, 1), d.map_pos(4, -1), d.map_pos(4, 1), d.map_pos(6, 1)], [1, 2, 2, 2, 5, 5, 5, 7]);
        let ins = ChangeDesc::of(&[Change { from: 2, to: 2, insert_len: 2 }], 4).unwrap();
        assert_eq!([ins.map_pos(2, -1), ins.map_pos(2, 1)], [2, 4]);
        assert!(ChangeDesc::of(&[Change { from: 3, to: 4, insert_len: 0 }, Change { from: 1, to: 2, insert_len: 0 }], 5).is_none());
        assert!(ChangeDesc::of(&[], 5).unwrap().is_empty());
    }

    #[test]
    fn lists_toggle_and_keep_plain_text() {
        let doc = "One\nTwo";
        let edits = toggle_list(doc, 0, 7, true);
        let doc = apply_edits(doc, &edits);
        assert_eq!(doc, "1. One\n2. Two");
        let doc = apply_edits(&doc, &toggle_list(&doc, 0, 13, true));
        assert_eq!(doc, "One\nTwo");
        // Bullets replace numbers; a selection ending at a line start excludes that line.
        let doc = "1. a\nb\nc";
        assert_eq!(apply_edits(doc, &toggle_list(doc, 0, 7, false)), "• a\n• b\nc");
        let doc = "• a\n• b";
        assert_eq!(apply_edits(doc, &toggle_list(doc, 1, 1, false)), "a\n• b");
        assert_eq!(apply_edits("", &toggle_list("", 0, 0, false)), "• ");
    }
}
