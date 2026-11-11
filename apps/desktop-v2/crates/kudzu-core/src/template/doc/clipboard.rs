//! Clipboard representation of a selection.
//!
//! [`EditorDoc::copy`](super::EditorDoc::copy) turns a display selection into template source
//! plus a rich payload. [`EditorDoc::paste_own`](super::EditorDoc::paste_own) rebuilds chips
//! from that payload. [`EditorDoc::paste_source`](super::EditorDoc::paste_source) parses plain
//! text from another app as template source, so `{{tags}}` become chips. Rune's HTML stays
//! in S52; this module only sees the structured payload.

use std::ops::Range;

use super::edit::{clip_rebase, concat_source, single_tag, slice_display, snap_range};
use super::load::{literal_from_display, literal_map, place_image_pill, utf16_len};
use super::{
    CopiedChip, CopiedImage, CopiedMark, CopiedRun, CopyPayload, EditorDoc, Image, Mark, PillId,
    Raw, RawPill,
};
use crate::template::chips::{self, PillKind as ChipKind};

impl EditorDoc {
    /// The template source of a selection, and the rich payload that pastes the chips back.
    ///
    /// The range grows over any chip it touches. `plain` is what another app should receive.
    /// `None` when the selection is empty.
    pub fn copy(&self, start: usize, end: usize) -> Option<CopyPayload> {
        let (start, end) = snap_range(self, start, end);
        if start == end {
            return None;
        }
        let abs = self.display_to_source(start, false);
        let abs_end = self.display_to_source(end, true);
        let mut plain = String::new();
        let mut runs = Vec::new();
        for (index, piece) in self.map.iter().enumerate() {
            if piece.display.end <= start || piece.display.start >= end {
                continue;
            }
            let raw = &self.raws[index];
            let (display, source, chip) = if let Some(pill) = &raw.pill {
                let chip = (pill.kind == ChipKind::Tag).then(|| CopiedChip {
                    id: pill.id,
                    tone: pill.tone,
                    kind: pill.kind,
                    html_id: self
                        .html_id(pill.id)
                        .unwrap_or_else(|| chips::html_pill_id(pill.tone, &raw.source, 1)),
                });
                (raw.display.clone(), raw.source.clone(), chip)
            } else {
                let from = start.max(piece.display.start) - piece.display.start;
                let to = end.min(piece.display.end) - piece.display.start;
                let sliced = slice_display(raw, from, to);
                (sliced.display, sliced.source, None)
            };
            plain.push_str(&source);
            runs.push(CopiedRun {
                display,
                source,
                chip,
            });
        }
        let marks = self
            .mark_src
            .iter()
            .filter_map(|mark| {
                clip_rebase(&mark.source, abs, abs_end).map(|source| CopiedMark {
                    source,
                    style: mark.style.clone(),
                })
            })
            .collect();
        let images = self
            .image_src
            .iter()
            .filter_map(|image| {
                clip_rebase(&image.source, abs, abs_end).map(|source| CopiedImage {
                    source,
                    data_url: image.data_url.clone(),
                    width: image.width,
                    alt: image.alt.clone(),
                })
            })
            .collect();
        Some(CopyPayload {
            plain,
            runs,
            marks,
            images,
        })
    }

    /// Paste a payload from [`Self::copy`]. Chips come back as chips with new [`PillId`]s.
    /// One undo step. False when a chip id is not one of ours.
    pub fn paste_own(
        &mut self,
        start: usize,
        end: usize,
        payload: &CopyPayload,
        snippet_name: &dyn Fn(&str) -> Option<&str>,
    ) -> bool {
        if !payload_is_own(payload) {
            return false;
        }
        let plain = payload.plain.clone();
        let marks = payload.marks.clone();
        let images = payload.images.clone();
        let runs = payload.runs.clone();
        self.transaction(|doc| {
            let inserted = runs_to_raws(&runs, snippet_name, &mut doc.next_pill);
            let region = doc.splice(start, end, inserted);
            doc.stamp_payload(region, &plain, &marks, &images);
        })
    }

    /// Paste template source from another app. `{{tags}}` become chips. Literals keep their
    /// escapes. One undo step. This is the CodeMirror behaviour, where the document is the source.
    pub fn paste_source(
        &mut self,
        start: usize,
        end: usize,
        source: &str,
        snippet_name: &dyn Fn(&str) -> Option<&str>,
    ) -> bool {
        self.transaction(|doc| {
            let inserted = pieces_owned(source, snippet_name, &mut doc.next_pill);
            doc.splice(start, end, inserted);
        })
    }

    fn stamp_payload(
        &mut self,
        region: Range<usize>,
        plain: &str,
        marks: &[CopiedMark],
        images: &[CopiedImage],
    ) {
        let slice = slice_source_units(
            &concat_source(&self.raws),
            region.start,
            region.start + utf16_len(plain),
        );
        if slice != plain {
            return;
        }
        for mark in marks {
            let source = region.start + mark.source.start..region.start + mark.source.end;
            if source.end - region.start > utf16_len(plain) {
                continue;
            }
            let seq = self.alloc_seq();
            self.mark_src.push(Mark {
                source,
                style: mark.style.clone(),
                seq,
            });
        }
        for image in images {
            let source = region.start + image.source.start..region.start + image.source.end;
            let seq = self.alloc_seq();
            if let Some(id) = place_image_pill(
                &mut self.raws,
                source.start,
                source.end,
                &mut self.next_pill,
            ) {
                self.image_src.push(Image {
                    pill: Some(id),
                    source: source.clone(),
                    data_url: image.data_url.clone(),
                    width: image.width,
                    alt: image.alt.clone(),
                    seq,
                });
            } else {
                self.image_src.push(Image {
                    pill: None,
                    source,
                    data_url: image.data_url.clone(),
                    width: image.width,
                    alt: image.alt.clone(),
                    seq,
                });
            }
        }
        self.install();
    }
}

fn payload_is_own(payload: &CopyPayload) -> bool {
    let mut plain = String::new();
    for run in &payload.runs {
        if let Some(chip) = &run.chip {
            if chip.kind != ChipKind::Tag {
                return false;
            }
            match chips::parse_html_pill_id(&chip.html_id) {
                Some((tone, source)) if tone == chip.tone && source == run.source => {}
                _ => return false,
            }
        }
        plain.push_str(&run.source);
    }
    plain == payload.plain
}

fn runs_to_raws(
    runs: &[CopiedRun],
    snippet_name: &dyn Fn(&str) -> Option<&str>,
    next_pill: &mut u64,
) -> Vec<Raw> {
    runs.iter()
        .map(|run| {
            if let Some(chip) = &run.chip {
                if let Some(tag) = single_tag(&run.source) {
                    let id = PillId(*next_pill);
                    *next_pill += 1;
                    return Raw {
                        display: chips::chip_label(&tag, &run.source, snippet_name),
                        source: run.source.clone(),
                        pill: Some(RawPill {
                            id,
                            tone: chip.tone,
                            kind: ChipKind::Tag,
                        }),
                        steps: Vec::new(),
                    };
                }
            }
            let (decoded, steps) = literal_map(&run.source);
            if decoded == run.display {
                Raw {
                    display: run.display.clone(),
                    source: run.source.clone(),
                    pill: None,
                    steps,
                }
            } else {
                literal_from_display(&run.display)
            }
        })
        .collect()
}

fn pieces_owned(
    source: &str,
    snippet_name: &dyn Fn(&str) -> Option<&str>,
    next_pill: &mut u64,
) -> Vec<Raw> {
    super::load::pieces_from_parse(source, snippet_name, next_pill)
        .into_iter()
        .filter(|raw| !(raw.display.is_empty() && raw.source.is_empty() && raw.pill.is_none()))
        .collect()
}

fn slice_source_units(source: &str, start: usize, end: usize) -> String {
    super::slice_utf16(source, start..end)
}
