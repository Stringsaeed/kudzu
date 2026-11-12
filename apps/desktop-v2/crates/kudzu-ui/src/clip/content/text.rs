//! Text and code bodies. Text is selectable and wraps; code is mono with line numbers and
//! scrolls sideways. Both mark search matches, draw at most [`RENDER_LIMIT`] characters or
//! [`CODE_LINE_LIMIT`] lines, and say what they left out.

use rune::{col, row, text, Align, El, Span, TextStyle};

use super::{well, Body, CODE_LINE_LIMIT, RENDER_LIMIT};
use crate::clip::list::highlight;
use crate::theme::Theme;

fn spans(theme: &Theme, shown: &str, terms: &[String]) -> Vec<Span> {
    highlight(shown, terms)
        .into_iter()
        .map(|(s, e)| Span::new(s, e).color(theme.c.iris_foreground))
        .collect()
}

fn note(theme: &Theme, line: String) -> El {
    text(line).font(theme.ty.control_xs).color(theme.c.faint)
}

/// The first `limit` characters of `s` and how many characters were left out.
pub fn cut(s: &str, limit: usize) -> (&str, usize) {
    match s.char_indices().nth(limit) {
        Some((at, _)) => (&s[..at], s[at..].chars().count()),
        None => (s, 0),
    }
}

/// "…and 1,234 more characters. Copy it to get everything."
pub fn more_characters(hidden: usize) -> String {
    format!(
        "…and {} more characters. Copy it to get everything.",
        group(hidden)
    )
}

fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn view(theme: &Theme, b: &Body) -> El {
    let c = &theme.c;
    let source = b.full.or(b.entry.text.as_deref()).unwrap_or("");
    let (shown, hidden) = cut(source, RENDER_LIMIT);
    let size = if b.large { 15.0 } else { 13.5 };
    let style = TextStyle {
        line_height: size * 1.625,
        ..TextStyle::new(size, 400.0)
    };
    let mut body = text(shown.to_owned())
        .key("clip-text")
        .font(style)
        .color(c.foreground.alpha(0.9))
        .wrap_anywhere()
        .selectable(true)
        .w_full();
    let marks = spans(theme, shown, b.terms);
    if !marks.is_empty() {
        body = body.spans(marks);
    }
    let mut column = col().key("clip-text-body").w_full().child(body);
    if b.full.is_none() && b.entry.text_len > source.chars().count() {
        // RUNE-GAP(Beat has no background-position): a still bar for the shimmer.
        column = column.child(
            row()
                .key("clip-text-loading")
                .w(64.0)
                .h(12.0)
                .mt(6.0)
                .radius(4.0)
                .fill(c.foreground.alpha(0.1)),
        );
    }
    if hidden > 0 {
        column = column.child(note(theme, more_characters(hidden)).mt(12.0));
    }
    column
}

pub fn code(theme: &Theme, b: &Body) -> El {
    let c = &theme.c;
    let source = b.full.or(b.entry.text.as_deref()).unwrap_or("");
    let lines: Vec<&str> = source.split('\n').collect();
    let shown = &lines[..lines.len().min(CODE_LINE_LIMIT)];
    let size = if b.large { 13.5 } else { 12.5 };
    let style = TextStyle {
        line_height: size * 1.65,
        ..TextStyle::new(size, 400.0).family(theme.ty.mono.family)
    };
    let joined = shown.join("\n");
    let numbers: String = (1..=shown.len())
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    let digits = shown.len().to_string().len() as f32;
    let mut code = text(joined.clone())
        .key("clip-code")
        .font(style)
        .color(c.foreground.alpha(0.9))
        .selectable(true);
    let marks = spans(theme, &joined, b.terms);
    if !marks.is_empty() {
        code = code.spans(marks);
    }
    let gutter = text(numbers)
        .font(style)
        .color(c.faint.alpha(0.7))
        .align(Align::End)
        .a11y_hidden()
        .w(digits * size * 0.62 + 24.0)
        .pl(12.0)
        .pr(16.0);
    let mut block = well(theme)
        .key("clip-code-body")
        .py(12.0)
        .child(row().w_full().scroll_x().child(gutter).child(code.pr(16.0)));
    if lines.len() > shown.len() {
        block = block.child(
            note(
                theme,
                format!("…and {} more lines.", group(lines.len() - shown.len())),
            )
            .px(12.0)
            .pt(8.0),
        );
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_counts_characters_not_bytes() {
        assert_eq!(cut("héllo", 2), ("hé", 3));
        assert_eq!(cut("abc", 5), ("abc", 0));
        assert_eq!(cut("😀😀😀", 1).1, 2);
    }

    #[test]
    fn the_truncation_note_groups_thousands() {
        assert_eq!(
            more_characters(1_234_567),
            "…and 1,234,567 more characters. Copy it to get everything."
        );
        assert_eq!(group(999), "999");
    }
}
