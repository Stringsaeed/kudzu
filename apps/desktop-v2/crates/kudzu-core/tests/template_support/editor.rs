//! Independent display-string oracle for sequences of editor operations.
use crate::template_support::{text, SplitMix64};
use kudzu_core::formatting::Formatting;
use kudzu_core::template::{
    self,
    doc::{EditorDoc, EditorMark, FormatChange, PillId},
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

pub fn random_editor_text(rng: &mut SplitMix64) -> String {
    const PARTS: &[&str] = &[
        "a",
        "xx",
        "abc",
        "é",
        "e\u{301}",
        "👩\u{200d}💻",
        "\u{301}",
        "\u{200d}",
        "\r\n",
        "العربية",
        "{",
        "{{",
        "\\",
        "\\{{",
        " ",
        "\n",
        "}",
    ];
    (0..1 + rng.below(5)).map(|_| *rng.pick(PARTS)).collect()
}

#[derive(Clone)]
struct State {
    text: String,
    saved: (String, Option<Formatting>),
    pills: Vec<PillId>,
}
struct Evidence {
    prefix: String,
    prefix_at: usize,
    suffix: String,
    suffix_at: usize,
    marks: Vec<EditorMark>,
}
impl Evidence {
    fn before(doc: &EditorDoc, range: &Range<usize>) -> Self {
        // Escape repair may change the cut step and its neighbors. Everything beyond those
        // boundaries must remain byte-exact, even noncanonical spellings such as C:\Users.
        let clusters: Vec<_> = doc
            .text()
            .grapheme_indices(true)
            .map(|(at, text)| (at, at + text.len(), text))
            .collect();
        let mut prefix_at = clusters
            .iter()
            .find(|&&(a, b, _)| a < range.start && range.start < b)
            .map_or(range.start, |c| c.0);
        let mut suffix_at = clusters
            .iter()
            .find(|&&(a, b, _)| a < range.end && range.end < b)
            .map_or(range.end, |c| c.1);
        while let Some(&(a, _, _)) = clusters
            .iter()
            .find(|&&(_, b, g)| b == prefix_at && g.starts_with(['\\', '{']))
        {
            prefix_at = a;
        }
        while let Some(&(_, b, _)) = clusters
            .iter()
            .find(|&&(a, _, g)| a == suffix_at && g.starts_with(['\\', '{']))
        {
            suffix_at = b;
        }
        let source = doc.to_template().0;
        Self {
            prefix: units(&source, 0..doc.display_to_source(prefix_at, false)),
            prefix_at,
            suffix: units(
                &source,
                doc.display_to_source(suffix_at, true)..source.encode_utf16().count(),
            ),
            suffix_at,
            marks: doc
                .marks()
                .iter()
                .filter(|m| m.range.end < range.start || m.range.start > range.end)
                .cloned()
                .collect(),
        }
    }
    fn after(self, doc: &EditorDoc, range: &Range<usize>, inserted: usize) {
        let source = doc.to_template().0;
        assert_eq!(
            units(&source, 0..doc.display_to_source(self.prefix_at, false)),
            self.prefix,
            "untouched prefix"
        );
        let suffix_at = self.suffix_at - range.len() + inserted;
        assert_eq!(
            units(
                &source,
                doc.display_to_source(suffix_at, true)..source.encode_utf16().count()
            ),
            self.suffix,
            "untouched suffix"
        );
        for mut mark in self.marks {
            if mark.range.start > range.end {
                mark.range = mark.range.start - range.len() + inserted
                    ..mark.range.end - range.len() + inserted;
            }
            assert!(
                doc.marks()
                    .iter()
                    .any(|m| m.style == mark.style && m.range == mark.range),
                "untouched mark {:?} in {:?}",
                mark,
                doc.marks()
            );
        }
    }
}
fn units(s: &str, range: Range<usize>) -> String {
    String::from_utf16(
        &s.encode_utf16()
            .skip(range.start)
            .take(range.len())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn selection(doc: &EditorDoc, model: &str, rng: &mut SplitMix64) -> Range<usize> {
    let boundaries: Vec<_> = model
        .char_indices()
        .map(|(i, _)| i)
        .chain([model.len()])
        .filter(|i| {
            !doc.pills()
                .iter()
                .any(|p| p.range.start < *i && *i < p.range.end)
        })
        .collect();
    let a = *rng.pick(&boundaries);
    let b = if rng.bool() {
        a
    } else {
        *rng.pick(&boundaries)
    };
    a.min(b)..a.max(b)
}

pub fn exercise(seed: u64, cases: usize) {
    let mut rng = SplitMix64(seed);
    for case in 0..cases {
        let body = random_editor_text(&mut rng);
        let source = format!(
            "C:\\Users safe text {} safe tail \\q",
            template::serialize(&[text(&body)])
        );
        let mut doc = EditorDoc::from_template(&source, None);
        let mut model = format!("C:\\Users safe text {body} safe tail \\q");
        let mut undo: Vec<State> = Vec::new();
        let mut redo: Vec<State> = Vec::new();
        for step in 0..32 {
            let before = State {
                text: model.clone(),
                saved: doc.to_template(),
                pills: doc.pills().iter().map(|p| p.id).collect(),
            };
            let operation = rng.below(10);
            if operation == 8 || operation == 9 {
                let (stack, other) = if operation == 8 {
                    (&mut undo, &mut redo)
                } else {
                    (&mut redo, &mut undo)
                };
                let changed = if operation == 8 {
                    doc.undo()
                } else {
                    doc.redo()
                };
                assert_eq!(changed, !stack.is_empty());
                if let Some(state) = stack.pop() {
                    model = state.text;
                    assert_eq!(doc.to_template(), state.saved, "history {case}:{step}");
                    assert_eq!(
                        doc.pills().iter().map(|p| p.id).collect::<Vec<_>>(),
                        state.pills
                    );
                    other.push(before);
                }
            } else {
                let range = selection(&doc, &model, &mut rng);
                if operation == 6 {
                    let style = rng
                        .pick(&[
                            FormatChange::Bold,
                            FormatChange::Italic,
                            FormatChange::Underline,
                            FormatChange::Clear,
                        ])
                        .clone();
                    let untouched: Vec<_> = doc
                        .marks()
                        .iter()
                        .filter(|m| m.range.end <= range.start || m.range.start >= range.end)
                        .cloned()
                        .collect();
                    doc.change_format(range.start, range.end, style);
                    assert_eq!(doc.to_template().0, before.saved.0);
                    for mark in untouched {
                        assert!(doc.marks().contains(&mark));
                    }
                } else if operation == 7 {
                    // Independent all-lines toggle; skip line starts inside atomic pills.
                    let ordered = rng.bool();
                    let mut starts = Vec::new();
                    let mut at = 0;
                    for line in model.split('\n') {
                        if at == model.len() && !model.is_empty() {
                            break;
                        }
                        let count = line.bytes().take_while(u8::is_ascii_digit).count();
                        let numbered = count > 0 && line[count..].starts_with(". ");
                        let bullet = line.starts_with("• ");
                        if !doc
                            .pills()
                            .iter()
                            .any(|p| p.range.start < at && at < p.range.end)
                        {
                            starts.push((
                                at,
                                if bullet {
                                    4
                                } else if numbered {
                                    count + 2
                                } else {
                                    0
                                },
                                if ordered { numbered } else { bullet },
                            ));
                        }
                        at += line.len() + 1;
                    }
                    let remove = starts.iter().all(|s| s.2);
                    let mut expected_source: Vec<u16> = before.saved.0.encode_utf16().collect();
                    let mut untouched: Vec<_> = doc
                        .marks()
                        .iter()
                        .filter(|m| {
                            starts
                                .iter()
                                .all(|&(at, len, _)| m.range.end < at || m.range.start > at + len)
                        })
                        .cloned()
                        .collect();
                    for (i, &(at, len, _)) in starts.iter().enumerate().rev() {
                        let prefix = if remove {
                            String::new()
                        } else if ordered {
                            format!("{}. ", i + 1)
                        } else {
                            "• ".into()
                        };
                        expected_source.splice(
                            doc.display_to_source(at, false)..doc.display_to_source(at + len, true),
                            prefix.encode_utf16(),
                        );
                        for mark in &mut untouched {
                            if mark.range.start > at + len {
                                mark.range = mark.range.start - len + prefix.len()
                                    ..mark.range.end - len + prefix.len();
                            }
                        }
                        model.replace_range(at..at + len, &prefix);
                    }
                    doc.toggle_list(0, doc.text().len(), ordered);
                    assert_eq!(
                        doc.to_template().0,
                        String::from_utf16(&expected_source).unwrap(),
                        "list source"
                    );
                    for mark in untouched {
                        assert!(doc
                            .marks()
                            .iter()
                            .any(|m| m.style == mark.style && m.range == mark.range));
                    }
                } else {
                    let evidence = Evidence::before(&doc, &range);
                    let insert = match operation {
                        0..=2 => {
                            let insert = if operation == 0 {
                                String::new()
                            } else {
                                random_editor_text(&mut rng)
                            };
                            doc.replace(range.start, range.end, &insert);
                            insert
                        }
                        3 => {
                            doc.paste_source(range.start, range.end, "{{date}}", &|_| None);
                            "Date · ISO date".into()
                        }
                        4 => {
                            let insert = random_editor_text(&mut rng);
                            let copy = EditorDoc::from_template(
                                &template::serialize(&[text(&insert)]),
                                None,
                            );
                            let payload = copy.copy(0, copy.text().len()).unwrap();
                            doc.paste_own(range.start, range.end, &payload, &|_| None);
                            insert
                        }
                        _ => {
                            let insert = random_editor_text(&mut rng);
                            doc.paste_source(
                                range.start,
                                range.end,
                                &template::serialize(&[text(&insert)]),
                                &|_| None,
                            );
                            insert
                        }
                    };
                    model.replace_range(range.clone(), &insert);
                    evidence.after(&doc, &range, insert.len());
                }
                if before.saved != doc.to_template()
                    || before.pills != doc.pills().iter().map(|p| p.id).collect::<Vec<_>>()
                {
                    undo.push(before);
                    redo.clear();
                }
            }
            assert_eq!(
                doc.text(),
                model,
                "seed {seed}, case {case}, step {step}, op {operation}"
            );
            let (source, formatting) = doc.to_template();
            let loaded = EditorDoc::from_template(&source, formatting.as_ref());
            assert_eq!(loaded.text(), model, "reload {case}:{step}: {source:?}");
            assert_eq!(
                loaded.to_template(),
                (source, formatting),
                "save {case}:{step}"
            );
        }
    }
}
