//! Prototype codec between a template source and the text a rich field shows.
//!
//! `text` is the display string (literals plus chip labels). Each piece keeps the
//! exact source slice, so `to_template` splices the original bytes back: recognised
//! tags stay as written, and unknown or verbatim tags stay byte-exact. Formatting
//! spans stay UTF-16 offsets into that source.

use std::ops::Range;

use kudzu_core::formatting::{safe_image, FormatSpan, FormatStyle, Formatting};
use kudzu_core::template::{parse, Node, Tag};
use unicode_segmentation::UnicodeSegmentation;

use crate::labels::{self, Tone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillKind {
    Tag,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorStyle {
    Bold,
    Italic,
    Underline,
    Link { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPill {
    /// UTF-8 byte range of `label` inside [`EditorDoc::text`].
    pub range: Range<usize>,
    pub id: String,
    /// Exact template slice this chip replaces on the way back out.
    pub source: String,
    pub label: String,
    pub tone: Tone,
    pub kind: PillKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMark {
    /// UTF-8 byte range in [`EditorDoc::text`]. A span that cuts a chip grows to the whole chip.
    pub range: Range<usize>,
    pub style: EditorStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorImage {
    pub pill: usize,
    pub data_url: String,
    pub width: u32,
    pub alt: String,
}

/// One slice of the display, in order. Concatenating `source` rebuilds the template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePiece {
    pub display: Range<usize>,
    pub source: String,
    pub source_utf16: Range<usize>,
    pub pill: Option<usize>,
    /// Literal grapheme steps: `(display byte, source utf-16)`, both from the piece start.
    /// Pills leave this empty and map as a whole.
    pub steps: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorDoc {
    pub text: String,
    pub pills: Vec<EditorPill>,
    pub marks: Vec<EditorMark>,
    pub images: Vec<EditorImage>,
    pub map: Vec<SourcePiece>,
}

struct Raw {
    display: String,
    source: String,
    source_utf16: Range<usize>,
    pill: Option<RawPill>,
    steps: Vec<(usize, usize)>,
}

struct RawPill {
    tone: Tone,
    kind: PillKind,
    data_url: Option<String>,
    width: u32,
    alt: String,
}

pub fn from_template(source: &str, formatting: Option<&Formatting>) -> EditorDoc {
    from_template_named(source, formatting, &|_| None)
}

/// [`from_template`] with the name lookup a nested `{{snippet}}` chip needs.
/// Without a name the label is `↳ Missing snippet`.
pub fn from_template_named(
    source: &str,
    formatting: Option<&Formatting>,
    snippet_name: &dyn Fn(&str) -> Option<&str>,
) -> EditorDoc {
    let mut raws = pieces_from_parse(source, snippet_name);
    if formatting.is_none_or(|f| f.source.is_empty() || f.source == source) {
        if let Some(formatting) = formatting {
            for span in &formatting.spans {
                if let FormatStyle::Image { data_url, width } = &span.style {
                    if safe_image(data_url) {
                        apply_image(&mut raws, span.start, span.end, data_url, *width);
                    }
                }
            }
        }
    }
    let mut doc = layout(raws);
    if formatting.is_none_or(|f| f.source.is_empty() || f.source == source) {
        if let Some(formatting) = formatting {
            for span in &formatting.spans {
                if let Some(style) = editor_style(&span.style) {
                    let range = display_of_source(&doc.map, span.start, span.end);
                    if range.start < range.end {
                        doc.marks.push(EditorMark { range, style });
                    }
                }
            }
        }
    }
    doc
}

impl EditorDoc {
    pub fn to_template(&self) -> (String, Option<Formatting>) {
        let source: String = self.map.iter().map(|p| p.source.as_str()).collect();
        let mut spans = Vec::new();
        for mark in &self.marks {
            let range = source_utf16_of(self, mark.range.clone());
            if range.start < range.end {
                spans.push(FormatSpan {
                    start: range.start,
                    end: range.end,
                    style: format_style(&mark.style),
                });
            }
        }
        for image in &self.images {
            let Some(pill) = self.pills.get(image.pill) else {
                continue;
            };
            let range = source_utf16_of(self, pill.range.clone());
            if range.start < range.end {
                spans.push(FormatSpan {
                    start: range.start,
                    end: range.end,
                    style: FormatStyle::Image {
                        data_url: image.data_url.clone(),
                        width: image.width,
                    },
                });
            }
        }
        let formatting = (!spans.is_empty()).then(|| Formatting {
            source: source.clone(),
            spans,
        });
        (source, formatting)
    }
}

fn pieces_from_parse(source: &str, snippet_name: &dyn Fn(&str) -> Option<&str>) -> Vec<Raw> {
    let parsed = parse(source);
    let total = utf16_len(source);
    let mut raws = Vec::new();
    let mut at = 0usize;
    for node in &parsed.nodes {
        let range = node_range(node);
        if range.start > at {
            raws.push(literal_piece(source, at..range.start));
        }
        match node {
            Node::Text { text, .. } => {
                let slice = slice_utf16(source, range.clone());
                let (decoded, steps) = literal_map(&slice);
                let steps = if decoded == *text {
                    steps
                } else {
                    vec![(0, 0), (text.len(), utf16_len(&slice))]
                };
                raws.push(Raw {
                    display: text.clone(),
                    source: slice,
                    source_utf16: range.clone(),
                    pill: None,
                    steps,
                });
            }
            Node::Tag(tag) => raws.push(tag_piece(source, tag, range, snippet_name)),
        }
        at = at.max(node_range(node).end);
    }
    if at < total {
        raws.push(literal_piece(source, at..total));
    }
    raws
}

fn tag_piece(
    source: &str,
    tag: &Tag,
    range: Range<usize>,
    snippet_name: &dyn Fn(&str) -> Option<&str>,
) -> Raw {
    let slice = slice_utf16(source, range.clone());
    let (label, tone) = labels::chip_face_named(tag, &slice, snippet_name);
    Raw {
        display: label,
        source: slice,
        source_utf16: range,
        pill: Some(RawPill {
            tone,
            kind: PillKind::Tag,
            data_url: None,
            width: 0,
            alt: String::new(),
        }),
        steps: Vec::new(),
    }
}

fn literal_piece(source: &str, range: Range<usize>) -> Raw {
    let slice = slice_utf16(source, range.clone());
    let (display, steps) = literal_map(&slice);
    Raw {
        display,
        source: slice,
        source_utf16: range,
        pill: None,
        steps,
    }
}

fn apply_image(raws: &mut Vec<Raw>, mut start: usize, mut end: usize, data_url: &str, width: u32) {
    if start >= end {
        return;
    }
    for raw in raws.iter() {
        if raw.pill.is_some() && raw.source_utf16.start < end && raw.source_utf16.end > start {
            start = start.min(raw.source_utf16.start);
            end = end.max(raw.source_utf16.end);
        }
    }
    let Some(first) = raws
        .iter()
        .position(|r| r.source_utf16.end > start && r.source_utf16.start < end)
    else {
        return;
    };
    split_literal(raws, first, start);
    let first = raws
        .iter()
        .position(|r| r.source_utf16.end > start && r.source_utf16.start < end)
        .unwrap_or(first);
    let last = raws
        .iter()
        .rposition(|r| r.source_utf16.start < end && r.source_utf16.end > start)
        .unwrap_or(first);
    split_literal(raws, last, end);
    let first = raws
        .iter()
        .position(|r| r.source_utf16.end > start && r.source_utf16.start < end)
        .unwrap_or(first);
    let last = raws
        .iter()
        .rposition(|r| r.source_utf16.start < end && r.source_utf16.end > start)
        .unwrap_or(first);
    if first > last {
        return;
    }
    let merged = raws.drain(first..=last).collect::<Vec<_>>();
    let source: String = merged.iter().map(|r| r.source.as_str()).collect();
    let plain = merged.iter().all(|r| r.pill.is_none());
    let alt: String = merged.iter().map(|r| r.display.as_str()).collect();
    let label = if plain { alt.clone() } else { "Image".into() };
    let source_utf16 = merged
        .first()
        .map(|r| r.source_utf16.start)
        .unwrap_or(start)
        ..merged.last().map(|r| r.source_utf16.end).unwrap_or(end);
    raws.insert(
        first,
        Raw {
            display: label,
            source,
            source_utf16,
            pill: Some(RawPill {
                tone: Tone::Form,
                kind: PillKind::Image,
                data_url: Some(data_url.to_string()),
                width,
                alt,
            }),
            steps: Vec::new(),
        },
    );
}

/// Split the literal `index` so a new piece starts at absolute utf-16 `at`.
fn split_literal(raws: &mut Vec<Raw>, index: usize, at: usize) {
    let Some(raw) = raws.get(index) else { return };
    if raw.pill.is_some() || at <= raw.source_utf16.start || at >= raw.source_utf16.end {
        return;
    }
    let local = at - raw.source_utf16.start;
    let left_end = step_display(&raw.steps, local, false);
    let right_at = step_display(&raw.steps, local, true);
    if left_end != right_at {
        return;
    }
    let display = raw.display.clone();
    let source = raw.source.clone();
    let steps = raw.steps.clone();
    let end = raw.source_utf16.end;
    let start = raw.source_utf16.start;
    let left_src = slice_utf16(&source, 0..local);
    let right_src = slice_utf16(&source, local..utf16_len(&source));
    let left_steps: Vec<_> = steps.iter().copied().filter(|(_, s)| *s <= local).collect();
    let right_steps: Vec<_> = steps
        .iter()
        .copied()
        .filter(|(_, s)| *s >= local)
        .map(|(d, s)| (d - left_end, s - local))
        .collect();
    raws[index] = Raw {
        display: display[..left_end].to_string(),
        source: left_src,
        source_utf16: start..at,
        pill: None,
        steps: left_steps,
    };
    raws.insert(
        index + 1,
        Raw {
            display: display[right_at..].to_string(),
            source: right_src,
            source_utf16: at..end,
            pill: None,
            steps: if right_steps.is_empty() {
                vec![(0, 0)]
            } else {
                right_steps
            },
        },
    );
}

fn layout(raws: Vec<Raw>) -> EditorDoc {
    let mut text = String::new();
    let mut pills = Vec::new();
    let mut images = Vec::new();
    let mut map = Vec::new();
    for raw in raws {
        let start = text.len();
        text.push_str(&raw.display);
        let display = start..text.len();
        let pill = raw.pill.as_ref().map(|pill| {
            let index = pills.len();
            let id = format!("{}:{index}", pill.tone.name());
            if pill.kind == PillKind::Image {
                images.push(EditorImage {
                    pill: index,
                    data_url: pill.data_url.clone().unwrap_or_default(),
                    width: pill.width,
                    alt: pill.alt.clone(),
                });
            }
            pills.push(EditorPill {
                range: display.clone(),
                id,
                source: raw.source.clone(),
                label: raw.display.clone(),
                tone: pill.tone,
                kind: pill.kind,
            });
            index
        });
        map.push(SourcePiece {
            display,
            source: raw.source,
            source_utf16: raw.source_utf16,
            pill,
            steps: raw.steps,
        });
    }
    EditorDoc {
        text,
        pills,
        marks: Vec::new(),
        images,
        map,
    }
}

fn display_of_source(map: &[SourcePiece], start: usize, end: usize) -> Range<usize> {
    let mut out_start = None;
    let mut out_end = 0usize;
    for piece in map {
        if piece.source_utf16.end <= start || piece.source_utf16.start >= end {
            continue;
        }
        let range = if piece.pill.is_some() {
            piece.display.clone()
        } else {
            let local_s = start.max(piece.source_utf16.start) - piece.source_utf16.start;
            let local_e = end.min(piece.source_utf16.end) - piece.source_utf16.start;
            let ds = step_display(&piece.steps, local_s, false);
            let de = step_display(&piece.steps, local_e, true);
            piece.display.start + ds..piece.display.start + de
        };
        if out_start.is_none() {
            out_start = Some(range.start);
        }
        out_end = range.end;
    }
    out_start.unwrap_or(0)..out_end
}

fn source_utf16_of(doc: &EditorDoc, display: Range<usize>) -> Range<usize> {
    let mut start = None;
    let mut end = 0usize;
    for piece in &doc.map {
        if piece.display.end <= display.start || piece.display.start >= display.end {
            continue;
        }
        let (s, e) = if piece.pill.is_some() {
            (piece.source_utf16.start, piece.source_utf16.end)
        } else {
            let local_s = display.start.max(piece.display.start) - piece.display.start;
            let local_e = display.end.min(piece.display.end) - piece.display.start;
            (
                piece.source_utf16.start + step_source(&piece.steps, local_s, false),
                piece.source_utf16.start + step_source(&piece.steps, local_e, true),
            )
        };
        if start.is_none() {
            start = Some(s);
        }
        end = e;
    }
    start.unwrap_or(0)..end
}

fn editor_style(style: &FormatStyle) -> Option<EditorStyle> {
    Some(match style {
        FormatStyle::Bold => EditorStyle::Bold,
        FormatStyle::Italic => EditorStyle::Italic,
        FormatStyle::Underline => EditorStyle::Underline,
        FormatStyle::Link { url } => EditorStyle::Link { url: url.clone() },
        FormatStyle::Image { .. } => return None,
    })
}

fn format_style(style: &EditorStyle) -> FormatStyle {
    match style {
        EditorStyle::Bold => FormatStyle::Bold,
        EditorStyle::Italic => FormatStyle::Italic,
        EditorStyle::Underline => FormatStyle::Underline,
        EditorStyle::Link { url } => FormatStyle::Link { url: url.clone() },
    }
}

fn node_range(node: &Node) -> Range<usize> {
    match node {
        Node::Text { range, .. } => range.clone(),
        Node::Tag(tag) => tag.range.clone(),
    }
}

fn utf16_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

fn slice_utf16(s: &str, range: Range<usize>) -> String {
    let mut at = 0usize;
    let mut out = String::new();
    for ch in s.chars() {
        let n = ch.len_utf16();
        if at >= range.end {
            break;
        }
        if at >= range.start && at + n <= range.end {
            out.push(ch);
        }
        at += n;
    }
    out
}

fn literal_map(source: &str) -> (String, Vec<(usize, usize)>) {
    let graphemes: Vec<&str> = source.graphemes(true).collect();
    let mut src = 0usize;
    let mut display = String::new();
    let mut steps = vec![(0usize, 0usize)];
    let mut i = 0;
    while i < graphemes.len() {
        if graphemes[i] == "\\" && i + 1 < graphemes.len() && graphemes[i + 1] == "\\" {
            display.push('\\');
            src += utf16_len(graphemes[i]) + utf16_len(graphemes[i + 1]);
            i += 2;
            steps.push((display.len(), src));
            continue;
        }
        if graphemes[i] == "\\"
            && i + 2 < graphemes.len()
            && graphemes[i + 1] == "{"
            && graphemes[i + 2] == "{"
        {
            display.push_str("{{");
            src +=
                utf16_len(graphemes[i]) + utf16_len(graphemes[i + 1]) + utf16_len(graphemes[i + 2]);
            i += 3;
            steps.push((display.len(), src));
            continue;
        }
        display.push_str(graphemes[i]);
        src += utf16_len(graphemes[i]);
        i += 1;
        steps.push((display.len(), src));
    }
    (display, steps)
}

fn step_display(steps: &[(usize, usize)], src_local: usize, snap_end: bool) -> usize {
    if let Some((display, _)) = steps.iter().find(|(_, src)| *src == src_local) {
        return *display;
    }
    if snap_end {
        steps
            .iter()
            .find(|(_, src)| *src > src_local)
            .map(|(d, _)| *d)
            .unwrap_or(steps.last().map(|s| s.0).unwrap_or(0))
    } else {
        steps
            .iter()
            .rev()
            .find(|(_, src)| *src < src_local)
            .map(|(d, _)| *d)
            .unwrap_or(0)
    }
}

fn step_source(steps: &[(usize, usize)], display_local: usize, snap_end: bool) -> usize {
    if let Some((_, src)) = steps.iter().find(|(display, _)| *display == display_local) {
        return *src;
    }
    if snap_end {
        steps
            .iter()
            .find(|(display, _)| *display > display_local)
            .map(|(_, s)| *s)
            .unwrap_or(steps.last().map(|s| s.1).unwrap_or(0))
    } else {
        steps
            .iter()
            .rev()
            .find(|(display, _)| *display < display_local)
            .map(|(_, s)| *s)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::template::serialize;

    fn round_trip(source: &str) {
        let doc = from_template(source, None);
        let (out, formatting) = doc.to_template();
        assert_eq!(out, source, "source bytes changed");
        assert!(formatting.is_none());
        assert_eq!(
            serialize(&parse(source).nodes),
            serialize(&parse(&out).nodes)
        );
        let rebuilt: String = doc.map.iter().map(|p| p.source.as_str()).collect();
        assert_eq!(rebuilt, source);
        for pill in &doc.pills {
            assert_eq!(&doc.text[pill.range.clone()], pill.label.as_str());
        }
    }

    #[test]
    fn grammar_and_fixture_sources_round_trip_byte_exact() {
        let mut sources = grammar_sources();
        sources.extend(fixture_sources());
        sources.push("hello \\{{date}}".into());
        sources.push("Hello 😀 {{date}} world".into());
        sources.push("{{not-a-real-tag attr=1}}".into());
        assert!(
            sources.len() > 40,
            "corpus collapsed to {} sources",
            sources.len()
        );
        for source in sources {
            round_trip(&source);
        }
    }

    #[test]
    fn fixture_chip_labels_match() {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/migration/reference/fixtures/template.json"
        ))
        .unwrap();
        let cases = v["cases"].as_array().unwrap();
        assert!(cases.len() > 10);
        for case in cases {
            let source = case["source"].as_str().unwrap();
            // The recorded snippet chip used a library name. The spike has no library;
            // this one id is the fixture's lookup, and a missing name stays "Missing snippet".
            let doc = from_template_named(source, None, &|id| {
                (id == "c5af65ac-e426-4046-802b-11ccdd3a62ba").then_some("Email signature")
            });
            let chips = case["chips"].as_array().unwrap();
            assert_eq!(doc.pills.len(), chips.len(), "{source}");
            for (pill, chip) in doc.pills.iter().zip(chips) {
                assert_eq!(pill.label, chip["label"].as_str().unwrap(), "{source}");
                assert_eq!(pill.tone.name(), chip["tone"].as_str().unwrap(), "{source}");
            }
        }
    }

    #[test]
    fn a_snippet_with_no_name_reads_missing() {
        let source = "{{snippet id=\"c5af65ac-e426-4046-802b-11ccdd3a62ba\"}}";
        let doc = from_template(source, None);
        assert_eq!(doc.pills[0].label, "↳ Missing snippet");
        assert_eq!(doc.pills[0].tone.name(), "snippet");
        let Node::Tag(tag) = &parse(source).nodes[0] else {
            panic!("snippet tag")
        };
        assert_eq!(labels::chip_face(tag, source).0, "↳ Missing snippet");
    }

    #[test]
    fn eleven_date_presets_are_the_editor_list() {
        assert_eq!(labels::date_presets().len(), 11);
    }

    #[test]
    fn formatting_over_a_chip_and_an_image_round_trip() {
        let source = "Hello 😀 {{date}} world";
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6x8AAAAASUVORK5CYII=";
        let formatting = Formatting {
            source: source.into(),
            spans: vec![
                FormatSpan {
                    start: 0,
                    end: 23,
                    style: FormatStyle::Bold,
                },
                FormatSpan {
                    start: 0,
                    end: 23,
                    style: FormatStyle::Italic,
                },
                FormatSpan {
                    start: 0,
                    end: 23,
                    style: FormatStyle::Underline,
                },
                FormatSpan {
                    start: 0,
                    end: 23,
                    style: FormatStyle::Link {
                        url: "https://example.com/edited".into(),
                    },
                },
            ],
        };
        let doc = from_template(source, Some(&formatting));
        assert!(doc
            .pills
            .iter()
            .any(|p| p.kind == PillKind::Tag && p.label == "Date \u{00b7} ISO date"));
        let (out, back) = doc.to_template();
        assert_eq!(out, source);
        let back = back.unwrap();
        assert_eq!(sorted(&back.spans), sorted(&formatting.spans));

        let image = Formatting {
            source: source.into(),
            spans: vec![FormatSpan {
                start: 0,
                end: 5,
                style: FormatStyle::Image {
                    data_url: url.into(),
                    width: 32,
                },
            }],
        };
        let doc = from_template(source, Some(&image));
        let image_pill = doc
            .pills
            .iter()
            .find(|p| p.kind == PillKind::Image)
            .unwrap();
        assert_eq!(image_pill.label, "Hello");
        assert_eq!(image_pill.source, "Hello");
        assert!(doc.images.iter().any(|i| i.width == 32));
        let (out, back) = doc.to_template();
        assert_eq!(out, source);
        assert_eq!(sorted(&back.unwrap().spans), sorted(&image.spans));
    }

    fn sorted(spans: &[FormatSpan]) -> Vec<(usize, usize, String)> {
        let mut v: Vec<_> = spans
            .iter()
            .map(|s| (s.start, s.end, format!("{:?}", s.style)))
            .collect();
        v.sort();
        v
    }

    fn fixture_sources() -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/migration/reference/fixtures/template.json"
        ))
        .unwrap();
        v["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c["source"].as_str().map(str::to_string))
            .collect()
    }

    fn grammar_sources() -> Vec<String> {
        let text = include_str!("../../../crates/kudzu-core/tests/template_grammar.rs");
        let mut out = Vec::new();
        for raw in rust_strings(text) {
            if raw.contains("{{") || raw.contains("\\\\") {
                out.push(raw);
            }
        }
        out.sort();
        out.dedup();
        out
    }

    fn rust_strings(text: &str) -> Vec<String> {
        let b = text.as_bytes();
        let mut i = 0;
        let mut out = Vec::new();
        while i < b.len() {
            if b[i] == b'r' && i + 1 < b.len() && b[i + 1] == b'"' {
                i += 2;
                let start = i;
                while i + 1 < b.len() && !(b[i] == b'"' && b[i + 1] != b'#') {
                    i += 1;
                }
                if i < b.len() {
                    out.push(text[start..i].to_string());
                    i += 1;
                }
                continue;
            }
            if b[i] == b'"' {
                i += 1;
                let mut s = String::new();
                while i < b.len() && b[i] != b'"' {
                    if b[i] == b'\\' && i + 1 < b.len() {
                        s.push(match b[i + 1] {
                            b'n' => '\n',
                            b't' => '\t',
                            b'r' => '\r',
                            b'\\' => '\\',
                            b'"' => '"',
                            b'{' => '{',
                            other => other as char,
                        });
                        i += 2;
                    } else {
                        s.push(b[i] as char);
                        i += 1;
                    }
                }
                out.push(s);
                i += 1;
                continue;
            }
            i += 1;
        }
        out
    }
}
