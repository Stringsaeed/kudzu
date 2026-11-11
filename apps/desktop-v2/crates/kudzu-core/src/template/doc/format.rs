//! Formatting and image edits on an [`EditorDoc`](super::EditorDoc).
//! One public method is one undo step. See the parent module for span rules.

use std::ops::Range;

use super::edit::{concat_source, overlaps, snap_range};
use super::load::{literal_from_display, literal_map};
use super::{EditorDoc, EditorStyle, FormatChange, Image, Mark, PillId};
use crate::formatting::safe_link;
use crate::template::chips::{self, PillKind as ChipKind};
use crate::template::images::{self, ImageError};

impl EditorDoc {
    /// Bold, italic, underline, link, or clear, over the display selection.
    ///
    /// The selection grows over chips and images. The same style covering the whole range
    /// toggles off (a link toggles off only when the URL matches). Otherwise overlapping spans
    /// of that kind are cut and the new span is pushed last. An empty selection does nothing.
    /// An unsafe link is not added.
    pub fn change_format(&mut self, start: usize, end: usize, change: FormatChange) -> bool {
        self.transaction(|doc| doc.apply_format(start, end, &change))
    }

    /// Replace the display range with an image chip. Width is clamped to 16..=512.
    /// The chip's text is `[` + sanitised alt + `]`. One undo step.
    pub fn insert_image(
        &mut self,
        start: usize,
        end: usize,
        data_url: &str,
        width: u32,
        alt: &str,
    ) -> Result<PillId, ImageError> {
        if !images::accept_data_url(data_url) {
            return Err(ImageError::UnsafeData);
        }
        let (start, end) = snap_range(self, start, end);
        let cut_from = self.display_to_source(start, false);
        let cut_to = self.display_to_source(end, true);
        let used: usize = self
            .image_src
            .iter()
            .filter(|image| !overlaps(&image.source, cut_from, cut_to))
            .map(|image| image.data_url.len())
            .sum();
        if !images::within_budget(used + data_url.len()) {
            return Err(ImageError::TooMany);
        }
        let width = images::clamp_width(width);
        let label = images::alt_literal(alt);
        let url = data_url.to_string();
        let mut id = None;
        self.transaction(|doc| {
            doc.image_src
                .retain(|image| !overlaps(&image.source, cut_from, cut_to));
            let pill = doc.alloc_pill(chips::Tone::Form, ChipKind::Image);
            id = Some(pill.id);
            let mut raw = literal_from_display(&label);
            raw.pill = Some(pill);
            raw.steps.clear();
            doc.splice(start, end, vec![raw]);
            let seq = doc.alloc_seq();
            doc.image_src.push(Image {
                pill: id,
                source: 0..0,
                data_url: url,
                width,
                alt: label,
                seq,
            });
            doc.install();
        });
        id.ok_or(ImageError::Missing)
    }

    /// Set an image chip's label to the sanitised alt, without adding brackets. One undo step.
    pub fn set_image_alt(&mut self, id: PillId, alt: &str) -> Result<(), ImageError> {
        if !self.image_src.iter().any(|image| image.pill == Some(id)) {
            return Err(ImageError::Missing);
        }
        let label = images::sanitize_alt(alt);
        self.transaction(|doc| {
            let Some(index) = doc
                .raws
                .iter()
                .position(|raw| raw.pill.as_ref().is_some_and(|p| p.id == id))
            else {
                return;
            };
            if doc.raws[index].display == label {
                return;
            }
            let old = concat_source(&doc.raws);
            let pill = doc.raws[index].pill.clone();
            let mut raw = literal_from_display(&label);
            raw.pill = pill;
            raw.steps.clear();
            doc.raws[index] = raw;
            doc.finish_source_edit(&old);
        });
        Ok(())
    }

    /// Clamp `width` to 16..=512 and store it. One undo step unless the width does not change.
    pub fn set_image_width(&mut self, id: PillId, width: u32) -> Result<(), ImageError> {
        if !self.image_src.iter().any(|image| image.pill == Some(id)) {
            return Err(ImageError::Missing);
        }
        let width = images::clamp_width(width);
        self.transaction(|doc| {
            if let Some(image) = doc
                .image_src
                .iter_mut()
                .find(|image| image.pill == Some(id))
            {
                image.width = width;
            }
            doc.install();
        });
        Ok(())
    }

    fn apply_format(&mut self, start: usize, end: usize, change: &FormatChange) {
        let (start, end) = snap_range(self, start, end);
        if start == end {
            return;
        }
        let from = self.display_to_source(start, false);
        let to = self.display_to_source(end, true);
        if from >= to {
            return;
        }
        let mut items = self.span_items();
        let remove = matches!(change, FormatChange::Clear)
            || items.iter().any(|item| {
                item.source.start <= from && item.source.end >= to && item.same_as(change)
            });
        let mut next = Vec::new();
        for item in items.drain(..) {
            if item.source.end <= from || item.source.start >= to || !item.kind_matches(change) {
                next.push(item);
                continue;
            }
            if item.source.start < from {
                next.push(item.clipped(item.source.start..from));
            }
            if item.source.end > to {
                next.push(item.clipped(to..item.source.end));
            }
        }
        if !remove {
            if let Some(item) = SpanItem::fresh(change, from..to) {
                next.push(item);
            }
        }
        let old = concat_source(&self.raws);
        self.write_spans(&next);
        self.unpill_orphan_images();
        if concat_source(&self.raws) != old {
            self.finish_source_edit(&old);
        } else {
            self.install();
        }
    }

    fn span_items(&self) -> Vec<SpanItem> {
        let mut items: Vec<(u32, SpanItem)> = self
            .mark_src
            .iter()
            .map(|mark| {
                (
                    mark.seq,
                    SpanItem {
                        source: mark.source.clone(),
                        body: Body::Mark(mark.style.clone()),
                    },
                )
            })
            .chain(self.image_src.iter().map(|image| {
                (
                    image.seq,
                    SpanItem {
                        source: image.source.clone(),
                        body: Body::Image {
                            data_url: image.data_url.clone(),
                            width: image.width,
                            alt: image.alt.clone(),
                            pill: image.pill,
                        },
                    },
                )
            }))
            .collect();
        items.sort_by_key(|(seq, _)| *seq);
        items.into_iter().map(|(_, item)| item).collect()
    }

    fn write_spans(&mut self, items: &[SpanItem]) {
        self.mark_src.clear();
        self.image_src.clear();
        self.seq = 0;
        for item in items {
            if item.source.start >= item.source.end {
                continue;
            }
            let seq = self.alloc_seq();
            match &item.body {
                Body::Mark(style) => self.mark_src.push(Mark {
                    source: item.source.clone(),
                    style: style.clone(),
                    seq,
                }),
                Body::Image {
                    data_url,
                    width,
                    alt,
                    pill,
                } => self.image_src.push(Image {
                    pill: *pill,
                    source: item.source.clone(),
                    data_url: data_url.clone(),
                    width: *width,
                    alt: alt.clone(),
                    seq,
                }),
            }
        }
    }

    fn unpill_orphan_images(&mut self) {
        let live: Vec<PillId> = self
            .image_src
            .iter()
            .filter_map(|image| image.pill)
            .collect();
        for raw in &mut self.raws {
            let Some(pill) = &raw.pill else { continue };
            if pill.kind != ChipKind::Image || live.contains(&pill.id) {
                continue;
            }
            let (decoded, steps) = literal_map(&raw.source);
            if decoded == raw.display {
                raw.steps = steps;
                raw.pill = None;
            } else {
                let display = raw.display.clone();
                *raw = literal_from_display(&display);
            }
        }
    }
}

struct SpanItem {
    source: Range<usize>,
    body: Body,
}

enum Body {
    Mark(EditorStyle),
    Image {
        data_url: String,
        width: u32,
        alt: String,
        pill: Option<PillId>,
    },
}

impl SpanItem {
    fn kind_matches(&self, change: &FormatChange) -> bool {
        matches!(
            (&self.body, change),
            (_, FormatChange::Clear)
                | (Body::Mark(EditorStyle::Bold), FormatChange::Bold)
                | (Body::Mark(EditorStyle::Italic), FormatChange::Italic)
                | (Body::Mark(EditorStyle::Underline), FormatChange::Underline)
                | (Body::Mark(EditorStyle::Link { .. }), FormatChange::Link(_))
        )
    }

    fn same_as(&self, change: &FormatChange) -> bool {
        match (&self.body, change) {
            (Body::Mark(EditorStyle::Bold), FormatChange::Bold) => true,
            (Body::Mark(EditorStyle::Italic), FormatChange::Italic) => true,
            (Body::Mark(EditorStyle::Underline), FormatChange::Underline) => true,
            (Body::Mark(EditorStyle::Link { url }), FormatChange::Link(want)) => url == want,
            _ => false,
        }
    }

    fn clipped(&self, source: Range<usize>) -> Self {
        Self {
            source,
            body: match &self.body {
                Body::Mark(style) => Body::Mark(style.clone()),
                Body::Image {
                    data_url,
                    width,
                    alt,
                    ..
                } => Body::Image {
                    data_url: data_url.clone(),
                    width: *width,
                    alt: alt.clone(),
                    pill: None,
                },
            },
        }
    }

    fn fresh(change: &FormatChange, source: Range<usize>) -> Option<Self> {
        let body = match change {
            FormatChange::Bold => Body::Mark(EditorStyle::Bold),
            FormatChange::Italic => Body::Mark(EditorStyle::Italic),
            FormatChange::Underline => Body::Mark(EditorStyle::Underline),
            FormatChange::Link(url) if safe_link(url) => {
                Body::Mark(EditorStyle::Link { url: url.clone() })
            }
            FormatChange::Link(_) | FormatChange::Clear => return None,
        };
        Some(Self { source, body })
    }
}
