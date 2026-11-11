//! Clipboard representation of a selection.
//!
//! [`EditorDoc::copy`](super::EditorDoc::copy) turns a display selection into template source
//! plus a rich payload. [`EditorDoc::paste_own`](super::EditorDoc::paste_own) rebuilds chips
//! from that payload. [`EditorDoc::paste_source`](super::EditorDoc::paste_source) parses plain
//! text from another app as template source, so `{{tags}}` become chips. Rune's HTML stays
//! in S52; this module only sees the structured payload.

use std::ops::Range;

use super::edit::{single_tag, snap_range};
use super::literal::slice_display;
use super::load::{literal_from_display, literal_map, place_image_pill, utf16_len};
use super::mapping::{copied_range, pasted_range};
use super::{
    CopiedChip, CopiedImage, CopiedMark, CopiedRun, CopyPayload, EditorDoc, Image, Mark, PillId,
    Raw, RawPill,
};
use crate::template::chips::{self, PillKind as ChipKind, SnippetNames};

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
                        .unwrap_or_else(|| chips::html_pill_id(pill.tone, &raw.source)),
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
        let mut fragment = EditorDoc::from_template("", None);
        fragment.raws = runs
            .iter()
            .map(|run| Raw {
                display: run.display.clone(),
                source: run.source.clone(),
                pill: run.chip.as_ref().map(|p| RawPill {
                    id: p.id,
                    tone: p.tone,
                    kind: p.kind,
                }),
                steps: if run.chip.is_some() {
                    Vec::new()
                } else {
                    literal_map(&run.source).1
                },
            })
            .collect();
        fragment.install();
        let marks = self
            .mark_src
            .iter()
            .filter_map(|mark| {
                copied_range(self, &fragment, &mark.source, start, end).map(|source| CopiedMark {
                    source,
                    style: mark.style.clone(),
                })
            })
            .collect();
        let images = self
            .image_src
            .iter()
            .filter_map(|image| {
                copied_range(self, &fragment, &image.source, start, end).map(|source| CopiedImage {
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
        snippet_name: SnippetNames<'_>,
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
            doc.stamp_payload(region.new.start, &plain, &marks, &images, snippet_name);
        })
    }

    /// Paste template source from another app. `{{tags}}` become chips. Literals keep their
    /// escapes. One undo step. This is the CodeMirror behaviour, where the document is the source.
    pub fn paste_source(
        &mut self,
        start: usize,
        end: usize,
        source: &str,
        snippet_name: SnippetNames<'_>,
    ) -> bool {
        self.transaction(|doc| {
            let inserted = pieces_owned(source, snippet_name, &mut doc.next_pill);
            doc.splice(start, end, inserted);
        })
    }

    fn stamp_payload(
        &mut self,
        start: usize,
        plain: &str,
        marks: &[CopiedMark],
        images: &[CopiedImage],
        names: SnippetNames<'_>,
    ) {
        let fragment = EditorDoc::from_template_named(plain, None, names);
        if !marks.is_empty() || !images.is_empty() {
            self.formatted = true;
        }
        for mark in marks {
            let source = pasted_range(&fragment, self, &mark.source, start);
            if mark.source.end > utf16_len(plain) || source.start >= source.end {
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
            let source = pasted_range(&fragment, self, &image.source, start);
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
                Some((tone, source))
                    if tone == chip.tone
                        && source == run.source
                        && single_tag(&source)
                            .is_some_and(|tag| chips::chip_tone(&tag) == tone) => {}
                _ => return false,
            }
        }
        plain.push_str(&run.source);
    }
    plain == payload.plain
}

fn runs_to_raws(
    runs: &[CopiedRun],
    snippet_name: SnippetNames<'_>,
    next_pill: &mut u64,
) -> Vec<Raw> {
    runs.iter()
        .map(|run| {
            if run.chip.is_some() {
                if let Some(tag) = single_tag(&run.source) {
                    let id = PillId(*next_pill);
                    *next_pill += 1;
                    return Raw {
                        display: chips::normalize_label(&chips::chip_label(
                            &tag,
                            &run.source,
                            snippet_name,
                        )),
                        source: run.source.clone(),
                        pill: Some(RawPill {
                            id,
                            tone: chips::chip_tone(&tag),
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

fn pieces_owned(source: &str, snippet_name: SnippetNames<'_>, next_pill: &mut u64) -> Vec<Raw> {
    super::load::pieces_from_parse(source, snippet_name, next_pill)
        .into_iter()
        .filter(|raw| !(raw.display.is_empty() && raw.source.is_empty() && raw.pill.is_none()))
        .collect()
}

impl CopyPayload {
    /// Decode Rune's rich fragment without depending on Rune. Ranges are ordered display
    /// bytes; ids are `{tone}:{source}`. Reject foreign/mismatched pills or plain text.
    /// S52 supplies the fragment's text and pill marks after parsing its HTML with Rune.
    pub fn from_fragment(
        plain: &str,
        pills: &[(Range<usize>, String)],
        display: &str,
    ) -> Option<Self> {
        let parsed = EditorDoc::from_template(plain, None);
        let mut at = 0;
        let mut index = 0;
        let mut runs = Vec::new();
        for raw in &parsed.raws {
            let (text, chip) = if let Some(pill) = &raw.pill {
                let (range, html_id) = pills.get(index)?;
                let (tone, source) = chips::parse_html_pill_id(html_id)?;
                if range.start != at || tone != pill.tone || source != raw.source {
                    return None;
                }
                let text = display.get(range.clone())?;
                if text.is_empty() || chips::normalize_label(text) != text {
                    return None;
                }
                index += 1;
                (
                    text.to_string(),
                    Some(CopiedChip {
                        id: pill.id,
                        tone,
                        kind: ChipKind::Tag,
                        html_id: html_id.clone(),
                    }),
                )
            } else {
                if !display.get(at..)?.starts_with(&raw.display) {
                    return None;
                }
                (raw.display.clone(), None)
            };
            at += text.len();
            runs.push(CopiedRun {
                display: text,
                source: raw.source.clone(),
                chip,
            });
        }
        (at == display.len() && index == pills.len()).then(|| Self {
            plain: plain.into(),
            runs,
            marks: Vec::new(),
            images: Vec::new(),
        })
    }
}
