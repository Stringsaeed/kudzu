//! Inline Markdown: code spans, emphasis, links and images inside one block.

use std::collections::HashMap;

use super::super::{image, image_label, Dropped, RichText};
use super::{MAX_DELIMITERS, MAX_TAG};
use crate::formatting::{safe_link, FormatStyle};

// MARK: - Inline

/// Inline content after emphasis is resolved: text and the edges of styled ranges.
enum Piece {
    Text(String),
    Open(usize),
    Close(usize),
}

enum Item {
    Piece(Piece),
    Delim {
        ch: char,
        count: usize,
        orig: usize,
        open: bool,
        close: bool,
    },
}

/// Style table shared by an inline block and the link text nested in it; `None` keeps text only.
type Styles = Vec<Option<FormatStyle>>;

pub(super) fn inline(source: &str, out: &mut RichText) {
    let mut styles: Styles = Vec::new();
    let pieces = parse_inline(source, out, &mut styles, true);
    let mut starts = vec![0; styles.len()];
    for piece in pieces {
        match piece {
            Piece::Text(t) => out.push(&t),
            Piece::Open(id) => starts[id] = out.len(),
            Piece::Close(id) => {
                if let Some(style) = styles[id].clone() {
                    out.span(starts[id], out.len(), style);
                }
            }
        }
    }
}

const ESCAPABLE: &str = "!\"#%&'()*+,-./:;<=>?@[]^_`~";

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation() || (!c.is_ascii() && !c.is_alphanumeric() && !c.is_whitespace())
}

/// Matching closer index for each opener in `chars` (`[`/`]` or `(`/`)`), skipping escapes.
fn matches(chars: &[char], open: char, close: char) -> HashMap<usize, usize> {
    let mut stack = Vec::new();
    let mut out = HashMap::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            c if c == open => stack.push(i),
            c if c == close => {
                if let Some(o) = stack.pop() {
                    out.insert(o, i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// For each backtick run start, the start of the next run of the same length.
fn backtick_closers(chars: &[char]) -> HashMap<usize, usize> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            let start = i;
            while i < chars.len() && chars[i] == '`' {
                i += 1;
            }
            runs.push((start, i - start));
        } else {
            i += 1;
        }
    }
    let mut next_of_len: HashMap<usize, usize> = HashMap::new();
    let mut out = HashMap::new();
    for &(start, len) in runs.iter().rev() {
        if let Some(&next) = next_of_len.get(&len) {
            out.insert(start, next);
        }
        next_of_len.insert(len, start);
    }
    out
}

fn link_destination(chars: &[char]) -> String {
    let s: String = chars.iter().collect();
    let s = s.trim();
    if let Some(rest) = s.strip_prefix('<') {
        return rest.split('>').next().unwrap_or("").to_string();
    }
    s.split_whitespace().next().unwrap_or("").to_string()
}

fn parse_inline(source: &str, out: &mut RichText, styles: &mut Styles, links: bool) -> Vec<Piece> {
    let chars: Vec<char> = source.chars().collect();
    let n = chars.len();
    let brackets = matches(&chars, '[', ']');
    let parens = matches(&chars, '(', ')');
    let ticks = backtick_closers(&chars);
    let mut items: Vec<Item> = Vec::new();
    let mut text = String::new();
    let mut delimiters = 0;
    let mut html_stack: Vec<(String, usize)> = Vec::new();
    let mut next_close: Option<Option<usize>> = None;
    let flush = |text: &mut String, items: &mut Vec<Item>| {
        if !text.is_empty() {
            items.push(Item::Piece(Piece::Text(std::mem::take(text))));
        }
    };
    let new_style = |styles: &mut Styles, style: Option<FormatStyle>| {
        styles.push(style);
        styles.len() - 1
    };

    let mut i = 0;
    while i < n {
        let c = chars[i];
        match c {
            '\\' if chars
                .get(i + 1)
                .is_some_and(|next| ESCAPABLE.contains(*next)) =>
            {
                text.push(chars[i + 1]);
                i += 2;
            }
            // An Espanso reference is copied whole, so `_` in a variable name is never emphasis.
            '{' if chars.get(i + 1) == Some(&'{') => {
                let close = match next_close {
                    Some(Some(c)) if c >= i + 2 => Some(c),
                    Some(None) => None,
                    _ => {
                        let found = (i + 2..n.saturating_sub(1))
                            .find(|&k| chars[k] == '}' && chars[k + 1] == '}');
                        next_close = Some(found);
                        found
                    }
                };
                let end = close.map_or(i + 2, |c| c + 2);
                text.extend(&chars[i..end]);
                i = end;
            }
            '`' => {
                let run = chars[i..].iter().take_while(|c| **c == '`').count();
                match ticks.get(&i) {
                    Some(&close) => {
                        let inner: String = chars[i + run..close].iter().collect();
                        let inner = inner.replace('\n', " ");
                        let inner = match inner.strip_prefix(' ').and_then(|s| s.strip_suffix(' '))
                        {
                            Some(stripped) if !inner.trim().is_empty() => stripped.to_string(),
                            _ => inner,
                        };
                        text.push_str(&inner);
                        out.drop(Dropped::Code);
                        i = close + run;
                    }
                    None => {
                        text.extend(&chars[i..i + run]);
                        i += run;
                    }
                }
            }
            '*' | '_' | '~' => {
                let run = chars[i..].iter().take_while(|x| **x == c).count();
                if (c == '~' && run < 2) || delimiters >= MAX_DELIMITERS {
                    text.extend(&chars[i..i + run]);
                } else {
                    delimiters += 1;
                    let before = if i == 0 { ' ' } else { chars[i - 1] };
                    let after = chars.get(i + run).copied().unwrap_or(' ');
                    let left = !after.is_whitespace()
                        && (!is_punct(after) || before.is_whitespace() || is_punct(before));
                    let right = !before.is_whitespace()
                        && (!is_punct(before) || after.is_whitespace() || is_punct(after));
                    let (open, close) = if c == '_' {
                        (
                            left && (!right || is_punct(before)),
                            right && (!left || is_punct(after)),
                        )
                    } else {
                        (left, right)
                    };
                    flush(&mut text, &mut items);
                    items.push(Item::Delim {
                        ch: c,
                        count: run,
                        orig: run,
                        open,
                        close,
                    });
                }
                i += run;
            }
            '!' | '[' => {
                let image = c == '!';
                let open = if image { i + 1 } else { i };
                let target = (image && chars.get(open) == Some(&'[') || !image && links)
                    .then(|| brackets.get(&open).copied())
                    .flatten()
                    .filter(|close| chars.get(close + 1) == Some(&'('))
                    .and_then(|close| parens.get(&(close + 1)).map(|paren| (close, *paren)));
                let Some((close, paren)) = target else {
                    text.push(c);
                    i += 1;
                    continue;
                };
                let destination = link_destination(&chars[close + 2..paren]);
                flush(&mut text, &mut items);
                if image {
                    let alt: String = chars[open + 1..close].iter().collect();
                    let data = image::data_url_bytes(&destination);
                    let style = out.image_style(data, None);
                    let label = image_label(&alt, style.is_some());
                    if !label.is_empty() {
                        let id = new_style(styles, style);
                        items.push(Item::Piece(Piece::Open(id)));
                        items.push(Item::Piece(Piece::Text(label)));
                        items.push(Item::Piece(Piece::Close(id)));
                    }
                } else {
                    let inner: String = chars[open + 1..close].iter().collect();
                    let style = if safe_link(&destination) {
                        Some(FormatStyle::Link { url: destination })
                    } else {
                        out.drop(Dropped::Links);
                        None
                    };
                    let id = new_style(styles, style);
                    let nested = parse_inline(&inner, out, styles, false);
                    items.push(Item::Piece(Piece::Open(id)));
                    items.extend(nested.into_iter().map(Item::Piece));
                    items.push(Item::Piece(Piece::Close(id)));
                }
                i = paren + 1;
            }
            '<' => {
                let end = (i + 1..n.min(i + MAX_TAG)).find(|&k| chars[k] == '>' || chars[k] == '<');
                let Some(end) = end.filter(|&k| chars[k] == '>') else {
                    text.push('<');
                    i += 1;
                    continue;
                };
                let inner: String = chars[i + 1..end].iter().collect();
                let lower = inner.to_ascii_lowercase();
                let autolink = !inner.contains(char::is_whitespace)
                    && (lower.starts_with("http://")
                        || lower.starts_with("https://")
                        || lower.starts_with("mailto:")
                        || (inner.contains('@') && !inner.contains(':')));
                let tag_name: String = lower
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
                if autolink && links {
                    let url = if lower.starts_with("http") || lower.starts_with("mailto:") {
                        inner.clone()
                    } else {
                        format!("mailto:{inner}")
                    };
                    flush(&mut text, &mut items);
                    let style = safe_link(&url).then_some(FormatStyle::Link { url });
                    let id = new_style(styles, style);
                    items.push(Item::Piece(Piece::Open(id)));
                    items.push(Item::Piece(Piece::Text(inner)));
                    items.push(Item::Piece(Piece::Close(id)));
                } else if !tag_name.is_empty()
                    && lower.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/')
                {
                    let closing = lower.starts_with('/');
                    let style = match tag_name.as_str() {
                        "b" | "strong" => Some(FormatStyle::Bold),
                        "i" | "em" => Some(FormatStyle::Italic),
                        "u" | "ins" => Some(FormatStyle::Underline),
                        _ => None,
                    };
                    flush(&mut text, &mut items);
                    if tag_name == "br" {
                        text.push('\n');
                    } else if style.is_some() && !closing {
                        let id = new_style(styles, style);
                        items.push(Item::Piece(Piece::Open(id)));
                        html_stack.push((tag_name, id));
                    } else if style.is_some() {
                        if let Some(pos) =
                            html_stack.iter().rposition(|(name, _)| *name == tag_name)
                        {
                            for (_, id) in html_stack.drain(pos..).rev() {
                                items.push(Item::Piece(Piece::Close(id)));
                            }
                        }
                    } else {
                        out.drop(Dropped::Html);
                    }
                } else {
                    text.push('<');
                    i += 1;
                    continue;
                }
                i = end + 1;
            }
            _ => {
                text.push(c);
                i += 1;
            }
        }
    }
    flush(&mut text, &mut items);
    for (_, id) in html_stack.into_iter().rev() {
        items.push(Item::Piece(Piece::Close(id)));
    }
    resolve_emphasis(items, out, styles)
}

/// CommonMark's delimiter matching, simplified: each closer pairs with the nearest compatible
/// opener; runs of two make bold, one italic, and `~~` strikethrough (dropped).
fn resolve_emphasis(mut items: Vec<Item>, out: &mut RichText, styles: &mut Styles) -> Vec<Piece> {
    let delims: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| matches!(item, Item::Delim { .. }))
        .map(|(i, _)| i)
        .collect();
    let mut opens: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut closes: HashMap<usize, Vec<usize>> = HashMap::new();
    for ci in 0..delims.len() {
        let c = delims[ci];
        while let Item::Delim {
            ch,
            count,
            orig,
            close: true,
            open: c_open,
        } = items[c]
        {
            if count == 0 {
                break;
            }
            let found = (0..ci).rev().find(|&oi| match items[delims[oi]] {
                Item::Delim {
                    ch: och,
                    count: ocount,
                    orig: oorig,
                    open: true,
                    close: o_close,
                } => {
                    och == ch
                        && ocount > 0
                        && (ch != '~' || (ocount >= 2 && count >= 2))
                        && !((o_close || c_open)
                            && (oorig + orig) % 3 == 0
                            && !(oorig % 3 == 0 && orig % 3 == 0))
                }
                _ => false,
            });
            let Some(oi) = found else { break };
            let o = delims[oi];
            let Item::Delim { count: ocount, .. } = items[o] else {
                break;
            };
            let used = if ocount >= 2 && count >= 2 { 2 } else { 1 };
            let style = match (ch, used) {
                ('~', _) => {
                    out.drop(Dropped::Strikethrough);
                    None
                }
                (_, 2) => Some(FormatStyle::Bold),
                _ => Some(FormatStyle::Italic),
            };
            styles.push(style);
            let id = styles.len() - 1;
            opens.entry(o).or_default().push(id);
            closes.entry(c).or_default().push(id);
            if let Item::Delim { count, .. } = &mut items[o] {
                *count -= used;
            }
            if let Item::Delim { count, .. } = &mut items[c] {
                *count -= used;
            }
            for &between in &delims[oi + 1..ci] {
                if let Item::Delim { open, close, .. } = &mut items[between] {
                    *open = false;
                    *close = false;
                }
            }
        }
    }
    let mut pieces = Vec::with_capacity(items.len());
    for (index, item) in items.into_iter().enumerate() {
        match item {
            Item::Piece(p) => pieces.push(p),
            Item::Delim { ch, count, .. } => {
                // Closers end before the leftover literal characters; openers start after them.
                for id in closes.remove(&index).unwrap_or_default().into_iter().rev() {
                    pieces.push(Piece::Close(id));
                }
                if count > 0 {
                    pieces.push(Piece::Text(std::iter::repeat_n(ch, count).collect()));
                }
                for id in opens.remove(&index).unwrap_or_default() {
                    pieces.push(Piece::Open(id));
                }
            }
        }
    }
    pieces
}
