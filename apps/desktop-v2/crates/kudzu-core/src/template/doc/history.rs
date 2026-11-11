//! Bounded document undo. Image bytes are shared by every snapshot.
use super::*;
impl EditorDoc {
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

    pub(super) fn transaction(&mut self, body: impl FnOnce(&mut Self)) -> bool {
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
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    fn snap(&self) -> Snapshot {
        Snapshot {
            formatted: self.formatted,
            raws: self.raws.clone(),
            marks: self.mark_src.clone(),
            images: self.image_src.clone(),
            seq: self.seq,
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.formatted = snap.formatted;
        self.raws = snap.raws;
        self.mark_src = snap.marks;
        self.image_src = snap.images;
        self.seq = snap.seq;
        self.install();
    }

    fn same(&self, snap: &Snapshot) -> bool {
        self.formatted == snap.formatted
            && load::raws_eq(&self.raws, &snap.raws)
            && load::marks_eq(&self.mark_src, &snap.marks)
            && load::images_eq(&self.image_src, &snap.images)
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn two_thousand_edits_share_512_kib_of_images_and_bound_history() {
        let source = "abc";
        let sizes = [174_762, 174_762, 174_762];
        let formatting = Formatting {
            source: source.into(),
            spans: sizes
                .iter()
                .enumerate()
                .map(|(i, &size)| {
                    let mut url = "data:image/png;base64,iVBORw0KGgo".to_string();
                    // PNG base64 length must be divisible by four.
                    url.extend(std::iter::repeat_n('A', size - url.len()));
                    while !(url.len() - 22).is_multiple_of(4) {
                        url.push('A');
                    }
                    FormatSpan {
                        start: i,
                        end: i + 1,
                        style: FormatStyle::Image {
                            data_url: url,
                            width: 32,
                        },
                    }
                })
                .collect(),
        };
        let mut doc = EditorDoc::from_template(source, Some(&formatting));
        assert_eq!(doc.images().len(), 3);
        for i in 0..2000 {
            doc.replace(doc.text().len(), doc.text().len(), "x");
            let mut allocations = HashMap::new();
            for image in doc
                .image_src
                .iter()
                .chain(doc.undo.iter().flat_map(|s| &s.images))
                .chain(doc.redo.iter().flat_map(|s| &s.images))
            {
                allocations.insert(image.data_url.as_ptr(), image.data_url.len());
            }
            assert!(
                allocations.values().sum::<usize>() < 600 * 1024,
                "image bytes were copied at edit {i}"
            );
            let raw_bytes = |raws: &[Raw]| {
                raws.iter()
                    .map(|r| {
                        r.source.capacity()
                            + r.display.capacity()
                            + r.steps.capacity() * std::mem::size_of::<(usize, usize)>()
                    })
                    .sum::<usize>()
            };
            let retained = raw_bytes(&doc.raws)
                + doc
                    .undo
                    .iter()
                    .chain(&doc.redo)
                    .map(|s| raw_bytes(&s.raws))
                    .sum::<usize>()
                + allocations.values().sum::<usize>();
            assert!(
                retained < 16 * 1024 * 1024,
                "retained document buffers: {retained} at edit {i}"
            );
            assert!(doc.undo.len() + doc.redo.len() <= 200);
        }
        let mut undone = 0;
        while doc.undo() {
            undone += 1;
        }
        assert_eq!(undone, 200);
        assert_eq!(doc.text().len(), 1803);
        for _ in 0..200 {
            assert!(doc.redo());
        }
        assert!(!doc.redo());
    }

    #[test]
    fn span_equality_includes_cross_style_order() {
        let mut doc = EditorDoc::from_template("abc", None);
        doc.change_format(0, 3, FormatChange::Bold);
        doc.change_format(0, 3, FormatChange::Italic);
        let before = doc.snap();
        doc.mark_src[0].seq = 1;
        doc.mark_src[1].seq = 0;
        assert!(!doc.same(&before));
    }
}
