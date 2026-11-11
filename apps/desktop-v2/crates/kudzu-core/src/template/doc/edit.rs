//! Edits on an [`EditorDoc`](super::EditorDoc). One public method is one undo step.
//! Display offsets are UTF-8 bytes. See the parent module for the snap and repair rules.

use std::ops::Range;

use super::load::{literal_from_display, literal_map, place_image_pill, slice_raw, step_source};
use super::{EditorDoc, Image, Mark, PillId, Raw};
use crate::template::chips::{self, PillKind as ChipKind};
use crate::template::lists::list_edits;
use crate::template::{Node, Tag};

impl EditorDoc {
    /// Replace the display range with `text` (a literal; braces are escaped). One undo step.
    /// False when the document does not change.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) -> bool {
        self.transaction(|doc| {
            let inserted = if text.is_empty() {
                Vec::new()
            } else {
                vec![literal_from_display(text)]
            };
            doc.splice(start, end, inserted);
        })
    }

    /// Insert one tag at a caret. `tag_source` must be exactly one tag, and is kept byte-exact.
    /// The new chip gets a fresh [`PillId`]. `None` when `tag_source` is not one tag.
    pub fn insert_chip(
        &mut self,
        at: usize,
        tag_source: &str,
        snippet_name: &dyn Fn(&str) -> Option<&str>,
    ) -> Option<PillId> {
        let tag = single_tag(tag_source)?;
        let label = chips::chip_label(&tag, tag_source, snippet_name);
        let tone = chips::chip_tone(&tag);
        let source = tag_source.to_string();
        let mut id = None;
        self.transaction(|doc| {
            let pill = doc.alloc_pill(tone, ChipKind::Tag);
            id = Some(pill.id);
            let raw = Raw {
                display: label,
                source,
                pill: Some(pill),
                steps: Vec::new(),
            };
            doc.splice(at, at, vec![raw]);
        });
        id
    }
    /// Toggle `• ` or `1. ` on the selected lines. Markers are real text. One undo step.
    pub fn toggle_list(&mut self, from: usize, to: usize, ordered: bool) -> bool {
        self.transaction(|doc| {
            let edits = list_edits(doc.text(), from, to, ordered);
            for edit in edits.into_iter().rev() {
                let inserted = if edit.insert.is_empty() {
                    Vec::new()
                } else {
                    vec![literal_from_display(&edit.insert)]
                };
                doc.splice(edit.start, edit.end, inserted);
            }
        })
    }

    /// `cut_from..new_end` in the new template, when the caller needs to place pasted spans.
    pub(super) fn splice(&mut self, start: usize, end: usize, insert: Vec<Raw>) -> Range<usize> {
        let (start, end) = snap_range(self, start, end);
        let old = concat_source(&self.raws);
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
        self.finish_source_edit(&old)
    }

    pub(super) fn finish_source_edit(&mut self, old: &str) -> Range<usize> {
        let new = concat_source(&self.raws);
        let (cut_from, cut_to, new_end) = diff_region(old, &new);
        let (marks, images) = remap(
            &mut self.mark_src,
            &mut self.image_src,
            cut_from,
            cut_to,
            new_end,
        );
        self.install();
        normalise_marks(self, &marks);
        normalise_images(self, &images);
        self.install();
        cut_from..new_end
    }
}

fn normalise_marks(doc: &mut EditorDoc, touched: &[u32]) {
    let fixes: Vec<(usize, Range<usize>)> = doc
        .mark_src
        .iter()
        .enumerate()
        .filter(|(_, mark)| touched.contains(&mark.seq) && mark.source.start < mark.source.end)
        .map(|(index, mark)| (index, mark.source.clone()))
        .collect();
    for (index, range) in fixes {
        doc.mark_src[index].source = grow_and_snap(doc, &range);
    }
    doc.mark_src
        .retain(|mark| mark.source.start < mark.source.end);
}

fn normalise_images(doc: &mut EditorDoc, touched: &[u32]) {
    let pending: Vec<Image> = doc
        .image_src
        .iter()
        .filter(|image| image.pill.is_none() && touched.contains(&image.seq))
        .cloned()
        .collect();
    for image in pending {
        let (range, tag) = expand_pieces(doc, &image.source);
        if !tag {
            if let Some(id) =
                place_image_pill(&mut doc.raws, range.start, range.end, &mut doc.next_pill)
            {
                if let Some(slot) = doc.image_src.iter_mut().find(|slot| slot.seq == image.seq) {
                    slot.pill = Some(id);
                    slot.source = range;
                }
                continue;
            }
        }
        if let Some(slot) = doc.image_src.iter_mut().find(|slot| slot.seq == image.seq) {
            slot.source = range;
            slot.pill = None;
        }
    }
}

fn grow_and_snap(doc: &EditorDoc, range: &Range<usize>) -> Range<usize> {
    let mut start = range.start;
    let mut end = range.end;
    for piece in doc.pieces() {
        if piece.pill.is_some() && piece.source_utf16.start < end && piece.source_utf16.end > start
        {
            start = start.min(piece.source_utf16.start);
            end = end.max(piece.source_utf16.end);
        }
    }
    start = snap_unit(doc, start, false);
    end = snap_unit(doc, end, true);
    if start < end {
        start..end
    } else {
        start..start
    }
}

fn snap_unit(doc: &EditorDoc, unit: usize, end_side: bool) -> usize {
    for piece in doc.pieces() {
        if unit > piece.source_utf16.start && unit < piece.source_utf16.end && piece.pill.is_none()
        {
            let local = unit - piece.source_utf16.start;
            if piece.steps.iter().any(|(_, src)| *src == local) {
                return unit;
            }
            let snapped = if end_side {
                piece
                    .steps
                    .iter()
                    .find(|(_, src)| *src > local)
                    .map(|(_, src)| *src)
            } else {
                piece
                    .steps
                    .iter()
                    .rev()
                    .find(|(_, src)| *src < local)
                    .map(|(_, src)| *src)
            };
            return piece.source_utf16.start + snapped.unwrap_or(local);
        }
    }
    unit
}

fn expand_pieces(doc: &EditorDoc, range: &Range<usize>) -> (Range<usize>, bool) {
    let mut start = range.start;
    let mut end = range.end;
    let mut tag = false;
    for piece in doc.pieces() {
        if piece.source_utf16.end <= range.start || piece.source_utf16.start >= range.end {
            continue;
        }
        start = start.min(piece.source_utf16.start);
        end = end.max(piece.source_utf16.end);
        if piece.pill.is_some_and(|id| {
            doc.pills()
                .iter()
                .any(|pill| pill.id == id && pill.kind == ChipKind::Tag)
        }) {
            tag = true;
        }
    }
    (start..end, tag)
}

fn remap(
    marks: &mut Vec<Mark>,
    images: &mut Vec<Image>,
    cut_from: usize,
    cut_to: usize,
    new_end: usize,
) -> (Vec<u32>, Vec<u32>) {
    let mut touched_marks = Vec::new();
    for mark in marks.iter_mut() {
        if touched(&mark.source, cut_from, cut_to) {
            touched_marks.push(mark.seq);
        }
        let start = map_pos(mark.source.start, cut_from, cut_to, new_end, false);
        let end = map_pos(mark.source.end, cut_from, cut_to, new_end, true);
        mark.source = if start < end {
            start..end
        } else {
            start..start
        };
    }
    marks.retain(|mark| mark.source.start < mark.source.end);
    let mut touched_images = Vec::new();
    images.retain_mut(|image| {
        if image.pill.is_some() {
            return true;
        }
        if touched(&image.source, cut_from, cut_to) {
            touched_images.push(image.seq);
        }
        let start = map_pos(image.source.start, cut_from, cut_to, new_end, true);
        let end = map_pos(image.source.end, cut_from, cut_to, new_end, false);
        image.source = if start < end {
            start..end
        } else {
            start..start
        };
        image.source.start < image.source.end
    });
    (touched_marks, touched_images)
}

fn map_pos(pos: usize, cut_from: usize, cut_to: usize, new_end: usize, assoc_after: bool) -> usize {
    if pos < cut_from {
        return pos;
    }
    if pos > cut_to {
        return pos + new_end - cut_to;
    }
    if assoc_after {
        new_end
    } else {
        cut_from
    }
}

fn touched(range: &Range<usize>, cut_from: usize, cut_to: usize) -> bool {
    (range.start < cut_to && range.end > cut_from)
        || range.start == cut_from
        || range.end == cut_to
        || range.start == cut_to
        || range.end == cut_from
}

pub(super) fn overlaps(range: &Range<usize>, cut_from: usize, cut_to: usize) -> bool {
    range.start < cut_to && range.end > cut_from
}

fn diff_region(old: &str, new: &str) -> (usize, usize, usize) {
    let old_u: Vec<u16> = old.encode_utf16().collect();
    let new_u: Vec<u16> = new.encode_utf16().collect();
    let mut prefix = 0;
    while prefix < old_u.len() && prefix < new_u.len() && old_u[prefix] == new_u[prefix] {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_u.len() - prefix
        && suffix < new_u.len() - prefix
        && old_u[old_u.len() - 1 - suffix] == new_u[new_u.len() - 1 - suffix]
    {
        suffix += 1;
    }
    (prefix, old_u.len() - suffix, new_u.len() - suffix)
}

fn repair(raws: &mut Vec<Raw>) {
    let mut i = 0;
    while i < raws.len() {
        if raws[i].pill.is_some() {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < raws.len() && raws[j].pill.is_none() {
            j += 1;
        }
        let source: String = raws[i..j].iter().map(|raw| raw.source.as_str()).collect();
        let display: String = raws[i..j].iter().map(|raw| raw.display.as_str()).collect();
        if literal_run_ok(&source, &display) {
            i = j;
        } else {
            raws.splice(i..j, std::iter::once(literal_from_display(&display)));
            i += 1;
        }
    }
}

fn literal_run_ok(source: &str, display: &str) -> bool {
    let (decoded, _) = literal_map(source);
    if decoded != display {
        return false;
    }
    let parsed = crate::template::parse(source);
    // `{{` decodes as itself and is stored as text, but it is an unterminated tag. A display of
    // `{{` has to be saved as `\{{`, or the next edit can turn the braces into a tag.
    if !parsed.diagnostics.is_empty() {
        return false;
    }
    match parsed.nodes.as_slice() {
        [] => display.is_empty(),
        [Node::Text { text, .. }] => text == display,
        _ => false,
    }
}

pub(super) fn concat_source(raws: &[Raw]) -> String {
    raws.iter().map(|raw| raw.source.as_str()).collect()
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
    (snap_out(doc, start, false), snap_out(doc, end, true))
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
    snap_out_near(doc, pos)
}

fn snap_out(doc: &EditorDoc, pos: usize, end_side: bool) -> usize {
    let Some(piece) = doc
        .pieces()
        .iter()
        .find(|piece| pos > piece.display.start && pos < piece.display.end)
    else {
        return pos;
    };
    if piece.pill.is_some() {
        return if end_side {
            piece.display.end
        } else {
            piece.display.start
        };
    }
    let local = pos - piece.display.start;
    if piece.steps.iter().any(|(display, _)| *display == local) {
        return pos;
    }
    let snapped = if end_side {
        piece
            .steps
            .iter()
            .find(|(display, _)| *display > local)
            .map(|(display, _)| *display)
    } else {
        piece
            .steps
            .iter()
            .rev()
            .find(|(display, _)| *display < local)
            .map(|(display, _)| *display)
    };
    piece.display.start + snapped.unwrap_or(local)
}

fn snap_out_near(doc: &EditorDoc, pos: usize) -> usize {
    let Some(piece) = doc
        .pieces()
        .iter()
        .find(|piece| pos > piece.display.start && pos < piece.display.end)
    else {
        return pos;
    };
    if piece.pill.is_some() {
        return piece.display.start;
    }
    let local = pos - piece.display.start;
    let mut prev = 0usize;
    let mut next = piece.display.end - piece.display.start;
    for (display, _) in &piece.steps {
        if *display <= local {
            prev = *display;
        }
        if *display > local {
            next = *display;
            break;
        }
    }
    piece.display.start
        + if local - prev <= next - local {
            prev
        } else {
            next
        }
}

pub(super) fn slice_display(raw: &Raw, from: usize, to: usize) -> Raw {
    let src_from = step_source(&raw.steps, from, false);
    let src_to = step_source(&raw.steps, to, true);
    slice_raw(raw, src_from, src_to)
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

pub(super) fn clip_rebase(range: &Range<usize>, from: usize, to: usize) -> Option<Range<usize>> {
    let start = range.start.max(from);
    let end = range.end.min(to);
    (start < end).then_some((start - from)..(end - from))
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
