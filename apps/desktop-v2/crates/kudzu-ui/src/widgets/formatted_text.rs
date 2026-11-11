//! A rendered snippet as the previews show it (`components/app/formatted-text.tsx`): the text with
//! its formatting (bold, italic, underline, links in iris), inline images, the cursor marker
//! (a 2 point iris bar) and key and pause badges where the insertion presses a key or waits.
//! Built as one Rune rich text, so it wraps as the web's inline flow does: badges are pills
//! (keycaps on `foreground/6%`, a pause in faint ink on a well), the cursor a hair-space pill
//! in iris, a repeated key adds a faint `×N`.
//!
//! An image draws as the image when the caller can decode it (`image`: data URL to
//! [`Pixels`], from the preview's image cache), else as a pill with an image glyph and its alt
//! text, the editor's chip look (D2).

use rune::{rich, Color, El, InlineIcon, PillStyle, Pixels, Span, TextStyle};

use kudzu_core::formatting::{safe_image, safe_link, FormatSpan, FormatStyle};
use kudzu_core::template::{ActionKind, Key, RenderResult};

use crate::theme::{icons, Theme};

/// One run of text between formatting, cursor and action boundaries (`formattedParts`), as
/// UTF-16 offsets into the rendered text.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub start: usize,
    pub end: usize,
    /// The styles covering the whole run.
    pub styles: Vec<FormatStyle>,
    /// The image span this run is inside, if any (its start, end, url and width).
    pub image: Option<(usize, usize, String, u32)>,
}

/// Spans that apply: inside the text, non-empty, with safe links and images.
fn valid(len: usize, spans: &[FormatSpan]) -> Vec<&FormatSpan> {
    spans
        .iter()
        .filter(|s| {
            s.start < s.end
                && s.end <= len
                && match &s.style {
                    FormatStyle::Link { url } => safe_link(url),
                    FormatStyle::Image { data_url, width } => {
                        safe_image(data_url) && (1..=512).contains(width)
                    }
                    _ => true,
                }
        })
        .collect()
}

/// `formattedParts`: the text cut at every span edge, the cursor and each break.
pub fn parts(
    len16: usize,
    spans: &[FormatSpan],
    cursor: Option<usize>,
    breaks: &[usize],
) -> Vec<Part> {
    let valid = valid(len16, spans);
    let mut cuts = vec![0, len16];
    for s in &valid {
        cuts.extend([s.start, s.end]);
    }
    cuts.extend(
        breaks
            .iter()
            .chain(cursor.as_ref())
            .filter(|at| **at <= len16),
    );
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .map(|w| Part {
            start: w[0],
            end: w[1],
            styles: valid
                .iter()
                .filter(|s| s.start <= w[0] && s.end >= w[1])
                .map(|s| s.style.clone())
                .collect(),
            image: valid.iter().find_map(|s| match &s.style {
                FormatStyle::Image { data_url, width } if s.start <= w[0] && s.end > w[0] => {
                    Some((s.start, s.end, data_url.clone(), *width))
                }
                _ => None,
            }),
        })
        .collect()
}

/// What the keycap shows and a reader hears for `{{key}}` (`KEYS` in `lib/template.ts`).
pub fn key_cap(key: Key) -> (&'static str, &'static str) {
    match key {
        Key::Tab => ("⇥", "Tab"),
        Key::Return => ("↵", "Return"),
        Key::Escape => ("esc", "Escape"),
        Key::Space => ("Space", "Space"),
        Key::Backspace => ("⌫", "Backspace"),
        Key::Delete => ("⌦", "Forward delete"),
        Key::Left => ("←", "Left"),
        Key::Right => ("→", "Right"),
        Key::Up => ("↑", "Up"),
        Key::Down => ("↓", "Down"),
    }
}

/// `formatDelay`: "300 ms", "1.5 s".
pub fn format_delay(ms: u32) -> String {
    if ms < 1000 {
        return format!("{ms} ms");
    }
    let s = format!("{:.2}", ms as f64 / 1000.0);
    format!("{} s", s.trim_end_matches('0').trim_end_matches('.'))
}

/// Byte offset of UTF-16 offset `at` in `text` (clamped to the end).
fn byte_at(text: &str, at: usize) -> usize {
    let mut units = 0;
    for (b, c) in text.char_indices() {
        if units >= at {
            return b;
        }
        units += c.len_utf16();
    }
    text.len()
}

/// Decodes an image's data URL for drawing, when the caller has a decoder.
pub type ImageSource<'a> = &'a dyn Fn(&str) -> Option<Pixels>;

/// The preview's text: `result` in `style` and `ink`.
pub fn formatted_text(
    theme: &Theme,
    result: &RenderResult,
    style: TextStyle,
    ink: Color,
    image: Option<ImageSource>,
) -> El {
    let c = &theme.c;
    let text = &result.text;
    let len16 = text.encode_utf16().count();
    let breaks: Vec<usize> = result.actions.iter().map(|a| a.offset).collect();
    let all = parts(
        len16,
        result.formatting.as_deref().unwrap_or(&[]),
        result.cursor_offset,
        &breaks,
    );
    let mut out = String::new();
    let (mut spans, mut pills, mut icons_at) = (Vec::new(), Vec::new(), Vec::new());
    let small = TextStyle::new(10.5, 500.0).lh(10.5);
    let marks = |at: usize, out: &mut String, spans: &mut Vec<Span>, pills: &mut Vec<_>| {
        for a in result.actions.iter().filter(|a| a.offset == at) {
            let start = out.len();
            match a.action {
                ActionKind::Pause { ms } => {
                    out.push_str(&format_delay(ms));
                    pills.push((
                        start..out.len(),
                        PillStyle::new().bg(c.foreground.alpha(0.025)).fg(c.faint),
                    ));
                    spans.push(Span::new(start, out.len()).size(small.size).weight(500.0));
                }
                ActionKind::Key { key, count } => {
                    out.push_str(key_cap(key).0);
                    pills.push((
                        start..out.len(),
                        PillStyle::new()
                            .bg(c.foreground.alpha(0.06))
                            .fg(c.muted_foreground),
                    ));
                    spans.push(Span::new(start, out.len()).size(small.size).weight(500.0));
                    if count > 1 {
                        let n = out.len();
                        out.push_str(&format!("×{count}"));
                        spans.push(
                            Span::new(n, out.len())
                                .size(small.size)
                                .weight(500.0)
                                .color(c.faint),
                        );
                    }
                }
            }
            out.push('\u{2009}');
        }
    };
    let cursor = |at: usize, out: &mut String, pills: &mut Vec<_>| {
        if result.cursor_offset == Some(at) {
            let start = out.len();
            out.push('\u{200A}');
            pills.push((start..out.len(), PillStyle::new().bg(c.iris).fg(c.iris)));
        }
    };
    for p in &all {
        marks(p.start, &mut out, &mut spans, &mut pills);
        cursor(p.start, &mut out, &mut pills);
        let (from, to) = (byte_at(text, p.start), byte_at(text, p.end));
        if let Some((start, end, url, width)) = &p.image {
            if *start != p.start {
                continue;
            }
            let alt = &text[byte_at(text, *start)..byte_at(text, *end)];
            match image.and_then(|decode| decode(url)) {
                Some(px) => {
                    icons_at.push((
                        out.len(),
                        InlineIcon::image(px).size(*width as f32).gap(0.0),
                    ));
                    out.push('\u{00A0}');
                }
                None => {
                    let at = out.len();
                    out.push_str(if alt.trim().is_empty() { "Image" } else { alt });
                    pills.push((
                        at..out.len(),
                        PillStyle::new()
                            .bg(c.foreground.alpha(0.06))
                            .fg(c.muted_foreground)
                            .icon(&icons::IMAGE),
                    ));
                }
            }
            continue;
        }
        let at = out.len();
        out.push_str(&text[from..to]);
        let mut span = Span::new(at, out.len());
        let mut styled = false;
        for s in &p.styles {
            styled = true;
            span = match s {
                FormatStyle::Bold => span.weight(700.0),
                FormatStyle::Italic => span.italic(),
                FormatStyle::Underline => span.underline(ink),
                FormatStyle::Link { .. } => span.color(c.iris).underline(c.iris),
                FormatStyle::Image { .. } => span,
            };
        }
        if styled && at < out.len() {
            spans.push(span);
        }
    }
    marks(len16, &mut out, &mut spans, &mut pills);
    cursor(len16, &mut out, &mut pills);
    let mut e = rich(out, spans).font(style).color(ink).wrap().pills(pills);
    for (at, i) in icons_at {
        e = e.inline_icon(at, i);
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::template::RenderAction;

    fn span(start: usize, end: usize, style: FormatStyle) -> FormatSpan {
        FormatSpan { start, end, style }
    }

    #[test]
    fn parts_cut_at_spans_cursor_and_actions() {
        let p = parts(11, &[span(0, 5, FormatStyle::Bold)], Some(8), &[6]);
        let cuts: Vec<(usize, usize)> = p.iter().map(|p| (p.start, p.end)).collect();
        assert_eq!(cuts, vec![(0, 5), (5, 6), (6, 8), (8, 11)]);
        assert_eq!(p[0].styles, vec![FormatStyle::Bold]);
        assert!(p[1].styles.is_empty());
    }

    #[test]
    fn unsafe_links_and_bad_spans_are_dropped() {
        let p = parts(
            4,
            &[
                span(
                    0,
                    2,
                    FormatStyle::Link {
                        url: "javascript:x".into(),
                    },
                ),
                span(3, 9, FormatStyle::Bold),
            ],
            None,
            &[],
        );
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn delays_read_as_the_web_writes_them() {
        assert_eq!(format_delay(300), "300 ms");
        assert_eq!(format_delay(1500), "1.5 s");
        assert_eq!(format_delay(2000), "2 s");
        assert_eq!(format_delay(1250), "1.25 s");
    }

    #[test]
    fn utf16_offsets_map_to_bytes() {
        let t = "a😀b";
        assert_eq!(byte_at(t, 1), 1);
        assert_eq!(byte_at(t, 3), 5);
        assert_eq!(byte_at(t, 9), 6);
    }

    #[test]
    fn badges_and_cursor_draw_inline() {
        let result = RenderResult {
            text: "Hi there".into(),
            formatting: Some(vec![span(0, 2, FormatStyle::Bold)]),
            cursor_offset: Some(8),
            diagnostics: Vec::new(),
            actions: vec![
                RenderAction {
                    offset: 2,
                    action: ActionKind::Key {
                        key: Key::Tab,
                        count: 2,
                    },
                },
                RenderAction {
                    offset: 2,
                    action: ActionKind::Pause { ms: 300 },
                },
            ],
        };
        fn view(r: &RenderResult, _: &mut rune::Cx<RenderResult>) -> El {
            let theme = Theme::light();
            formatted_text(theme, r, theme.ty.preview, theme.c.foreground, None)
        }
        let mut rt = rune::Runtime::new(result, view, (400.0, 100.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let shown: Vec<String> = rt
            .elements()
            .iter()
            .filter_map(|e| e.text.clone())
            .collect();
        assert!(
            shown
                .iter()
                .any(|t| t.starts_with("Hi⇥×2\u{2009}300 ms\u{2009} there\u{200A}")),
            "{shown:?}"
        );
    }
}
