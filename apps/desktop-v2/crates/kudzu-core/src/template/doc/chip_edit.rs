//! Tag insertion, configuration, and display-only refreshes of nested snippet names.
use super::edit::single_tag;
use super::{EditorDoc, PillId, Raw, Snapshot};
use crate::template::chips::{self, PillKind, SnippetNames};
use std::ops::Range;

impl EditorDoc {
    /// Insert one exact tag at a caret, in one undo step.
    pub fn insert_chip(
        &mut self,
        at: usize,
        source: &str,
        names: SnippetNames<'_>,
    ) -> Option<PillId> {
        self.replace_with_chip(at..at, source, names)
    }

    /// Replace a slash query or selection with one tag, in one undo step.
    pub fn replace_with_chip(
        &mut self,
        range: Range<usize>,
        source: &str,
        names: SnippetNames<'_>,
    ) -> Option<PillId> {
        let tag = single_tag(source)?;
        let mut id = None;
        self.transaction(|doc| {
            let pill = doc.alloc_pill(chips::chip_tone(&tag), PillKind::Tag);
            id = Some(pill.id);
            doc.splice(
                range.start,
                range.end,
                vec![Raw {
                    display: chips::normalize_label(&chips::chip_label(&tag, source, names)),
                    source: source.into(),
                    pill: Some(pill),
                    steps: Vec::new(),
                }],
            );
        });
        id
    }

    /// Configure an existing tag. Preserve its identity and covering marks; one undo step.
    /// Invalid source, an image, a missing id, or an unchanged tag returns false.
    pub fn set_chip_source(&mut self, id: PillId, source: &str, names: SnippetNames<'_>) -> bool {
        let Some(tag) = single_tag(source) else {
            return false;
        };
        let Some(pill) = self
            .pills
            .iter()
            .find(|p| p.id == id && p.kind == PillKind::Tag)
        else {
            return false;
        };
        if pill.source == source {
            return false;
        }
        let range = pill.range.clone();
        let raw = Raw {
            display: chips::normalize_label(&chips::chip_label(&tag, source, names)),
            source: source.into(),
            pill: Some(super::RawPill {
                id,
                tone: chips::chip_tone(&tag),
                kind: PillKind::Tag,
            }),
            steps: Vec::new(),
        };
        self.transaction(|doc| {
            doc.splice(range.start, range.end, vec![raw]);
        })
    }

    /// Refresh labels after library renames. Source, formatting, identities and history stay.
    /// Refresh snapshots too, so undo cannot resurrect a stale library name.
    pub fn relabel(&mut self, names: SnippetNames<'_>) {
        relabel_raws(&mut self.raws, names);
        for Snapshot { raws, .. } in self.undo.iter_mut().chain(&mut self.redo) {
            relabel_raws(raws, names);
        }
        self.install();
    }
}
fn relabel_raws(raws: &mut [Raw], names: SnippetNames<'_>) {
    for raw in raws {
        if raw.pill.as_ref().is_some_and(|p| p.kind == PillKind::Tag) {
            if let Some(tag) = single_tag(&raw.source) {
                raw.display = chips::normalize_label(&chips::chip_label(&tag, &raw.source, names));
            }
        }
    }
}
