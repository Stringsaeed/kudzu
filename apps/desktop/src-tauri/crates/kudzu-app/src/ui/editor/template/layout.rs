//! Lays the template out as CodeMirror's `.kz-editor` drew it: lines wrapped at the editor's
//! width (`white-space: pre-wrap; overflow-wrap: anywhere`), template tags as inline-flex chips
//! and image spans as inline images at their display width, all vertically aligned the way CSS
//! aligns them in CodeMirror's 18.2 pt line.
//!
//! Text is shaped with Rune's own text system (the same shaping the runtime draws with), so the
//! caret and selection land on the glyphs. Every fragment remembers the caret stops inside it,
//! which is all the editor needs for hit testing, vertical motion and selection rectangles.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use base64::Engine;
use kudzu_core::client::formatting::valid_spans;
use kudzu_core::client::js;
use kudzu_core::client::template::{chip_label, chip_tone, tokenize, ChipTone, Token};
use kudzu_core::formatting::{FormatSpan, FormatStyle};
use rune::text::{Run, TextSystem};
use rune::{Family, Pixels, TextStyle};

use crate::ui::theme::{self, weight};

/// `.cm-line { padding: 0 14px }`.
pub const PAD_X: f32 = 14.0;
/// `.cm-content { padding: 12px 0 }`.
pub const PAD_Y: f32 = 12.0;
/// `font-size: 13px`, and CodeMirror's `.cm-scroller { line-height: 1.4 }` (its base theme
/// wins over `.kz-editor .cm-editor`'s 1.7, which sits on an ancestor).
pub const LINE_H: f32 = 18.2;
pub const SIZE: f32 = 13.0;

/// `.kz-chip`: 20 px tall, `padding: 0 7px`, `margin: 0 1px`.
pub const CHIP_H: f32 = 20.0;
pub const CHIP_PAD: f32 = 7.0;
pub const CHIP_MARGIN: f32 = 1.0;
/// `font-size: 11.5px; font-weight: 500` on the app's sans face (mono for unrecognised tags).
pub const CHIP_FONT: TextStyle = TextStyle::new(11.5, weight::MEDIUM).family(Family::Sans).lh(11.5);
pub const CHIP_FONT_MONO: TextStyle = TextStyle::new(11.5, weight::MEDIUM).family(Family::Mono).lh(11.5);

/// The editor's text face: Paper Mono for plain text, the app's sans once formatted.
pub fn base_font(formatted: bool) -> TextStyle {
    let f = TextStyle::new(SIZE, weight::NORMAL).lh(LINE_H);
    if formatted { f.family(Family::Sans) } else { f.family(Family::Mono) }
}

/// Above-baseline part of the line's strut (ascent plus half-leading) for a face, from its
/// ascent and descent in ems: SF's 0.952 / 0.241, Paper Mono's 0.9 / 0.25.
fn strut_ascent(formatted: bool) -> f32 {
    let (asc, desc) = if formatted { (0.952, 0.241) } else { (0.9, 0.25) };
    LINE_H / 2.0 + (asc - desc) / 2.0 * SIZE
}

/// Half the x-height, which `vertical-align: middle` centres images on.
const HALF_X: f32 = 0.26 * SIZE;
/// Where a chip's label baseline sits from the chip's top (centred 11.5 px label, line-height 1).
const CHIP_BASELINE: f32 = 14.1;

// MARK: Shaping

/// Caret stops of a shaped run: each grapheme's byte and x, then (len, width).
#[derive(Debug)]
pub struct Shaped {
    pub stops: Vec<(usize, f32)>,
}

impl Shaped {
    pub fn width(&self) -> f32 {
        self.stops.last().map_or(0.0, |s| s.1)
    }
    /// x at byte `b` (a grapheme boundary; otherwise the stop before it).
    pub fn x(&self, b: usize) -> f32 {
        match self.stops.binary_search_by_key(&b, |s| s.0) {
            Ok(i) => self.stops[i].1,
            Err(i) => self.stops[i.saturating_sub(1)].1,
        }
    }
}

thread_local! {
    static TEXT: RefCell<Option<TextSystem>> = const { RefCell::new(None) };
    static SHAPED: RefCell<HashMap<(String, u64), Rc<Shaped>>> = RefCell::new(HashMap::new());
}

fn style_key(s: TextStyle) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// `text` shaped on one line in `style`, with every grapheme's x in points.
pub fn shape(text: &str, style: TextStyle) -> Rc<Shaped> {
    let key = (text.to_string(), style_key(style));
    if let Some(s) = SHAPED.with_borrow(|m| m.get(&key).cloned()) {
        return s;
    }
    let shaped = Rc::new(if text.is_empty() {
        Shaped { stops: vec![(0, 0.0)] }
    } else {
        TEXT.with_borrow_mut(|t| {
            let ts = t.get_or_insert_with(|| TextSystem::new(theme::FONTS, (theme::SANS, theme::MONO)));
            let run = ts.run_layout(&Run::new(text), style, 1.0);
            let mut stops = Vec::new();
            let mut last = (0usize, 0.0f32);
            for (b, _) in text.char_indices() {
                if let Some(x) = run.x(b) {
                    stops.push((b, x));
                    last = (b, x);
                }
            }
            let end = last.1 + run.advance(last.0, 1.0).unwrap_or(0.0);
            stops.push((text.len(), end));
            ts.end_commit();
            Shaped { stops }
        })
    });
    SHAPED.with_borrow_mut(|m| {
        if m.len() > 4096 {
            m.clear();
        }
        m.insert(key, shaped.clone());
    });
    shaped
}

// MARK: Images

/// An embedded image's pixels and natural size, decoded once per data URL.
#[derive(Clone)]
pub struct Img {
    pub pixels: Pixels,
    pub size: (u32, u32),
}

pub fn image(data_url: &str) -> Option<Img> {
    thread_local! {
        static IMAGES: RefCell<HashMap<String, Option<Img>>> = RefCell::new(HashMap::new());
    }
    if let Some(i) = IMAGES.with_borrow(|m| m.get(data_url).cloned()) {
        return i;
    }
    let decoded = (|| {
        let b64 = data_url.split_once(";base64,")?.1;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let pixels = Pixels::new();
        pixels.set(w, h, img.into_raw());
        Some(Img { pixels, size: (w, h) })
    })();
    IMAGES.with_borrow_mut(|m| {
        if m.len() > 64 {
            m.clear();
        }
        m.insert(data_url.to_string(), decoded.clone());
    });
    decoded
}

// MARK: Layout

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Text { text: String, style: TextStyle, underline: bool, link: bool },
    Chip { label: String, tone: ChipTone, verbatim: bool, source: String, italic: bool },
    Image { url: String, alt: String },
    Tab,
    /// The input method's composition, drawn at the caret and not yet in the document.
    Preedit { text: String, style: TextStyle },
}

/// One drawn piece of a line. `stops` are the caret positions inside it (document offset and
/// x in editor points); a preedit has none.
#[derive(Clone, Debug)]
pub struct Frag {
    pub from: usize,
    pub to: usize,
    pub x: f32,
    pub w: f32,
    /// Top in editor points and height, for chips and images.
    pub y: f32,
    pub h: f32,
    pub kind: Kind,
    pub stops: Vec<(usize, f32)>,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub top: f32,
    pub height: f32,
    /// The baseline, from the editor's top.
    pub baseline: f32,
    pub from: usize,
    pub to: usize,
    pub frags: Vec<Frag>,
    /// Ends at a line break or the document's end (not a soft wrap).
    pub hard: bool,
}

impl Line {
    /// Every caret stop in the line, in order.
    pub fn stops(&self) -> Vec<(usize, f32)> {
        let mut out: Vec<(usize, f32)> = Vec::new();
        for f in &self.frags {
            for &s in &f.stops {
                if out.last().is_some_and(|l| l.0 == s.0) {
                    continue;
                }
                out.push(s);
            }
        }
        if out.is_empty() {
            out.push((self.from, PAD_X));
        }
        out
    }
    pub fn x_of(&self, pos: usize) -> f32 {
        let stops = self.stops();
        stops.iter().find(|s| s.0 == pos).or_else(|| stops.iter().rev().find(|s| s.0 <= pos)).map_or(PAD_X, |s| s.1)
    }
    /// The stop nearest `x`.
    pub fn pos_at(&self, x: f32) -> usize {
        let stops = self.stops();
        let mut best = stops[0];
        for &s in &stops {
            if (s.1 - x).abs() < (best.1 - x).abs() {
                best = s;
            }
        }
        // A press on an atom's left half lands before it, right half after.
        best.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub lines: Vec<Line>,
    /// The editor's full height (padding included).
    pub height: f32,
    pub width: f32,
}

/// What the layout needs to know about the document.
pub struct Input<'a> {
    pub text: &'a str,
    pub spans: Option<&'a [FormatSpan]>,
    pub width: f32,
    pub snippet_name: &'a dyn Fn(&str) -> Option<String>,
    /// The composition and the caret it sits at.
    pub preedit: Option<(&'a str, usize)>,
}

/// One unbreakable-or-not piece before line breaking.
struct Item {
    piece: usize,
    /// Byte range in the piece's text (text pieces) or the whole piece.
    b0: usize,
    b1: usize,
    w: f32,
    /// Width of trailing spaces (they hang past the line's end).
    hang: f32,
    break_after: bool,
}

struct Piece {
    from: usize,
    to: usize,
    kind: Kind,
    /// Text pieces: the shaped run and UTF-16 offset of each byte boundary.
    shaped: Option<Rc<Shaped>>,
    w: f32,
    h: f32,
}

fn chip_width(label: &str, verbatim: bool) -> f32 {
    shape(label, if verbatim { CHIP_FONT_MONO } else { CHIP_FONT }).width() + 2.0 * (CHIP_PAD + CHIP_MARGIN)
}

/// Byte offset → UTF-16 offset within `s`.
fn u16_at(s: &str, b: usize) -> usize {
    s[..b].encode_utf16().count()
}

fn styles_at(spans: &[FormatSpan], from: usize, to: usize) -> (bool, bool, bool, bool) {
    let has = |f: &dyn Fn(&FormatStyle) -> bool| spans.iter().any(|s| s.start <= from && s.end >= to && f(&s.style));
    (
        has(&|s| matches!(s, FormatStyle::Bold)),
        has(&|s| matches!(s, FormatStyle::Italic)),
        has(&|s| matches!(s, FormatStyle::Underline)),
        has(&|s| matches!(s, FormatStyle::Link { .. })),
    )
}

pub fn layout(input: &Input<'_>) -> Layout {
    let formatted = input.spans.is_some();
    let spans: Vec<FormatSpan> = input.spans.map(|s| valid_spans(input.text, s)).unwrap_or_default();
    let base = base_font(formatted);
    let units = js::utf16(input.text);
    let content_w = (input.width - 2.0 * PAD_X).max(1.0);

    // 1. Pieces: images, chips, then text split at style boundaries, tabs and line breaks.
    let images: Vec<&FormatSpan> = spans.iter().filter(|s| matches!(s.style, FormatStyle::Image { .. })).collect();
    let mut cuts: Vec<usize> = vec![0, units.len()];
    for s in &spans {
        cuts.extend([s.start, s.end]);
    }
    if let Some((_, at)) = input.preedit {
        cuts.push(at.min(units.len()));
    }
    let mut pieces: Vec<Piece> = Vec::new();
    let push_text = |pieces: &mut Vec<Piece>, from: usize, to: usize| {
        let mut a = from;
        while a < to {
            // Next cut, line break or tab.
            let mut b = to;
            for &c in &cuts {
                if c > a && c < b {
                    b = c;
                }
            }
            if let Some(i) = units[a..b].iter().position(|&u| u == b'\n' as u16 || u == b'\t' as u16) {
                b = a + i;
            }
            if b == a {
                let u = units[a];
                if u == b'\t' as u16 {
                    pieces.push(Piece { from: a, to: a + 1, kind: Kind::Tab, shaped: None, w: 0.0, h: 0.0 });
                } else {
                    // A line break: a zero-width marker the breaker splits on.
                    pieces.push(Piece { from: a, to: a + 1, kind: Kind::Text { text: "\n".into(), style: base, underline: false, link: false }, shaped: None, w: 0.0, h: 0.0 });
                }
                a += 1;
                continue;
            }
            let text = js::from16(&units[a..b]);
            let (bold, italic, underline, link) = styles_at(&spans, a, b);
            let mut style = base;
            if bold {
                style.weight = weight::BOLD;
            }
            if italic {
                style = style.italic();
            }
            let shaped = shape(&text, style);
            let w = shaped.width();
            pieces.push(Piece { from: a, to: b, kind: Kind::Text { text, style, underline: underline || link, link }, shaped: Some(shaped), w, h: 0.0 });
            a = b;
        }
    };
    let mut at = 0;
    let tokens = tokenize(input.text);
    let mut events: Vec<(usize, usize, Option<Kind>)> = Vec::new();
    for t in &tokens {
        if let Token::Tag(tag) = t {
            if images.iter().any(|s| s.start < tag.to && s.end > tag.from) {
                continue;
            }
            let label = chip_label(tag, Some(input.snippet_name));
            let italic = formatted && styles_at(&spans, tag.from, tag.to).1;
            events.push((tag.from, tag.to, Some(Kind::Chip { label, tone: chip_tone(tag), verbatim: tag.verbatim, source: tag.source.clone(), italic })));
        }
    }
    for s in &images {
        if let FormatStyle::Image { data_url, .. } = &s.style {
            let alt = js::from16(&units[s.start..s.end]);
            events.push((s.start, s.end, Some(Kind::Image { url: data_url.clone(), alt })));
        }
    }
    if let Some((text, caret)) = input.preedit {
        let (bold, italic, ..) = styles_at(&spans, caret, caret);
        let mut style = base;
        if bold {
            style.weight = weight::BOLD;
        }
        if italic {
            style = style.italic();
        }
        events.push((caret.min(units.len()), caret.min(units.len()), Some(Kind::Preedit { text: text.to_string(), style })));
    }
    events.sort_by_key(|e| (e.0, e.1));
    for (from, to, kind) in events {
        if from < at {
            continue;
        }
        push_text(&mut pieces, at, from);
        let kind = kind.expect("kind");
        let (w, h, shaped) = match &kind {
            Kind::Chip { label, verbatim, .. } => (chip_width(label, *verbatim), CHIP_H, None),
            Kind::Image { url, .. } => {
                let width = images.iter().find(|s| s.start == from).and_then(|s| if let FormatStyle::Image { width, .. } = s.style { Some(width as f32) } else { None }).unwrap_or(16.0);
                let w = width.min(content_w);
                let h = image(url).map_or(w, |i| if i.size.0 > 0 { w * i.size.1 as f32 / i.size.0 as f32 } else { w });
                (w, h, None)
            }
            Kind::Preedit { text, style } => {
                let s = shape(text, *style);
                (s.width(), 0.0, Some(s))
            }
            _ => (0.0, 0.0, None),
        };
        pieces.push(Piece { from, to, kind, shaped, w, h });
        at = to;
    }
    push_text(&mut pieces, at, units.len());

    // 2. Items with break opportunities: after a run of spaces, and around chips and images.
    let mut items: Vec<Item> = Vec::new();
    for (pi, p) in pieces.iter().enumerate() {
        match &p.kind {
            Kind::Text { text, .. } if text == "\n" => {
                items.push(Item { piece: pi, b0: 0, b1: 0, w: 0.0, hang: 0.0, break_after: true });
            }
            Kind::Text { text, .. } => {
                let shaped = p.shaped.as_ref().expect("shaped");
                let bytes = text.as_bytes();
                let mut start = 0;
                let mut i = 0;
                while i < bytes.len() {
                    if bytes[i] == b' ' {
                        let mut j = i;
                        while j < bytes.len() && bytes[j] == b' ' {
                            j += 1;
                        }
                        let w = shaped.x(j) - shaped.x(start);
                        let hang = shaped.x(j) - shaped.x(i);
                        items.push(Item { piece: pi, b0: start, b1: j, w, hang, break_after: true });
                        start = j;
                        i = j;
                    } else {
                        i += 1;
                    }
                }
                if start < bytes.len() {
                    items.push(Item { piece: pi, b0: start, b1: bytes.len(), w: shaped.x(bytes.len()) - shaped.x(start), hang: 0.0, break_after: false });
                }
            }
            Kind::Chip { .. } | Kind::Image { .. } => {
                if let Some(prev) = items.last_mut() {
                    prev.break_after = true;
                }
                items.push(Item { piece: pi, b0: 0, b1: 0, w: p.w, hang: 0.0, break_after: true });
            }
            Kind::Tab => items.push(Item { piece: pi, b0: 0, b1: 0, w: 0.0, hang: 0.0, break_after: false }),
            Kind::Preedit { .. } => items.push(Item { piece: pi, b0: 0, b1: 0, w: p.w, hang: 0.0, break_after: false }),
        }
    }

    // 3. Greedy line filling. A line is a list of item indices (split items allowed: an item
    // too wide for an empty line breaks between graphemes).
    let space_w = shape(" ", base).width();
    let tab_w = 4.0 * space_w;
    struct Piece2 {
        piece: usize,
        b0: usize,
        b1: usize,
    }
    let mut lines_items: Vec<(Vec<Piece2>, bool)> = Vec::new();
    let mut cur: Vec<Piece2> = Vec::new();
    let mut cur_w = 0.0f32;
    let mut k = 0;
    while k < items.len() {
        // A segment: items up to and including the next break opportunity.
        let mut end = k;
        while end < items.len() && !items[end].break_after {
            end += 1;
        }
        let end = end.min(items.len() - 1);
        let seg = &items[k..=end];
        // A line break inside the segment ends the line there.
        if let Some(nl) = seg.iter().position(|it| matches!(&pieces[it.piece].kind, Kind::Text { text, .. } if text == "\n")) {
            let nl = k + nl;
            for it in &items[k..nl] {
                cur.push(Piece2 { piece: it.piece, b0: it.b0, b1: it.b1 });
            }
            lines_items.push((std::mem::take(&mut cur), true));
            cur_w = 0.0;
            k = nl + 1;
            continue;
        }
        // Tabs take their width from where they land.
        let mut seg_w = 0.0;
        let mut x = cur_w;
        for it in seg {
            let w = if matches!(pieces[it.piece].kind, Kind::Tab) { ((x / tab_w).floor() + 1.0) * tab_w - x } else { it.w };
            x += w;
            seg_w += w;
        }
        let hang = seg.last().map_or(0.0, |it| it.hang);
        if cur_w + seg_w - hang <= content_w + 0.01 || cur.is_empty() && seg_w - hang <= content_w + 0.01 {
            for it in seg {
                cur.push(Piece2 { piece: it.piece, b0: it.b0, b1: it.b1 });
            }
            cur_w += seg_w;
            k = end + 1;
            continue;
        }
        if !cur.is_empty() {
            lines_items.push((std::mem::take(&mut cur), false));
            cur_w = 0.0;
            continue;
        }
        // Wider than a whole line: break anywhere, grapheme by grapheme.
        let mut w = 0.0;
        for it in seg {
            let p = &pieces[it.piece];
            let Some(shaped) = &p.shaped else {
                if w + it.w > content_w && !cur.is_empty() {
                    lines_items.push((std::mem::take(&mut cur), false));
                    w = 0.0;
                }
                cur.push(Piece2 { piece: it.piece, b0: it.b0, b1: it.b1 });
                w += it.w;
                continue;
            };
            let mut b0 = it.b0;
            let stops: Vec<usize> = shaped.stops.iter().map(|s| s.0).filter(|&b| b > it.b0 && b <= it.b1).collect();
            let mut prev = it.b0;
            for b in stops {
                let gw = shaped.x(b) - shaped.x(prev);
                let trailing_space = p_is_space(p, prev, b);
                if w + gw > content_w + 0.01 && w > 0.0 && !trailing_space {
                    cur.push(Piece2 { piece: it.piece, b0, b1: prev });
                    lines_items.push((std::mem::take(&mut cur), false));
                    w = 0.0;
                    b0 = prev;
                }
                w += gw;
                prev = b;
            }
            cur.push(Piece2 { piece: it.piece, b0, b1: it.b1 });
        }
        cur_w = w;
        k = end + 1;
    }
    lines_items.push((cur, true));

    // 4. Frags, positions and vertical metrics.
    let asc = strut_ascent(formatted);
    let desc = LINE_H - asc;
    let mut lines = Vec::new();
    let mut y = PAD_Y;
    let mut line_start = 0usize;
    for (parts, hard) in lines_items {
        let mut frags: Vec<Frag> = Vec::new();
        let mut x = PAD_X;
        // Merge consecutive slices of one text piece.
        let mut merged: Vec<Piece2> = Vec::new();
        for p in parts {
            if let Some(last) = merged.last_mut()
                && last.piece == p.piece
                && last.b1 == p.b0
                && pieces[p.piece].shaped.is_some()
                && !matches!(pieces[p.piece].kind, Kind::Preedit { .. })
            {
                last.b1 = p.b1;
                continue;
            }
            merged.push(p);
        }
        let (mut top_ext, mut bot_ext) = (asc, desc);
        for p in &merged {
            let piece = &pieces[p.piece];
            match piece.kind {
                Kind::Image { .. } => {
                    top_ext = top_ext.max(HALF_X + piece.h / 2.0);
                    bot_ext = bot_ext.max(piece.h / 2.0 - HALF_X);
                }
                // `vertical-align: 1px` on a 20 px inline-flex box.
                Kind::Chip { .. } => {
                    top_ext = top_ext.max(CHIP_BASELINE + 1.0);
                    bot_ext = bot_ext.max(CHIP_H - CHIP_BASELINE - 1.0);
                }
                _ => {}
            }
        }
        let height = top_ext + bot_ext;
        let baseline = y + top_ext;
        let mut first: Option<usize> = None;
        let mut last_pos = line_start;
        for p in merged {
            let piece = &pieces[p.piece];
            match &piece.kind {
                Kind::Text { text, style, underline, link } => {
                    if p.b0 == p.b1 {
                        continue;
                    }
                    let slice = &text[p.b0..p.b1];
                    let shaped = shape(slice, *style);
                    let base16 = piece.from + u16_at(text, p.b0);
                    let stops: Vec<(usize, f32)> = shaped.stops.iter().map(|&(b, sx)| (base16 + u16_at(slice, b), x + sx)).collect();
                    let w = shaped.width();
                    let (from, to) = (base16, base16 + slice.encode_utf16().count());
                    first.get_or_insert(from);
                    last_pos = to;
                    frags.push(Frag { from, to, x, w, y: baseline - asc, h: LINE_H, kind: Kind::Text { text: slice.to_string(), style: *style, underline: *underline, link: *link }, stops });
                    x += w;
                }
                Kind::Tab => {
                    let rel = x - PAD_X;
                    let w = ((rel / tab_w).floor() + 1.0) * tab_w - rel;
                    first.get_or_insert(piece.from);
                    last_pos = piece.to;
                    frags.push(Frag { from: piece.from, to: piece.to, x, w, y: baseline - asc, h: LINE_H, kind: Kind::Tab, stops: vec![(piece.from, x), (piece.to, x + w)] });
                    x += w;
                }
                Kind::Chip { .. } => {
                    first.get_or_insert(piece.from);
                    last_pos = piece.to;
                    let top = baseline - 1.0 - CHIP_BASELINE;
                    frags.push(Frag { from: piece.from, to: piece.to, x, w: piece.w, y: top, h: CHIP_H, kind: piece.kind.clone(), stops: vec![(piece.from, x), (piece.to, x + piece.w)] });
                    x += piece.w;
                }
                Kind::Image { .. } => {
                    first.get_or_insert(piece.from);
                    last_pos = piece.to;
                    let top = baseline - HALF_X - piece.h / 2.0;
                    frags.push(Frag { from: piece.from, to: piece.to, x, w: piece.w, y: top, h: piece.h, kind: piece.kind.clone(), stops: vec![(piece.from, x), (piece.to, x + piece.w)] });
                    x += piece.w;
                }
                Kind::Preedit { .. } => {
                    frags.push(Frag { from: piece.from, to: piece.to, x, w: piece.w, y: baseline - asc, h: LINE_H, kind: piece.kind.clone(), stops: Vec::new() });
                    x += piece.w;
                }
            }
        }
        let from = first.unwrap_or(line_start);
        let to = last_pos.max(from);
        lines.push(Line { top: y, height, baseline, from, to, frags, hard });
        y += height;
        // The next line starts after a hard break's newline, or where this one stopped.
        line_start = if hard { next_line_start(&units, to) } else { to };
    }
    // Empty lines carry no frags: give them their position.
    for l in lines.iter_mut() {
        if l.frags.iter().all(|f| matches!(f.kind, Kind::Preedit { .. })) && l.frags.iter().all(|f| f.stops.is_empty()) {
            l.to = l.from;
        }
    }
    Layout { height: y + PAD_Y, width: input.width, lines }
}

fn p_is_space(p: &Piece, b0: usize, b1: usize) -> bool {
    matches!(&p.kind, Kind::Text { text, .. } if text[b0..b1].bytes().all(|c| c == b' '))
}

fn next_line_start(units: &[u16], from: usize) -> usize {
    units[from.min(units.len())..].iter().position(|&u| u == b'\n' as u16).map_or(units.len(), |i| from + i + 1)
}

impl Layout {
    /// The visual line showing `pos`. At a soft wrap the caret belongs to the later line
    /// unless `before`.
    pub fn line_of(&self, pos: usize, before: bool) -> usize {
        let hits: Vec<usize> = (0..self.lines.len()).filter(|&i| self.lines[i].from <= pos && pos <= self.lines[i].to).collect();
        match (hits.first(), hits.last()) {
            (Some(&a), Some(&b)) => if before { a } else { b },
            _ => self.lines.iter().rposition(|l| l.from <= pos).unwrap_or(0),
        }
    }

    /// The caret's rect `[x, y, w, h]` (2 pt wide, the text's height on its line).
    pub fn caret(&self, pos: usize, before: bool) -> [f32; 4] {
        let li = self.line_of(pos, before);
        let l = &self.lines[li];
        let x = l.x_of(pos);
        let h = (SIZE * 1.2).round();
        [x, l.baseline - SIZE * 0.95, 2.0, h]
    }

    /// The document offset nearest a point in editor points.
    pub fn pos_at(&self, x: f32, y: f32) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let li = self.lines.iter().position(|l| y < l.top + l.height).unwrap_or(self.lines.len() - 1);
        self.lines[li].pos_at(x)
    }

    /// The chip or image under a point.
    pub fn atom_at(&self, x: f32, y: f32) -> Option<&Frag> {
        self.lines.iter().flat_map(|l| l.frags.iter()).find(|f| {
            matches!(f.kind, Kind::Chip { .. } | Kind::Image { .. })
                && x >= f.x + if matches!(f.kind, Kind::Chip { .. }) { CHIP_MARGIN } else { 0.0 }
                && x <= f.x + f.w - if matches!(f.kind, Kind::Chip { .. }) { CHIP_MARGIN } else { 0.0 }
                && y >= f.y
                && y <= f.y + f.h
        })
    }

    pub fn frag_from(&self, from: usize) -> Option<&Frag> {
        self.lines.iter().flat_map(|l| l.frags.iter()).find(|f| f.from == from && matches!(f.kind, Kind::Chip { .. } | Kind::Image { .. }))
    }

    /// One line up or down from `pos`, keeping `goal_x`.
    pub fn vertical(&self, pos: usize, goal_x: f32, down: bool, before: bool) -> Option<usize> {
        let li = self.line_of(pos, before);
        let next = if down { li.checked_add(1).filter(|&i| i < self.lines.len()) } else { li.checked_sub(1) }?;
        Some(self.lines[next].pos_at(goal_x))
    }

    /// The visual line's ends around `pos`.
    pub fn line_bounds(&self, pos: usize, before: bool) -> (usize, usize) {
        let l = &self.lines[self.line_of(pos, before)];
        (l.from, l.to)
    }

    /// Selection rectangles for `from..to`, as CodeMirror's selection layer draws them: line
    /// boxes from the start to the content's right edge, full middle lines, then to the end.
    pub fn selection_rects(&self, from: usize, to: usize) -> Vec<[f32; 4]> {
        let mut out = Vec::new();
        if from >= to {
            return out;
        }
        let right = self.width - PAD_X;
        for l in &self.lines {
            if to < l.from || from > l.to || (to == l.from && from < l.from) || (from == l.to && !l.hard && l.to != l.from) {
                continue;
            }
            let x0 = if from < l.from { PAD_X } else { l.x_of(from) };
            let x1 = if to > l.to { right } else { l.x_of(to) };
            if x1 > x0 {
                out.push([x0, l.top, x1 - x0, l.height]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lay(text: &str, spans: Option<&[FormatSpan]>, width: f32) -> Layout {
        layout(&Input { text, spans, width, snippet_name: &|_| None, preedit: None })
    }

    #[test]
    fn lines_cover_the_source_in_order() {
        let src = "Best regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}";
        let l = lay(src, None, 600.0);
        assert_eq!(l.lines.len(), 3);
        assert_eq!((l.lines[0].from, l.lines[0].to), (0, 13));
        assert_eq!((l.lines[1].from, l.lines[1].to), (14, 19));
        assert_eq!((l.lines[2].from, l.lines[2].to), (20, 50));
        assert!(matches!(&l.lines[2].frags[0].kind, Kind::Chip { label, .. } if label == "Date · Long date"));
        // Every source offset maps to a caret position and back.
        for pos in [0, 5, 13, 14, 19, 20, 50] {
            let li = l.line_of(pos, false);
            let x = l.lines[li].x_of(pos);
            assert_eq!(l.lines[li].pos_at(x), pos, "{pos}");
        }
        assert!(l.height > 2.0 * PAD_Y + 3.0 * LINE_H, "the chip's line is taller");
    }

    #[test]
    fn long_text_wraps_at_spaces_and_breaks_long_words() {
        let words = "lorem ipsum dolor sit amet ".repeat(8);
        let l = lay(&words, None, 300.0);
        assert!(l.lines.len() > 2);
        for w in l.lines.windows(2) {
            assert_eq!(w[0].to, w[1].from, "soft wraps share their boundary");
        }
        let long = "x".repeat(200);
        let l = lay(&long, None, 200.0);
        assert!(l.lines.len() > 2);
        assert_eq!(l.lines.last().unwrap().to, 200);
    }

    #[test]
    fn empty_lines_and_empty_doc() {
        let l = lay("", None, 400.0);
        assert_eq!(l.lines.len(), 1);
        let l = lay("a\n\nb", None, 400.0);
        assert_eq!(l.lines.len(), 3);
        assert_eq!((l.lines[1].from, l.lines[1].to), (2, 2));
        assert_eq!(l.pos_at(100.0, l.lines[1].top + 2.0), 2);
    }

    #[test]
    fn selection_spans_lines() {
        let l = lay("ab\ncd", None, 400.0);
        let r = l.selection_rects(1, 4);
        assert_eq!(r.len(), 2);
        assert!((r[0][0] + r[0][2] - (400.0 - PAD_X)).abs() < 0.01);
        assert!((r[1][0] - PAD_X).abs() < 0.01);
    }
}
