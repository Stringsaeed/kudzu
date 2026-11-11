//! Map saved endpoints through the actual display replacement, including escape repair.
use super::load::{paint_range, slice_utf16};
use super::{EditorDoc, Image, Mark};
use std::ops::Range;

#[derive(Clone, Debug)]
pub(super) struct DisplayEdit {
    pub old: Range<usize>,
    pub new: Range<usize>,
}
impl DisplayEdit {
    pub fn position(&self, pos: usize, after: bool) -> usize {
        if pos < self.old.start {
            pos
        } else if pos > self.old.end {
            pos - self.old.end + self.new.end
        } else if after {
            self.new.end
        } else {
            self.new.start
        }
    }
    fn leaves_cell(&self, cell: &Range<usize>) -> bool {
        self.old.end <= cell.start || self.old.start >= cell.end
    }
}

struct Endpoint {
    display: usize,
    // Preserve a saved endpoint inside an untouched chip or escape step byte-exact.
    interior: Option<(Range<usize>, String, usize)>,
}
impl Endpoint {
    fn capture(doc: &EditorDoc, source: usize, end: bool) -> Self {
        let interior = doc.map.iter().find_map(|piece| {
            if source <= piece.source_utf16.start || source >= piece.source_utf16.end {
                return None;
            }
            let local = source - piece.source_utf16.start;
            let (display, units) = if piece.pill.is_some() {
                (piece.display.clone(), 0..piece.source_utf16.len())
            } else {
                let pair = piece
                    .steps
                    .windows(2)
                    .find(|w| w[0].1 < local && local < w[1].1)?;
                (
                    piece.display.start + pair[0].0..piece.display.start + pair[1].0,
                    pair[0].1..pair[1].1,
                )
            };
            Some((
                display,
                slice_utf16(&piece.source, units.clone()),
                local - units.start,
            ))
        });
        Self {
            display: doc.source_to_display(source, end),
            interior,
        }
    }
    fn resolve(&self, doc: &EditorDoc, edit: &DisplayEdit, end: bool, after: bool) -> usize {
        if let Some((cell, bytes, offset)) = &self.interior {
            if edit.leaves_cell(cell) {
                let a = doc.display_to_source(edit.position(cell.start, true), false);
                let b = doc.display_to_source(edit.position(cell.end, false), true);
                let source: String = doc
                    .map
                    .iter()
                    .filter(|p| p.source_utf16.start < b && p.source_utf16.end > a)
                    .map(|p| {
                        slice_utf16(
                            &p.source,
                            a.saturating_sub(p.source_utf16.start)
                                ..(b.min(p.source_utf16.end) - p.source_utf16.start),
                        )
                    })
                    .collect();
                if &source == bytes {
                    return a + offset;
                }
            }
        }
        doc.display_to_source(edit.position(self.display, after), end)
    }
}

pub(super) struct SpanMap {
    marks: Vec<(Mark, Endpoint, Endpoint)>,
    images: Vec<(Image, Endpoint, Endpoint)>,
}
impl SpanMap {
    pub fn capture(doc: &EditorDoc) -> Self {
        Self {
            marks: doc
                .mark_src
                .iter()
                .map(|m| {
                    (
                        m.clone(),
                        Endpoint::capture(doc, m.source.start, false),
                        Endpoint::capture(doc, m.source.end, true),
                    )
                })
                .collect(),
            images: doc
                .image_src
                .iter()
                .map(|m| {
                    (
                        m.clone(),
                        Endpoint::capture(doc, m.source.start, false),
                        Endpoint::capture(doc, m.source.end, true),
                    )
                })
                .collect(),
        }
    }
    pub fn apply(self, doc: &mut EditorDoc, edit: &DisplayEdit) {
        doc.mark_src = self
            .marks
            .into_iter()
            .filter_map(|(mut mark, start, end)| {
                mark.source =
                    start.resolve(doc, edit, false, false)..end.resolve(doc, edit, true, true);
                (mark.source.start < mark.source.end).then_some(mark)
            })
            .collect();
        doc.image_src = self
            .images
            .into_iter()
            .filter_map(|(mut image, start, end)| {
                if let Some(id) = image.pill {
                    image.source = doc
                        .map
                        .iter()
                        .find(|p| p.pill == Some(id))?
                        .source_utf16
                        .clone();
                } else {
                    image.source =
                        start.resolve(doc, edit, false, true)..end.resolve(doc, edit, true, false);
                }
                (image.source.start < image.source.end).then_some(image)
            })
            .collect();
        doc.install();
    }
}

/// Rebase a copied span through its display positions after insertion and repair.
pub(super) fn pasted_range(
    fragment: &EditorDoc,
    doc: &EditorDoc,
    range: &Range<usize>,
    start: usize,
) -> Range<usize> {
    let translation = DisplayEdit {
        old: 0..0,
        new: start..start,
    };
    Endpoint::capture(fragment, range.start, false).resolve(doc, &translation, false, false)
        ..Endpoint::capture(fragment, range.end, true).resolve(doc, &translation, true, true)
}

pub(super) fn copied_range(
    original: &EditorDoc,
    fragment: &EditorDoc,
    range: &Range<usize>,
    from: usize,
    to: usize,
) -> Option<Range<usize>> {
    let display = paint_range(original, range);
    if display.end <= from || display.start >= to {
        return None;
    }
    let translation = DisplayEdit {
        old: 0..from,
        new: 0..0,
    };
    let start = if display.start < from {
        0
    } else {
        Endpoint::capture(original, range.start, false).resolve(
            fragment,
            &translation,
            false,
            false,
        )
    };
    let end = if display.end > to {
        fragment.display_to_source(fragment.text.len(), true)
    } else {
        Endpoint::capture(original, range.end, true).resolve(fragment, &translation, true, true)
    };
    (start < end).then_some(start..end)
}
