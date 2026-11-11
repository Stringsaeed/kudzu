//! Edits on an [`EditorDoc`](super::EditorDoc). One public method is one undo step.
//! Display offsets are UTF-8 bytes. See the parent module for the snap and repair rules.

use std::ops::Range;

use super::literal::{repair, slice_display};
use super::load::literal_from_display;
use super::mapping::{DisplayEdit, SpanMap};
use super::{EditorDoc, Raw};
use crate::template::lists::list_edits;
use crate::template::{chips, Node, Tag};

impl EditorDoc {
    /// Replace the display range with `text` (a literal; braces are escaped). One undo step.
    /// Returns the applied range in the new display (a caret range for a deletion).
    pub fn replace(&mut self, start: usize, end: usize, text: &str) -> Range<usize> {
        let mut applied = 0..0;
        self.transaction(|doc| {
            let inserted = if text.is_empty() {
                Vec::new()
            } else {
                vec![literal_from_display(text)]
            };
            applied = doc.splice(start, end, inserted).new;
        });
        applied
    }

    /// Toggle `• ` or `1. ` on the selected lines. Markers are real text. One undo step.
    pub fn toggle_list(&mut self, from: usize, to: usize, ordered: bool) -> bool {
        self.transaction(|doc| {
            let edits = list_edits(doc.text(), from, to, ordered);
            for edit in edits.into_iter().rev() {
                if doc
                    .pills()
                    .iter()
                    .any(|p| p.range.start < edit.start && edit.start < p.range.end)
                {
                    continue;
                }
                let inserted = if edit.insert.is_empty() {
                    Vec::new()
                } else {
                    vec![literal_from_display(&edit.insert)]
                };
                doc.splice(edit.start, edit.end, inserted);
            }
        })
    }

    /// The actual old/new display ranges, including atomic-chip snapping.
    pub(super) fn splice(&mut self, start: usize, end: usize, insert: Vec<Raw>) -> DisplayEdit {
        let (start, end) = snap_range(self, start, end);
        let edit = DisplayEdit {
            old: start..end,
            new: start..start + insert.iter().map(|r| r.display.len()).sum::<usize>(),
        };
        let spans = SpanMap::capture(self);
        let mut out = Vec::new();
        let mut cursor = 0usize;
        let mut placed = false;
        for raw in &self.raws {
            let d0 = cursor;
            let d1 = cursor + raw.display.len();
            cursor = d1;
            let outside = d1 <= start || d0 >= end;
            if outside && d0 >= end && !placed {
                out.extend(insert.iter().cloned());
                placed = true;
            }
            if outside {
                out.push(raw.clone());
                continue;
            }
            if raw.pill.is_some() {
                continue;
            }
            let left = start.saturating_sub(d0);
            let right = end.max(d0).saturating_sub(d0).min(raw.display.len());
            if left > 0 {
                out.push(slice_display(raw, 0, left));
            }
            if end <= d1 && !placed {
                out.extend(insert.iter().cloned());
                placed = true;
            }
            if right < raw.display.len() {
                out.push(slice_display(raw, right, raw.display.len()));
            }
        }
        if !placed {
            out.extend(insert);
        }
        self.raws = out;
        repair(&mut self.raws);
        self.install();
        spans.apply(self, &edit);
        edit
    }
}

pub(super) fn overlaps(range: &Range<usize>, from: usize, to: usize) -> bool {
    range.start < to && range.end > from
}

pub(super) fn snap_range(doc: &EditorDoc, mut start: usize, mut end: usize) -> (usize, usize) {
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    start = floor_char(doc.text(), start.min(doc.text().len()));
    end = ceil_char(doc.text(), end.min(doc.text().len()));
    if start == end {
        let at = snap_caret(doc, start);
        return (at, at);
    }
    for pill in doc.pills() {
        if start < pill.range.end && end > pill.range.start {
            start = start.min(pill.range.start);
            end = end.max(pill.range.end);
        }
    }
    (start, end)
}

fn snap_caret(doc: &EditorDoc, pos: usize) -> usize {
    if let Some(pill) = doc
        .pills()
        .iter()
        .find(|pill| pos > pill.range.start && pos < pill.range.end)
    {
        return if pos - pill.range.start <= pill.range.end - pos {
            pill.range.start
        } else {
            pill.range.end
        };
    }
    pos
}

fn floor_char(text: &str, mut byte: usize) -> usize {
    byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn ceil_char(text: &str, mut byte: usize) -> usize {
    byte = byte.min(text.len());
    while byte < text.len() && !text.is_char_boundary(byte) {
        byte += 1;
    }
    byte
}

pub(super) fn single_tag(source: &str) -> Option<Tag> {
    if !chips::is_single_tag(source) {
        return None;
    }
    match crate::template::parse(source).nodes.into_iter().next() {
        Some(Node::Tag(tag)) => Some(tag),
        _ => None,
    }
}
