//! The editor document: template source and formatting on one side, the text the field edits
//! on the other.
//!
//! [`EditorDoc`] is pure Rust. It does not mention Rune. S52 binds a `TextEdit` to it.
//!
//! # What is stored
//!
//! Each [`SourcePiece`] is either a literal run or one chip. Literal pieces remember every
//! scalar boundary, including both displayed braces of `\{{` (three source units). Chip pieces remember the original tag bytes. [`EditorDoc::to_template`]
//! concatenates those bytes, so an untouched document round-trips exactly.
//!
//! Marks and images store template UTF-16 endpoints. Loading does not grow a span that only
//! partly covers a chip, and it does not fold a tag into an image. [`EditorMark::range`] is
//! only the painted display range; S52 must save with [`EditorDoc::to_template`], never by
//! reading painted ranges back.
//!
//! # Edits and mode
//!
//! Offsets are UTF-8 display bytes. Only chips are atomic. Any scalar boundary in a literal
//! can be edited, including the middle of `\{{`, combining text or CRLF. [`EditorDoc::replace`]
//! returns the inserted display range. Typing `{{date}}` stays literal; [`EditorDoc::paste_source`]
//! deliberately parses external template source into chips.
//!
//! Every edit maps saved span endpoints through the old display, the actual replacement,
//! and the new source map. Repair changes only conflicting escape steps; distant source stays
//! byte-exact. Untouched partial chip/escape endpoints retain their original source offsets.
//! Marks include text typed at their edges; images exclude it.
//!
//! [`EditorDoc::set_formatted`] switches modes and clears spans in one undo step. Formatted
//! documents save `Some(Formatting)` even with no spans. A style/image insertion or rich paste
//! with spans enables Formatted as part of that operation.
//!
//! # Identity, clipboard, and history
//!
//! [`PillId`] identifies one chip through edits and undo. Rune's [`EditorDoc::html_id`] carries
//! `{tone}:{source}` and is intentionally shared by identical chips. S52 maps Rune pills to
//! core pills in document order, using `PillId` plus the range/order, never a unique HTML id.
//! [`CopyPayload::from_fragment`] validates the plain source against Rune's ordered fragment.
//! [`EditorDoc::copy`] includes rich spans; image transport from Rune remains S76's work.
//!
//! Loading creates no undo step. S52 must also clear Rune's history after each load. Core
//! keeps at most 200 history steps and shares image data URLs with `Arc<str>` across snapshots.
//! [`EditorDoc::relabel`] refreshes library names in the current document and history without
//! becoming an undo step. Source and formatting do not change.

mod chip_edit;
mod clipboard;
mod edit;
mod format;
mod history;
mod literal;
mod load;
mod mapping;

use std::ops::Range;
use std::sync::Arc;

use super::chips::{self, PillKind, SnippetNames, Tone};
use crate::formatting::{FormatSpan, FormatStyle, Formatting};

/// The UTF-16 slice `range` of `source`. A unit that would split a scalar is omitted.
pub(super) fn slice_utf16(source: &str, range: Range<usize>) -> String {
    load::slice_utf16(source, range)
}

/// Stable identity of one chip. Not Rune's `data-pill` string; see [`EditorDoc::html_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PillId(u64);

impl PillId {
    /// The raw counter value. Unique among pills that have been created in this document.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// A style the field can paint. Images are chips, not marks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorStyle {
    Bold,
    Italic,
    Underline,
    Link { url: String },
}

/// What [`EditorDoc::change_format`] applies. `Clear` removes styles and image spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatChange {
    Bold,
    Italic,
    Underline,
    Link(String),
    Clear,
}

/// One chip in the display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPill {
    pub id: PillId,
    /// Display bytes of the whole chip. A caret cannot sit inside this range.
    pub range: Range<usize>,
    pub label: String,
    pub tone: Tone,
    pub kind: PillKind,
    /// Exact tag source, or the literal source an image chip stands in for.
    pub source: String,
}

/// A bold, italic, underline or link span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMark {
    /// Display bytes for painting. May cover a whole chip when [`Self::source`] covers only
    /// part of that chip's tag.
    pub range: Range<usize>,
    /// Template UTF-16. This is what [`EditorDoc::to_template`] writes.
    pub source: Range<usize>,
    pub style: EditorStyle,
}

/// An image, either as its own chip ([`Self::pill`]) or as an unaligned span (`None`) when
/// the saved range overlaps a tag or does not land on a step boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorImage {
    pub pill: Option<PillId>,
    /// Template UTF-16. Unaligned images keep the endpoints they were loaded with until an
    /// edit touches them.
    pub source: Range<usize>,
    pub data_url: Arc<str>,
    pub width: u32,
    pub alt: String,
}

/// One literal run or one chip, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePiece {
    /// Display bytes in [`EditorDoc::text`].
    pub display: Range<usize>,
    /// Original template bytes for this piece.
    pub source: String,
    /// UTF-16 of [`Self::source`] inside the concatenated template.
    pub source_utf16: Range<usize>,
    pub pill: Option<PillId>,
    /// Literal scalar boundaries as `(display byte, source utf-16)` from the start of this piece.
    /// Empty for a chip, which maps as one atom.
    pub steps: Vec<(usize, usize)>,
}

/// Plain template source plus the rich form a same-app paste needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyPayload {
    /// Template source for the plain pasteboard. Literals are escaped; chips are their tags.
    pub plain: String,
    pub runs: Vec<CopiedRun>,
    /// Marks in UTF-16 units of [`Self::plain`].
    pub marks: Vec<CopiedMark>,
    /// Images in UTF-16 units of [`Self::plain`].
    pub images: Vec<CopiedImage>,
}

/// One run of a copy, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedRun {
    pub display: String,
    /// Tag bytes, or the literal slice with the escapes it already had.
    pub source: String,
    pub chip: Option<CopiedChip>,
}

/// A chip inside a [`CopyPayload`]. Paste mints a new [`PillId`]; `html_id` is the Rune id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedChip {
    pub id: PillId,
    pub tone: Tone,
    pub kind: PillKind,
    pub html_id: String,
}

/// A mark clipped to a copy, in UTF-16 units of [`CopyPayload::plain`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedMark {
    pub source: Range<usize>,
    pub style: EditorStyle,
}

/// An image clipped to a copy, in UTF-16 units of [`CopyPayload::plain`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedImage {
    pub source: Range<usize>,
    pub data_url: Arc<str>,
    pub width: u32,
    pub alt: String,
}

#[derive(Clone)]
struct Raw {
    display: String,
    source: String,
    pill: Option<RawPill>,
    steps: Vec<(usize, usize)>,
}

#[derive(Clone)]
struct RawPill {
    id: PillId,
    tone: Tone,
    kind: PillKind,
}

#[derive(Clone)]
struct Mark {
    source: Range<usize>,
    style: EditorStyle,
    seq: u32,
}

#[derive(Clone)]
struct Image {
    pill: Option<PillId>,
    source: Range<usize>,
    data_url: Arc<str>,
    width: u32,
    alt: String,
    seq: u32,
}

#[derive(Clone)]
struct Snapshot {
    formatted: bool,
    raws: Vec<Raw>,
    marks: Vec<Mark>,
    images: Vec<Image>,
    seq: u32,
}

/// Display text, chips, marks and images, plus the ordered map back to template source.
#[derive(Clone)]
pub struct EditorDoc {
    formatted: bool,
    text: String,
    pills: Vec<EditorPill>,
    marks: Vec<EditorMark>,
    images: Vec<EditorImage>,
    map: Vec<SourcePiece>,
    raws: Vec<Raw>,
    mark_src: Vec<Mark>,
    image_src: Vec<Image>,
    next_pill: u64,
    seq: u32,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    busy: bool,
}

impl EditorDoc {
    /// Load `source`. Formatting is applied only when its `source` equals `source`.
    /// Snippet chips label as `↳ Missing snippet`.
    pub fn from_template(source: &str, formatting: Option<&Formatting>) -> Self {
        Self::from_template_named(source, formatting, &|_| None)
    }

    /// [`Self::from_template`] with the library lookup a `{{snippet}}` chip needs.
    ///
    /// The undo stack is empty. Building a document is not an edit.
    pub fn from_template_named(
        source: &str,
        formatting: Option<&Formatting>,
        snippet_name: SnippetNames<'_>,
    ) -> Self {
        let mut next_pill = 1u64;
        let mut raws = load::pieces_from_parse(source, snippet_name, &mut next_pill);
        let mut marks = Vec::new();
        let mut images = Vec::new();
        let mut seq = 0u32;
        if formatting.is_some_and(|f| f.source == source) {
            if let Some(formatting) = formatting {
                load::apply_formatting(
                    &mut raws,
                    &mut marks,
                    &mut images,
                    &mut seq,
                    source,
                    formatting,
                    &mut next_pill,
                );
            }
        }
        let mut doc = Self {
            formatted: formatting.is_some_and(|f| f.source == source),
            text: String::new(),
            pills: Vec::new(),
            marks: Vec::new(),
            images: Vec::new(),
            map: Vec::new(),
            raws,
            mark_src: marks,
            image_src: images,
            next_pill,
            seq,
            undo: Vec::new(),
            redo: Vec::new(),
            busy: false,
        };
        doc.install();
        doc
    }

    /// Template source and, in Formatted mode, the formatting that names that source.
    /// Span endpoints are the stored UTF-16 ranges, in the order they were applied.
    pub fn to_template(&self) -> (String, Option<Formatting>) {
        let source: String = self.raws.iter().map(|r| r.source.as_str()).collect();
        let mut spans: Vec<(u32, FormatSpan)> = Vec::new();
        for mark in &self.mark_src {
            if mark.source.start < mark.source.end {
                spans.push((
                    mark.seq,
                    FormatSpan {
                        start: mark.source.start,
                        end: mark.source.end,
                        style: load::to_format_style(&mark.style),
                    },
                ));
            }
        }
        for image in &self.image_src {
            if image.source.start < image.source.end {
                spans.push((
                    image.seq,
                    FormatSpan {
                        start: image.source.start,
                        end: image.source.end,
                        style: FormatStyle::Image {
                            data_url: image.data_url.to_string(),
                            width: image.width,
                        },
                    },
                ));
            }
        }
        spans.sort_by_key(|(seq, _)| *seq);
        let spans: Vec<_> = spans.into_iter().map(|(_, span)| span).collect();
        let formatting = self.formatted.then(|| Formatting {
            source: source.clone(),
            spans,
        });
        (source, formatting)
    }

    /// The field's display text. Chip labels stand in for tags.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Chips in document order.
    pub fn pills(&self) -> &[EditorPill] {
        &self.pills
    }

    /// Marks. [`EditorMark::source`] is authoritative; [`EditorMark::range`] is for painting.
    pub fn marks(&self) -> &[EditorMark] {
        &self.marks
    }

    /// Images, pill-backed and unaligned.
    pub fn images(&self) -> &[EditorImage] {
        &self.images
    }

    /// The ordered source map.
    pub fn pieces(&self) -> &[SourcePiece] {
        &self.map
    }

    /// Rune `data-pill` id for a tag chip, or `None` for an unknown id or an image chip.
    ///
    /// Identical chips share `{tone}:{source}`. Use `PillId` and document order to identify
    /// one occurrence when binding Rune marks back to the document.
    pub fn html_id(&self, id: PillId) -> Option<String> {
        let pill = self.pills.iter().find(|p| p.id == id)?;
        if pill.kind != PillKind::Tag {
            return None;
        }
        Some(chips::html_pill_id(pill.tone, &pill.source))
    }

    /// Display byte → template UTF-16. A byte that lands on a piece boundary ignores `end`.
    /// Inside a step or a chip, `end` selects the later edge.
    pub fn display_to_source(&self, byte: usize, end: bool) -> usize {
        let byte = byte.min(self.text.len());
        for (i, piece) in self.map.iter().enumerate() {
            if byte == piece.display.start {
                return piece.source_utf16.start;
            }
            if byte < piece.display.end {
                if piece.pill.is_some() {
                    return if end {
                        piece.source_utf16.end
                    } else {
                        piece.source_utf16.start
                    };
                }
                let local = byte - piece.display.start;
                return piece.source_utf16.start + load::step_source(&piece.steps, local, end);
            }
            if byte == piece.display.end && i + 1 == self.map.len() {
                return piece.source_utf16.end;
            }
        }
        self.map.last().map(|p| p.source_utf16.end).unwrap_or(0)
    }

    /// Template UTF-16 → display byte. A unit that lands on a piece boundary ignores `end`.
    pub fn source_to_display(&self, unit: usize, end: bool) -> usize {
        for (i, piece) in self.map.iter().enumerate() {
            if unit == piece.source_utf16.start {
                return piece.display.start;
            }
            if unit < piece.source_utf16.end {
                if piece.pill.is_some() {
                    return if end {
                        piece.display.end
                    } else {
                        piece.display.start
                    };
                }
                let local = unit - piece.source_utf16.start;
                return piece.display.start + load::step_display(&piece.steps, local, end);
            }
            if unit == piece.source_utf16.end && i + 1 == self.map.len() {
                return piece.display.end;
            }
        }
        self.text.len()
    }

    fn alloc_pill(&mut self, tone: Tone, kind: PillKind) -> RawPill {
        let id = PillId(self.next_pill);
        self.next_pill += 1;
        RawPill { id, tone, kind }
    }

    fn alloc_seq(&mut self) -> u32 {
        let seq = self.seq;
        self.seq += 1;
        seq
    }

    /// Rebuild display indexes from `raws`. Pill ids already on the raws are kept.
    fn install(&mut self) {
        let mut text = String::new();
        let mut map = Vec::new();
        let mut pills = Vec::new();
        let mut src = 0usize;
        for raw in &self.raws {
            let display = text.len()..text.len() + raw.display.len();
            text.push_str(&raw.display);
            let src_len = load::utf16_len(&raw.source);
            let source_utf16 = src..src + src_len;
            src += src_len;
            let pill = raw.pill.as_ref().map(|pill| {
                pills.push(EditorPill {
                    id: pill.id,
                    range: display.clone(),
                    label: raw.display.clone(),
                    tone: pill.tone,
                    kind: pill.kind,
                    source: raw.source.clone(),
                });
                pill.id
            });
            map.push(SourcePiece {
                display,
                source: raw.source.clone(),
                source_utf16,
                pill,
                steps: raw.steps.clone(),
            });
        }
        let live: Vec<PillId> = pills.iter().map(|p| p.id).collect();
        self.image_src
            .retain(|image| image.pill.is_none_or(|id| live.contains(&id)));
        for image in &mut self.image_src {
            if let Some(id) = image.pill {
                if let Some(piece) = map.iter().find(|p| p.pill == Some(id)) {
                    image.source = piece.source_utf16.clone();
                }
                if let Some(pill) = pills.iter().find(|p| p.id == id) {
                    image.alt = pill.label.clone();
                }
            }
        }
        self.text = text;
        self.map = map;
        self.pills = pills;
        self.marks = self
            .mark_src
            .iter()
            .filter(|m| m.source.start < m.source.end)
            .map(|m| EditorMark {
                range: load::paint_range(self, &m.source),
                source: m.source.clone(),
                style: m.style.clone(),
            })
            .collect();
        self.images = self
            .image_src
            .iter()
            .map(|image| EditorImage {
                pill: image.pill,
                source: image.source.clone(),
                data_url: image.data_url.clone(),
                width: image.width,
                alt: image.alt.clone(),
            })
            .collect();
    }
}
