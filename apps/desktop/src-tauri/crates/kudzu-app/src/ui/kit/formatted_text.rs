//! `components/app/formatted-text.tsx`: a rendered snippet as rich text — bold, italic,
//! underline and links (iris, underlined) — with inline images and the `{{cursor}}` caret
//! (a 2 pt iris bar).

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use kudzu_core::client::formatting::formatted_parts;
use kudzu_core::formatting::FormatStyle;
use kudzu_core::template::RenderResult;
use rune::{rich, Color, El, InlineIcon, Pixels, Span, TextStyle};

use crate::ui::theme::{text as ty, Theme};

/// U+200A HAIR SPACE: the caret's room (about 1 pt), which its 2 pt bar covers.
const CARET: &str = "\u{200A}";

/// U+2060 WORD JOINER: an invisible, zero-width "word" for an inline image or the caret to sit on.
const ANCHOR: &str = "\u{2060}";

/// The decoded pixels of an inline image, by data URL; decoded once, on a worker.
fn image(data_url: &str) -> Pixels {
    static CACHE: OnceLock<Mutex<HashMap<String, Pixels>>> = OnceLock::new();
    let mut map = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = map.get(data_url) {
        return p.clone();
    }
    let pixels = Pixels::new();
    map.insert(data_url.to_string(), pixels.clone());
    let (url, out) = (data_url.to_string(), pixels.clone());
    let _ = std::thread::Builder::new().name("kudzu-inline-image".into()).spawn(move || {
        let Some(b64) = url.split_once(";base64,").map(|(_, d)| d) else { return };
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(b64) else { return };
        let Ok(img) = image::load_from_memory(&bytes) else { return };
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        out.set(w, h, rgba.into_raw());
    });
    pixels
}

/// `<FormattedText result />` in `font` and `color` (the text around it), wrapping.
pub struct FormattedText<'a> {
    result: &'a RenderResult,
    font: TextStyle,
    color: Option<Color>,
}

impl<'a> FormattedText<'a> {
    pub fn new(result: &'a RenderResult) -> Self {
        FormattedText { result, font: ty::BASE, color: None }
    }
    pub fn font(mut self, f: TextStyle) -> Self {
        self.font = f;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let ink = self.color.unwrap_or(t.foreground);
        let r = self.result;
        let cursor = r.cursor_offset;
        let parts = formatted_parts(&r.text, r.formatting.as_deref().unwrap_or(&[]), cursor);
        let mut content = String::new();
        let mut spans = Vec::new();
        let mut carets = Vec::new();
        let mut images = Vec::new();
        let caret = |content: &mut String, carets: &mut Vec<usize>| {
            carets.push(content.len());
            content.push_str(CARET);
        };
        for part in &parts {
            if cursor == Some(part.start) {
                caret(&mut content, &mut carets);
            }
            if let Some(img) = &part.image {
                if let FormatStyle::Image { data_url, width } = &img.style {
                    // One image per span, at its first piece.
                    if part.start == img.start {
                        images.push((content.len(), data_url.clone(), *width as f32));
                        content.push_str(ANCHOR);
                    }
                }
                continue;
            }
            let start = content.len();
            content.push_str(&part.text);
            let end = content.len();
            if start == end {
                continue;
            }
            let link = part.styles.iter().any(|s| matches!(s, FormatStyle::Link { .. }));
            let mut span = Span::new(start, end);
            let mut styled = false;
            if part.styles.iter().any(|s| matches!(s, FormatStyle::Bold)) {
                span = span.weight(700.0);
                styled = true;
            }
            if part.styles.iter().any(|s| matches!(s, FormatStyle::Italic)) {
                span = span.italic();
                styled = true;
            }
            if link {
                span = span.color(t.iris);
                styled = true;
            }
            if link || part.styles.iter().any(|s| matches!(s, FormatStyle::Underline)) {
                span.underline = Some(if link { t.iris } else { ink });
                styled = true;
            }
            if styled {
                spans.push(span);
            }
        }
        if cursor == Some(rune_len16(&r.text)) {
            caret(&mut content, &mut carets);
        }
        let mut e = rich(content, spans).font(self.font).color(ink).wrap().w_full();
        for (at, url, width) in images {
            e = e.inline_icon(at, InlineIcon::image(image(&url)).size(width).gap(0.0));
        }
        // The caret: `w-[2px] rounded-full bg-iris`, on its zero-width anchor.
        // The caret's room is no text: copying skips it.
        for &at in &carets {
            e = e.uncopied(at..at + CARET.len());
        }
        let highlights: Vec<_> = carets.into_iter().map(|at| rune::TextHighlight::new(at..at + CARET.len(), t.iris).pad(0.5)).collect();
        if !highlights.is_empty() {
            e = e.text_highlights(highlights);
        }
        e
    }
}

/// The text's length in UTF-16 code units (the offsets the renderer reports).
fn rune_len16(s: &str) -> usize {
    s.encode_utf16().count()
}
