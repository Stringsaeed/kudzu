//! Rich text readers for imports: Markdown and HTML (Espanso `markdown:`/`html:` matches) and
//! RTF (TextExpander rich text snippets).
//!
//! Each reader turns its source into [`RichText`]: plain text in the *source app's* template
//! language (Espanso `{{var}}`, TextExpander `%macro%`) plus byte-range styles Kudzu can keep.
//! [`to_template`] then converts that text to a Kudzu template one segment at a time, so every
//! span endpoint maps to a valid source boundary, and produces `formatting` in UTF-16 offsets.
//!
//! The readers are small, hand-written and bounded: input size, nesting depth and delimiter
//! counts are capped, so malformed or hostile input degrades to plain text instead of panicking
//! or hanging.

mod html;
mod image;
mod markdown;
mod rtf;

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::formatting::{safe_image, FormatSpan, FormatStyle, Formatting};
use crate::template::{self, Node};

pub(crate) use html::read as read_html;
pub(crate) use markdown::read as read_markdown;
pub(crate) use rtf::read as read_rtf;

/// Largest Markdown or HTML source read for formatting (embedded images included).
const MAX_INPUT: usize = 1 << 20;
/// Deepest element or group nesting accepted before falling back to plain text.
const MAX_DEPTH: usize = 256;
/// Image budget per snippet, as data-URL characters (the editor's limit).
const MAX_IMAGE_URLS: usize = 524_288;

/// Something the source carried that Kudzu can't represent, named in the import warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Dropped {
    Fonts,
    TextSizes,
    Colors,
    Highlights,
    Strikethrough,
    Scripts,
    Alignment,
    Headings,
    BlockQuotes,
    NestedLists,
    Code,
    Tables,
    Rules,
    Links,
    ImagesNotEmbedded,
    ImagesUnsupported,
    ImagesTooLarge,
    Html,
}

impl Dropped {
    fn describe(self) -> &'static str {
        match self {
            Dropped::Fonts => "fonts",
            Dropped::TextSizes => "text sizes",
            Dropped::Colors => "colors",
            Dropped::Highlights => "highlight colors",
            Dropped::Strikethrough => "strikethrough",
            Dropped::Scripts => "superscript and subscript",
            Dropped::Alignment => "alignment",
            Dropped::Headings => "heading sizes (headings are bold)",
            Dropped::BlockQuotes => "block quote styling",
            Dropped::NestedLists => "nested list indentation",
            Dropped::Code => "code styling",
            Dropped::Tables => "table layout (cells are separated by tabs)",
            Dropped::Rules => "horizontal rules",
            Dropped::Links => "links that aren't web or email addresses (their text was kept)",
            Dropped::ImagesNotEmbedded => {
                "images that aren't embedded in the file (their text alternative was kept)"
            }
            Dropped::ImagesUnsupported => "images that aren't PNG or JPEG",
            Dropped::ImagesTooLarge => {
                "images larger than Kudzu's limits of 512 pixels, 144 KiB each and 384 KiB per snippet"
            }
            Dropped::Html => "other HTML elements",
        }
    }
}

/// Text in the source app's template language, with the styles Kudzu keeps as byte ranges.
#[derive(Debug, Default)]
pub(crate) struct RichText {
    pub(crate) text: String,
    spans: Vec<(Range<usize>, FormatStyle)>,
    /// The last span of each style, so adjacent runs merge instead of piling up.
    last: HashMap<FormatStyle, usize>,
    pub(crate) dropped: BTreeSet<Dropped>,
    image_budget: usize,
}

impl RichText {
    pub(crate) fn len(&self) -> usize {
        self.text.len()
    }

    pub(crate) fn push(&mut self, s: &str) {
        self.text.push_str(s);
    }

    pub(crate) fn push_char(&mut self, c: char) {
        self.text.push(c);
    }

    pub(crate) fn drop(&mut self, what: Dropped) {
        self.dropped.insert(what);
    }

    /// Styles `start..end` (bytes of `text`). Touching or overlapping runs of one style merge.
    pub(crate) fn span(&mut self, start: usize, end: usize, style: FormatStyle) {
        if start >= end {
            return;
        }
        if !matches!(style, FormatStyle::Image { .. }) {
            if let Some(&i) = self.last.get(&style) {
                let range = &mut self.spans[i].0;
                if range.start <= start && range.end >= start {
                    range.end = range.end.max(end);
                    return;
                }
            }
            self.last.insert(style.clone(), self.spans.len());
        }
        self.spans.push((start..end, style));
    }

    /// Validates image bytes against Kudzu's limits and the snippet's remaining budget.
    /// `declared_width` is the display width the source asked for, in pixels.
    pub(crate) fn image_style(
        &mut self,
        data: Result<Vec<u8>, Dropped>,
        declared_width: Option<u32>,
    ) -> Option<FormatStyle> {
        let result = data.and_then(|bytes| {
            let (mime, width, height) = image::sniff(&bytes).ok_or(Dropped::ImagesUnsupported)?;
            if width == 0 || height == 0 {
                return Err(Dropped::ImagesUnsupported);
            }
            if width.max(height) > 512 {
                return Err(Dropped::ImagesTooLarge);
            }
            let data_url = format!("data:{mime};base64,{}", image::encode_base64(&bytes));
            if !safe_image(&data_url) || self.image_budget + data_url.len() > MAX_IMAGE_URLS {
                return Err(Dropped::ImagesTooLarge);
            }
            self.image_budget += data_url.len();
            let width = declared_width
                .filter(|w| *w > 0)
                .map_or_else(|| width.clamp(16, 240), |w| w.min(512));
            Ok(FormatStyle::Image { data_url, width })
        });
        match result {
            Ok(style) => Some(style),
            Err(dropped) => {
                self.drop(dropped);
                None
            }
        }
    }

    /// One warning naming what was dropped, or `None` when everything was kept.
    pub(crate) fn warning(&self) -> Option<String> {
        if self.dropped.is_empty() {
            return None;
        }
        let names: Vec<&str> = self.dropped.iter().map(|d| d.describe()).collect();
        let list = match names.split_last() {
            Some((last, [])) => (*last).to_string(),
            Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
            None => String::new(),
        };
        Some(format!("Formatting imported, except {list}."))
    }

    /// Whether the readable text agrees with the source app's own plain text. Only letters and
    /// digits are compared, since list markers, tabs and image placeholders differ legitimately.
    pub(crate) fn agrees_with(&self, plain: &str) -> bool {
        let mut images: Vec<Range<usize>> = self
            .spans
            .iter()
            .filter(|(_, s)| matches!(s, FormatStyle::Image { .. }))
            .map(|(r, _)| r.clone())
            .collect();
        images.sort_by_key(|r| r.start);
        let mut ours = String::new();
        let mut at = 0;
        for r in images {
            if r.start >= at {
                ours.push_str(&self.text[at..r.start]);
                at = r.end;
            }
        }
        ours.push_str(&self.text[at..]);
        let alnum = |s: &str| {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        };
        alnum(&ours) == alnum(plain)
    }
}

/// The text alternative an image covers: `[alt]`, `[Image]` for an embedded image without one,
/// or nothing for a dropped image without one. Braces and line breaks become spaces, like the editor.
pub(crate) fn image_label(alt: &str, embedded: bool) -> String {
    let clean: String = alt
        .chars()
        .map(|c| {
            if matches!(c, '{' | '}' | '\r' | '\n') {
                ' '
            } else {
                c
            }
        })
        .take(200)
        .collect();
    let clean = clean.trim();
    if clean.is_empty() {
        if embedded {
            "[Image]".into()
        } else {
            String::new()
        }
    } else {
        format!("[{clean}]")
    }
}

/// A Kudzu template and the formatting that annotates it.
pub(crate) struct Converted {
    pub(crate) content: String,
    pub(crate) formatting: Formatting,
}

/// Converts `rich` to a Kudzu template with `convert` (the importer's own text → template
/// conversion), one segment between span boundaries at a time.
///
/// `atoms` are byte ranges of the source app's placeholders: span endpoints inside one move
/// outward so the placeholder is formatted as a whole. Endpoints also snap to grapheme
/// boundaries, and any span that still doesn't land on a valid template boundary is dropped.
pub(crate) fn to_template(
    rich: &RichText,
    atoms: &[Range<usize>],
    mut convert: impl FnMut(&str) -> String,
) -> Converted {
    let text = rich.text.as_str();
    let mut atoms: Vec<Range<usize>> = atoms.iter().cloned().chain(brace_atoms(text)).collect();
    atoms.sort_by_key(|a| (a.start, std::cmp::Reverse(a.end)));
    let mut merged: Vec<Range<usize>> = Vec::with_capacity(atoms.len());
    for a in atoms {
        match merged.last_mut() {
            Some(last) if a.start < last.end => last.end = last.end.max(a.end),
            _ => merged.push(a),
        }
    }
    let graphemes: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();
    let inside = |p: usize| -> Option<&Range<usize>> {
        let i = merged.partition_point(|a| a.start < p);
        i.checked_sub(1).map(|i| &merged[i]).filter(|a| a.end > p)
    };
    let snap = |mut start: usize, mut end: usize| {
        for _ in 0..8 {
            let before = (start, end);
            if let Some(a) = inside(start) {
                start = a.start;
            }
            if let Some(a) = inside(end) {
                end = a.end;
            }
            start = graphemes[graphemes.partition_point(|&g| g <= start) - 1];
            end = graphemes[graphemes
                .partition_point(|&g| g < end)
                .min(graphemes.len() - 1)];
            if before == (start, end) {
                break;
            }
        }
        (start, end)
    };
    let spans: Vec<(usize, usize, &FormatStyle)> = rich
        .spans
        .iter()
        .map(|(r, style)| {
            let (s, e) = snap(r.start, r.end);
            (s, e, style)
        })
        .collect();

    let mut boundaries: Vec<usize> = vec![0, text.len()];
    for (s, e, _) in &spans {
        boundaries.extend([*s, *e]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut content = String::new();
    let mut mapped = vec![0usize];
    let mut utf16 = 0;
    for pair in boundaries.windows(2) {
        let out = convert(&text[pair[0]..pair[1]]);
        utf16 += out.encode_utf16().count();
        content.push_str(&out);
        mapped.push(utf16);
    }
    let at = |byte: usize| mapped[boundaries.binary_search(&byte).unwrap_or(0)];

    let valid = valid_boundaries(&content);
    let mut out: Vec<FormatSpan> = spans
        .into_iter()
        .map(|(s, e, style)| FormatSpan {
            start: at(s),
            end: at(e),
            style: style.clone(),
        })
        .filter(|s| s.start < s.end && valid[s.start] && valid[s.end])
        .collect();
    out.sort();
    out.dedup();
    Converted {
        formatting: Formatting {
            source: content.clone(),
            spans: out,
        },
        content,
    }
}

/// Runs of `{` (with any backslashes before them): splitting one could create or break a tag.
fn brace_atoms(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' || bytes[i] == b'{' {
            let start = i;
            while i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
            }
            let braces = i;
            while i < bytes.len() && bytes[i] == b'{' {
                i += 1;
            }
            if i - start >= 2 && i > braces {
                out.push(start..i);
            }
            if i == start {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Espanso placeholders, mirroring the importer's `replace:` conversion: `$|$`, `\{\{`, `\{{`
/// and `{{…}}` (or a lone `{{`).
pub(crate) fn espanso_atoms(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut next_close: Option<Option<usize>> = None;
    let mut i = 0;
    while i < bytes.len() {
        let rest = &bytes[i..];
        let len = if rest.starts_with(b"$|$") {
            3
        } else if rest.starts_with(b"\\{\\{") {
            4
        } else if rest.starts_with(b"\\{{") {
            3
        } else if rest.starts_with(b"{{") {
            // The next `}}` from here; cached so a run of unterminated `{{` stays linear.
            let close = match next_close {
                Some(Some(c)) if c >= i + 2 => Some(c),
                Some(None) => None,
                _ => {
                    let found = text[i + 2..].find("}}").map(|c| c + i + 2);
                    next_close = Some(found);
                    found
                }
            };
            close.map_or(2, |c| c + 2 - i)
        } else {
            0
        };
        if len > 0 {
            out.push(i..i + len);
            i += len;
        } else {
            i += 1;
        }
    }
    out
}

/// TextExpander macros: `%name:…%`, `%fillpart:…%…%fillpartend%`, `%clipboard`, `%|`, and runs
/// of date specifiers (`%@+1D%m/%d/%Y`) that the importer joins into one tag.
pub(crate) fn textexpander_atoms(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let next_percent = |from: usize| {
        bytes[from.min(bytes.len())..]
            .iter()
            .position(|&b| b == b'%')
            .map(|p| p + from)
    };
    let mut out = Vec::new();
    let mut fillpart_end: Option<Option<usize>> = None;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        let rest = &bytes[i..];
        let end = if rest.starts_with(b"%fillpart:") {
            let close = match fillpart_end {
                Some(Some(c)) if c > i => Some(c),
                Some(None) => None,
                _ => {
                    let found = text[i..].find("%fillpartend%").map(|c| c + i);
                    fillpart_end = Some(found);
                    found
                }
            };
            close
                .map(|c| c + "%fillpartend%".len())
                .or_else(|| next_percent(i + 1).map(|c| c + 1))
        } else if rest.starts_with(b"%clipboard") {
            Some(i + "%clipboard".len())
        } else if rest.starts_with(b"%|") {
            Some(i + 2)
        } else if rest.len() > 1 && rest[1].is_ascii_alphabetic() && {
            let name_len = rest[1..]
                .iter()
                .take_while(|b| b.is_ascii_alphabetic())
                .count();
            rest.get(1 + name_len) == Some(&b':')
        } {
            next_percent(i + 1).map(|c| c + 1)
        } else {
            date_run(bytes, i)
        };
        match end {
            Some(end) if end > i + 1 => {
                out.push(i..end);
                i = end;
            }
            _ => i += 1,
        }
    }
    out
}

/// The end of a run of date specifiers starting at `i`, separators of up to two characters
/// between them, optionally after date math (`%@+1D`).
fn date_run(bytes: &[u8], mut i: usize) -> Option<usize> {
    let start = i;
    if bytes.get(i + 1) == Some(&b'@') {
        let mut k = i + 2;
        if matches!(bytes.get(k), Some(b'+' | b'-')) {
            k += 1;
            while bytes.get(k).is_some_and(u8::is_ascii_digit) {
                k += 1;
            }
            if bytes.get(k).is_some_and(u8::is_ascii_alphabetic) {
                i = k + 1;
            }
        }
        if i == start {
            return None;
        }
    }
    let spec = |at: usize| -> Option<usize> {
        if bytes.get(at) != Some(&b'%') {
            return None;
        }
        match bytes.get(at + 1)? {
            b'1' if bytes.get(at + 2).is_some_and(u8::is_ascii_alphabetic) => Some(at + 3),
            c if c.is_ascii_alphabetic() => Some(at + 2),
            _ => None,
        }
    };
    let mut end = spec(i)?;
    loop {
        let mut k = end;
        while k - end < 2 && matches!(bytes.get(k), Some(b' ' | b'-' | b'/' | b':' | b',' | b'.')) {
            k += 1;
        }
        // A later specifier followed by a letter is the start of another macro (`%fillpart:`).
        match spec(k).filter(|&next| !bytes.get(next).is_some_and(u8::is_ascii_alphabetic)) {
            Some(next) => end = next,
            None => return Some(end),
        }
    }
}

/// For each UTF-16 offset of `source`, whether formatting may start or end there: tag edges and
/// grapheme boundaries of literal text, never inside an escape (mirrors the renderer).
fn valid_boundaries(source: &str) -> Vec<bool> {
    let utf16: Vec<u16> = source.encode_utf16().collect();
    let mut ok = vec![false; utf16.len() + 1];
    ok[0] = true;
    ok[utf16.len()] = true;
    for node in template::parse(source).nodes {
        match node {
            Node::Tag(tag) => {
                ok[tag.range.start] = true;
                ok[tag.range.end] = true;
            }
            Node::Text { range, .. } => {
                let raw = String::from_utf16_lossy(&utf16[range.clone()]);
                let graphemes: Vec<&str> = raw.graphemes(true).collect();
                let mut at = range.start;
                ok[at] = true;
                let mut i = 0;
                while i < graphemes.len() {
                    let escaped = graphemes[i] == "\\"
                        && (graphemes.get(i + 1) == Some(&"\\")
                            || (graphemes.get(i + 1) == Some(&"{")
                                && graphemes.get(i + 2) == Some(&"{")));
                    if escaped {
                        at += 1;
                        i += 1;
                    }
                    at += graphemes[i].encode_utf16().count();
                    if let Some(slot) = ok.get_mut(at) {
                        *slot = true;
                    }
                    i += 1;
                }
            }
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(rich: &RichText) -> Converted {
        to_template(rich, &[], |s| {
            crate::template::serialize(&[Node::Text {
                text: s.to_string(),
                range: 0..0,
            }])
        })
    }

    #[test]
    fn spans_snap_out_of_placeholders_and_graphemes() {
        let mut rich = RichText::default();
        rich.push("a {{date}} e\u{301}x");
        // Starts inside the placeholder, ends inside the combining sequence.
        rich.span(4, 13, FormatStyle::Bold);
        let atoms = espanso_atoms(&rich.text);
        assert_eq!(atoms, vec![2..10]);
        let out = to_template(&rich, &atoms, |s| {
            s.replace("{{date}}", "{{date format=\"y\"}}")
        });
        assert_eq!(out.content, "a {{date format=\"y\"}} e\u{301}x");
        assert_eq!(
            out.formatting.spans,
            vec![FormatSpan {
                start: 2,
                end: 24,
                style: FormatStyle::Bold
            }]
        );
    }

    #[test]
    fn literal_braces_are_escaped_and_never_split() {
        let mut rich = RichText::default();
        rich.push("x{{y");
        rich.span(2, 4, FormatStyle::Italic);
        let out = identity(&rich);
        assert_eq!(out.content, "x\\{{y");
        assert_eq!(
            out.formatting.spans,
            vec![FormatSpan {
                start: 1,
                end: 5,
                style: FormatStyle::Italic
            }]
        );
    }

    #[test]
    fn adjacent_runs_merge() {
        let mut rich = RichText::default();
        rich.push("abc");
        rich.span(0, 1, FormatStyle::Bold);
        rich.span(1, 2, FormatStyle::Bold);
        rich.span(2, 3, FormatStyle::Bold);
        assert_eq!(rich.spans.len(), 1);
    }

    #[test]
    fn textexpander_atoms_cover_macros() {
        let text = "a %filltext:name=x% %m/%d/%Y b %@+1D%A %fillpart:n%in%fillpartend% 50% %|";
        let atoms: Vec<&str> = textexpander_atoms(text)
            .into_iter()
            .map(|r| &text[r])
            .collect();
        assert_eq!(
            atoms,
            vec![
                "%filltext:name=x%",
                "%m/%d/%Y",
                "%@+1D%A",
                "%fillpart:n%in%fillpartend%",
                "%|"
            ]
        );
    }

    #[test]
    fn warning_lists_what_was_dropped() {
        let mut rich = RichText::default();
        assert_eq!(rich.warning(), None);
        rich.drop(Dropped::Colors);
        rich.drop(Dropped::Fonts);
        rich.drop(Dropped::Tables);
        assert_eq!(
            rich.warning().unwrap(),
            "Formatting imported, except fonts, colors and table layout (cells are separated by tabs)."
        );
    }
}
