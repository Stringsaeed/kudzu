//! Load a template into pieces, and the literal-step map those pieces share with edits.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use super::{EditorDoc, EditorStyle, Image, Mark, PillId, Raw, RawPill};
use crate::formatting::{safe_image, safe_link, FormatSpan, FormatStyle, Formatting};
use crate::template::chips::{self, PillKind, Tone};
use crate::template::{Node, Tag};

pub(super) fn paint_range(doc: &EditorDoc, source: &Range<usize>) -> Range<usize> {
    let start = doc.source_to_display(source.start, false);
    let end = doc.source_to_display(source.end, true);
    if start < end {
        start..end
    } else {
        start..start
    }
}

pub(super) fn raws_eq(a: &[Raw], b: &[Raw]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.display == b.display
                && a.source == b.source
                && a.steps == b.steps
                && match (&a.pill, &b.pill) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.id == b.id && a.tone == b.tone && a.kind == b.kind,
                    _ => false,
                }
        })
}

pub(super) fn marks_eq(a: &[Mark], b: &[Mark]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.source == b.source && a.style == b.style && a.seq == b.seq)
}

pub(super) fn images_eq(a: &[Image], b: &[Image]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.pill == b.pill
                && a.source == b.source
                && a.data_url == b.data_url
                && a.width == b.width
                && a.alt == b.alt
                && a.seq == b.seq
        })
}

pub(super) fn to_format_style(style: &EditorStyle) -> FormatStyle {
    match style {
        EditorStyle::Bold => FormatStyle::Bold,
        EditorStyle::Italic => FormatStyle::Italic,
        EditorStyle::Underline => FormatStyle::Underline,
        EditorStyle::Link { url } => FormatStyle::Link { url: url.clone() },
    }
}

fn from_format_style(style: &FormatStyle) -> Option<EditorStyle> {
    Some(match style {
        FormatStyle::Bold => EditorStyle::Bold,
        FormatStyle::Italic => EditorStyle::Italic,
        FormatStyle::Underline => EditorStyle::Underline,
        FormatStyle::Link { url } => EditorStyle::Link { url: url.clone() },
        FormatStyle::Image { .. } => return None,
    })
}

pub(super) fn apply_formatting(
    raws: &mut Vec<Raw>,
    marks: &mut Vec<Mark>,
    images: &mut Vec<Image>,
    seq: &mut u32,
    source: &str,
    formatting: &Formatting,
    next_pill: &mut u64,
) {
    for span in formatting.spans.iter().filter(|s| span_ok(source, s)) {
        let order = *seq;
        *seq += 1;
        match &span.style {
            FormatStyle::Image { data_url, width } => {
                images.push(
                    match place_image_pill(raws, span.start, span.end, next_pill) {
                        Some(id) => {
                            let alt = raws
                                .iter()
                                .find(|r| r.pill.as_ref().is_some_and(|p| p.id == id))
                                .map(|r| r.display.clone())
                                .unwrap_or_default();
                            Image {
                                pill: Some(id),
                                source: span.start..span.end,
                                data_url: data_url.clone(),
                                width: *width,
                                alt,
                                seq: order,
                            }
                        }
                        None => Image {
                            pill: None,
                            source: span.start..span.end,
                            data_url: data_url.clone(),
                            width: *width,
                            alt: String::new(),
                            seq: order,
                        },
                    },
                );
            }
            other => {
                if let Some(style) = from_format_style(other) {
                    marks.push(Mark {
                        source: span.start..span.end,
                        style,
                        seq: order,
                    });
                }
            }
        }
    }
}

fn span_ok(source: &str, span: &FormatSpan) -> bool {
    let len = utf16_len(source);
    span.start < span.end
        && span.end <= len
        && utf16_boundary(source, span.start)
        && utf16_boundary(source, span.end)
        && match &span.style {
            FormatStyle::Link { url } => safe_link(url),
            FormatStyle::Image { data_url, width } => {
                safe_image(data_url) && (1..=512).contains(width)
            }
            _ => true,
        }
}

fn utf16_boundary(text: &str, pos: usize) -> bool {
    let units: Vec<u16> = text.encode_utf16().collect();
    if pos > units.len() {
        return false;
    }
    if pos > 0 && pos < units.len() {
        let prev = units[pos - 1];
        let here = units[pos];
        if (0xD800..=0xDBFF).contains(&prev) && (0xDC00..=0xDFFF).contains(&here) {
            return false;
        }
    }
    true
}

pub(super) fn pieces_from_parse(
    source: &str,
    snippet_name: &dyn Fn(&str) -> Option<&str>,
    next_pill: &mut u64,
) -> Vec<Raw> {
    let parsed = crate::template::parse(source);
    let total = utf16_len(source);
    let mut raws = Vec::new();
    let mut at = 0usize;
    for node in &parsed.nodes {
        let range = node_range(node);
        if range.start > at {
            raws.push(literal_raw(&slice_utf16(source, at..range.start)));
        }
        match node {
            Node::Text { text, .. } => {
                raws.push(text_raw(&slice_utf16(source, range.clone()), text))
            }
            Node::Tag(tag) => raws.push(tag_raw(source, tag, &range, snippet_name, next_pill)),
        }
        at = at.max(range.end);
    }
    if at < total {
        raws.push(literal_raw(&slice_utf16(source, at..total)));
    }
    if raws.is_empty() {
        raws.push(literal_raw(""));
    }
    raws
}

fn tag_raw(
    source: &str,
    tag: &Tag,
    range: &Range<usize>,
    snippet_name: &dyn Fn(&str) -> Option<&str>,
    next_pill: &mut u64,
) -> Raw {
    let slice = slice_utf16(source, range.clone());
    let id = PillId(*next_pill);
    *next_pill += 1;
    Raw {
        display: chips::chip_label(tag, &slice, snippet_name),
        source: slice,
        pill: Some(RawPill {
            id,
            tone: chips::chip_tone(tag),
            kind: PillKind::Tag,
        }),
        steps: Vec::new(),
    }
}

fn text_raw(slice: &str, decoded: &str) -> Raw {
    let (display, steps) = literal_map(slice);
    if display == decoded {
        Raw {
            display,
            source: slice.to_string(),
            pill: None,
            steps,
        }
    } else {
        Raw {
            display: decoded.to_string(),
            source: slice.to_string(),
            pill: None,
            steps: vec![(0, 0), (decoded.len(), utf16_len(slice))],
        }
    }
}

fn literal_raw(slice: &str) -> Raw {
    let (display, steps) = literal_map(slice);
    Raw {
        display,
        source: slice.to_string(),
        pill: None,
        steps,
    }
}

fn node_range(node: &Node) -> Range<usize> {
    match node {
        Node::Text { range, .. } => range.clone(),
        Node::Tag(tag) => tag.range.clone(),
    }
}

/// The UTF-16 slice `range` of `source`. A unit that would split a scalar is omitted.
pub(super) fn slice_utf16(source: &str, range: Range<usize>) -> String {
    let mut at = 0usize;
    let mut out = String::new();
    for ch in source.chars() {
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

pub(super) fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

pub(super) fn escape_literal(text: &str) -> String {
    crate::template::serialize(&[Node::Text {
        text: text.to_string(),
        range: 0..0,
    }])
}

/// A literal piece whose source is the canonical escaping of `text`.
pub(super) fn literal_from_display(text: &str) -> Raw {
    let source = escape_literal(text);
    let (decoded, steps) = literal_map(&source);
    if decoded == text {
        Raw {
            display: decoded,
            source,
            pill: None,
            steps,
        }
    } else {
        let units = utf16_len(&source);
        Raw {
            display: text.to_string(),
            source,
            pill: None,
            steps: vec![(0, 0), (text.len(), units)],
        }
    }
}

pub(super) fn literal_map(source: &str) -> (String, Vec<(usize, usize)>) {
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

pub(super) fn step_display(steps: &[(usize, usize)], src_local: usize, snap_end: bool) -> usize {
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

pub(super) fn step_source(steps: &[(usize, usize)], display_local: usize, snap_end: bool) -> usize {
    if let Some((_, src)) = steps.iter().find(|(display, _)| *display == display_local) {
        return *src;
    }
    if snap_end {
        steps
            .iter()
            .find(|(d, _)| *d > display_local)
            .map(|(_, s)| *s)
            .unwrap_or(steps.last().map(|s| s.1).unwrap_or(0))
    } else {
        steps
            .iter()
            .rev()
            .find(|(d, _)| *d < display_local)
            .map(|(_, s)| *s)
            .unwrap_or(0)
    }
}

/// Replace the literal covered by `start..end` with an image chip. `None` when a tag overlaps
/// the range or an endpoint is not on a step — the caller keeps an unaligned image instead.
pub(super) fn place_image_pill(
    raws: &mut Vec<Raw>,
    start: usize,
    end: usize,
    next_pill: &mut u64,
) -> Option<PillId> {
    if start >= end {
        return None;
    }
    let mut cursor = 0usize;
    let mut overlaps_pill = false;
    let mut covered = false;
    for raw in raws.iter() {
        let len = utf16_len(&raw.source);
        let a = cursor;
        let b = cursor + len;
        if a < end && b > start {
            covered = true;
            if raw.pill.is_some() {
                overlaps_pill = true;
            } else if !on_step(&raw.steps, start.saturating_sub(a).min(len))
                || !on_step(&raw.steps, end.min(b) - a)
            {
                return None;
            }
        }
        cursor = b;
    }
    if !covered || overlaps_pill {
        return None;
    }
    split_and_replace_literals(raws, start, end, next_pill)
}

fn on_step(steps: &[(usize, usize)], src_local: usize) -> bool {
    steps.iter().any(|(_, src)| *src == src_local)
}

fn split_and_replace_literals(
    raws: &mut Vec<Raw>,
    start: usize,
    end: usize,
    next_pill: &mut u64,
) -> Option<PillId> {
    let mut cursor = 0usize;
    let mut out = Vec::new();
    let mut display = String::new();
    let mut source = String::new();
    let mut placed = None;
    for raw in raws.iter() {
        let len = utf16_len(&raw.source);
        let a = cursor;
        let b = cursor + len;
        cursor = b;
        if b <= start || a >= end {
            if a >= end && placed.is_none() {
                placed = Some(push_image(&mut out, &display, &source, next_pill));
            }
            out.push(raw.clone());
            continue;
        }
        if raw.pill.is_some() {
            return None;
        }
        let local_s = start.saturating_sub(a);
        let local_e = end.min(b) - a;
        if local_s > 0 {
            out.push(slice_raw(raw, 0, local_s));
        }
        let mid = slice_raw(raw, local_s, local_e);
        display.push_str(&mid.display);
        source.push_str(&mid.source);
        if end <= b && placed.is_none() {
            placed = Some(push_image(&mut out, &display, &source, next_pill));
        }
        if local_e < len {
            out.push(slice_raw(raw, local_e, len));
        }
    }
    if placed.is_none() {
        placed = Some(push_image(&mut out, &display, &source, next_pill));
    }
    if display.is_empty() {
        return None;
    }
    *raws = out;
    placed
}

fn push_image(out: &mut Vec<Raw>, display: &str, source: &str, next_pill: &mut u64) -> PillId {
    let id = PillId(*next_pill);
    *next_pill += 1;
    out.push(Raw {
        display: display.to_string(),
        source: source.to_string(),
        pill: Some(RawPill {
            id,
            tone: Tone::Form,
            kind: PillKind::Image,
        }),
        steps: Vec::new(),
    });
    id
}

pub(super) fn slice_raw(raw: &Raw, src_from: usize, src_to: usize) -> Raw {
    let display_from = step_display(&raw.steps, src_from, false);
    let display_to = step_display(&raw.steps, src_to, true);
    let source = slice_utf16(&raw.source, src_from..src_to);
    let steps = raw
        .steps
        .iter()
        .copied()
        .filter(|(_, s)| *s >= src_from && *s <= src_to)
        .map(|(d, s)| (d - display_from, s - src_from))
        .collect();
    let display = raw
        .display
        .get(display_from..display_to)
        .unwrap_or("")
        .to_string();
    Raw {
        display,
        source,
        pill: None,
        steps,
    }
}
