//! The editor document: template source and formatting on one side, the text the field edits
//! on the other.
//!
//! [`EditorDoc`] is pure Rust. It does not mention Rune. S52 binds a `TextEdit` to it.
//!
//! # What is stored
//!
//! Each [`SourcePiece`] is either a literal run or one chip. Literal pieces remember every
//! grapheme step, including `\{{` (three source units, two display bytes) and a combining
//! character. Chip pieces remember the original tag bytes. [`EditorDoc::to_template`]
//! concatenates those bytes, so an untouched document round-trips exactly.
//!
//! Marks and images store template UTF-16 endpoints. Loading does not grow a span that only
//! partly covers a chip, and it does not fold a tag into an image. [`EditorMark::range`] is
//! only the painted display range; S52 must save with [`EditorDoc::to_template`], never by
//! reading painted ranges back.
//!
//! # Edits
//!
//! [`EditorDoc::replace`], [`EditorDoc::insert_chip`], [`EditorDoc::change_format`],
//! [`EditorDoc::toggle_list`] and the image methods are one undo step each. Offsets are UTF-8
//! display bytes, which is what Rune uses. A caret inside a chip snaps to the nearer edge. A
//! non-empty range that overlaps a chip takes the whole chip. An edit cannot split `\{{`, a
//! combining grapheme, or a surrogate pair.
//!
//! After an edit, a run of literal pieces whose concatenated source would no longer parse as
//! that display text is rewritten with the serializer (`\` → `\\`, `{{` → `\{{`). A run that
//! still decodes is left byte-exact, so an untouched `\{{` stays. Deleting a chip that sat
//! between two literal `{` characters therefore leaves the display `{{` (source `\{{`). The
//! TypeScript editor would turn those braces into a tag, because there the document *is* the
//! source. This editor's document is the display.
//!
//! Marks the edit does not touch keep their endpoints and only shift. A mark the edit overlaps,
//! or whose endpoint sits on the cut, grows to the chips it intersects and snaps out to a step.
//! Typing at either end of a bold span extends it. Typing beside an image does not.
//!
//! # Identity and the pasteboard
//!
//! [`PillId`] is chosen when the chip is created and is not renumbered when other text changes.
//! Undo and redo put the same ids back. [`EditorDoc::html_id`] is a different id, for Rune's
//! `<span data-pill>`: `{tone}:{source}`, with `#2`, `#3`, … when the field holds two identical
//! chips. [`parse_html_pill_id`](crate::template::chips::parse_html_pill_id) strips that suffix.
//!
//! [`EditorDoc::copy`] writes the template source (chips as their tags, literals escaped) plus
//! a rich payload. [`EditorDoc::paste_own`] rebuilds chips from that payload in one undo step
//! and mints new [`PillId`]s. [`EditorDoc::paste_source`] does the same for plain text from
//! another app: the text is template source, so `{{tags}}` become chips. Image pills have no
//! tag source; a plain-text paste keeps their alt words, and only [`EditorDoc::paste_own`]
//! brings the image back (S76 owns the pasteboard bytes).
//!
//! # History
//!
//! [`EditorDoc::from_template`] starts with an empty undo stack. The first
//! [`EditorDoc::undo`] does nothing. S52 must also call `TextEdit::clear_history` after it
//! builds the field, or Rune's own undo will strip the chips it just inserted.

mod clipboard;
mod edit;
mod format;
mod load;

use std::ops::Range;

use super::chips::{self, PillKind, Tone};
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
    pub data_url: String,
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
    /// Literal graphemes as `(display byte, source utf-16)` from the start of this piece.
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
    pub data_url: String,
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
    data_url: String,
    width: u32,
    alt: String,
    seq: u32,
}

#[derive(Clone)]
struct Snapshot {
    raws: Vec<Raw>,
    marks: Vec<Mark>,
    images: Vec<Image>,
    seq: u32,
}

/// Display text, chips, marks and images, plus the ordered map back to template source.
#[derive(Clone)]
pub struct EditorDoc {
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
    /// Load `source`. Formatting is applied when its `source` is empty or equal to `source`.
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
        snippet_name: &dyn Fn(&str) -> Option<&str>,
    ) -> Self {
        let mut next_pill = 1u64;
        let mut raws = load::pieces_from_parse(source, snippet_name, &mut next_pill);
        let mut marks = Vec::new();
        let mut images = Vec::new();
        let mut seq = 0u32;
        if formatting.is_some_and(|f| f.source.is_empty() || f.source == source) {
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

    /// Template source and, when there are spans, the formatting that names that source.
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
                            data_url: image.data_url.clone(),
                            width: image.width,
                        },
                    },
                ));
            }
        }
        spans.sort_by_key(|(seq, _)| *seq);
        let spans: Vec<_> = spans.into_iter().map(|(_, span)| span).collect();
        let formatting = (!spans.is_empty()).then(|| Formatting {
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
    /// The first chip with a given tone and source is `{tone}:{source}`. Each further one in
    /// this document gets `#2`, `#3`, and so on. Paste strips the suffix; the two chips stay
    /// distinct here because each has its own [`PillId`].
    pub fn html_id(&self, id: PillId) -> Option<String> {
        let pill = self.pills.iter().find(|p| p.id == id)?;
        if pill.kind != PillKind::Tag {
            return None;
        }
        let occurrence = self
            .pills
            .iter()
            .take_while(|p| p.id != id)
            .filter(|p| p.kind == PillKind::Tag && p.tone == pill.tone && p.source == pill.source)
            .count() as u32
            + 1;
        Some(chips::html_pill_id(pill.tone, &pill.source, occurrence))
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

    /// Whether [`Self::undo`] would change the document. False on a freshly loaded document.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether [`Self::redo`] would change the document.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Restore the previous edit. Returns false when the stack is empty, and changes nothing.
    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snap());
        self.restore(prev);
        true
    }

    /// Reapply an undone edit. Returns false when there is nothing to redo.
    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snap());
        self.restore(next);
        true
    }

    fn transaction(&mut self, body: impl FnOnce(&mut Self)) -> bool {
        if self.busy {
            body(self);
            return true;
        }
        let before = self.snap();
        self.busy = true;
        body(self);
        self.busy = false;
        if self.same(&before) {
            return false;
        }
        self.undo.push(before);
        self.redo.clear();
        true
    }

    fn snap(&self) -> Snapshot {
        Snapshot {
            raws: self.raws.clone(),
            marks: self.mark_src.clone(),
            images: self.image_src.clone(),
            seq: self.seq,
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.raws = snap.raws;
        self.mark_src = snap.marks;
        self.image_src = snap.images;
        self.seq = snap.seq;
        self.install();
    }

    fn same(&self, snap: &Snapshot) -> bool {
        load::raws_eq(&self.raws, &snap.raws)
            && load::marks_eq(&self.mark_src, &snap.marks)
            && load::images_eq(&self.image_src, &snap.images)
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
