//! A CommonMark subset: emphasis, links, images, inline code, headings, lists, block quotes,
//! fenced code, rules, pipe tables and a few inline HTML tags.
//!
//! Line breaks inside a paragraph are kept as written (Kudzu content is plain text, not
//! reflowed HTML). Unsupported constructs keep their text and are named in the warning.

use std::collections::HashMap;

use super::{image, image_label, Dropped, RichText, MAX_INPUT};
use crate::formatting::{safe_link, FormatStyle};

/// Emphasis delimiter runs considered per inline block; the rest are literal text.
const MAX_DELIMITERS: usize = 2_000;
/// Longest inline HTML tag looked for.
const MAX_TAG: usize = 2_048;

pub(crate) fn read(source: &str) -> Result<RichText, String> {
    if source.len() > MAX_INPUT {
        return Err("it's too large to read its formatting".into());
    }
    let source = source.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = RichText::default();
    let blocks = blocks(&source, &mut out);
    let mut first = true;
    let mut gap = false;
    for block in blocks {
        match block {
            Block::Blank => {
                gap = true;
                continue;
            }
            Block::Break => continue,
            _ => {}
        }
        if !first {
            out.push(if gap { "\n\n" } else { "\n" });
        }
        first = false;
        gap = false;
        match block {
            Block::Para(text) => inline(&text, &mut out),
            Block::Heading(text) => {
                let start = out.len();
                inline(&text, &mut out);
                out.span(start, out.len(), FormatStyle::Bold);
            }
            Block::Item { marker, text } => {
                out.push(&marker);
                inline(&text, &mut out);
            }
            Block::Code(text) => out.push(&text),
            Block::Table(rows) => {
                for (r, row) in rows.iter().enumerate() {
                    if r > 0 {
                        out.push_char('\n');
                    }
                    for (c, cell) in row.iter().enumerate() {
                        if c > 0 {
                            out.push_char('\t');
                        }
                        inline(cell, &mut out);
                    }
                }
            }
            Block::Blank | Block::Break => {}
        }
    }
    Ok(out)
}

// MARK: - Blocks

enum Block {
    Para(String),
    Heading(String),
    Item { marker: String, text: String },
    Code(String),
    Table(Vec<Vec<String>>),
    Blank,
    /// A rule or fence: emits nothing, but ends the paragraph before it.
    Break,
}

fn indent_of(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn fence(line: &str) -> Option<(char, usize)> {
    let first = line.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = line.chars().take_while(|c| *c == first).count();
    (n >= 3).then_some((first, n))
}

fn thematic_break(line: &str) -> bool {
    let t: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && matches!(t[0], '*' | '-' | '_') && t.iter().all(|c| *c == t[0])
}

/// `-`, `*`, `+` or `1.`/`1)` followed by a space (or the end of the line): (marker, rest).
fn list_item(line: &str) -> Option<(String, &str)> {
    let mut chars = line.char_indices();
    let (_, first) = chars.next()?;
    let after_marker = |at: usize| -> Option<&str> {
        let rest = &line[at..];
        if rest.is_empty() {
            Some("")
        } else if rest.starts_with([' ', '\t']) {
            Some(rest.trim_start())
        } else {
            None
        }
    };
    if matches!(first, '-' | '*' | '+') {
        return after_marker(1).map(|rest| ("• ".to_string(), rest));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=9).contains(&digits) && matches!(line.as_bytes().get(digits), Some(b'.' | b')')) {
        return after_marker(digits + 1).map(|rest| (format!("{}. ", &line[..digits]), rest));
    }
    None
}

fn table_delimiter(line: &str) -> bool {
    let t = line.trim().trim_start_matches('|').trim_end_matches('|');
    !t.is_empty()
        && t.contains('-')
        && t.split('|').all(|cell| {
            let c = cell.trim();
            !c.is_empty() && c.trim_start_matches(':').trim_end_matches(':').chars().all(|ch| ch == '-')
        })
}

fn table_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').filter(|s| !s.ends_with('\\')).unwrap_or(t);
    let mut cells = vec![String::new()];
    let mut escaped = false;
    for c in t.chars() {
        if c == '|' && !escaped {
            cells.push(String::new());
        } else {
            cells.last_mut().expect("non-empty").push(c);
        }
        escaped = c == '\\' && !escaped;
    }
    cells.into_iter().map(|c| c.trim().to_string()).collect()
}

/// Removes a hard line break marker: two trailing spaces or a trailing backslash.
fn strip_hard_break(line: &str) -> &str {
    if line.ends_with("  ") {
        line.trim_end_matches(' ')
    } else if line.ends_with('\\') && !line.ends_with("\\\\") {
        &line[..line.len() - 1]
    } else {
        line
    }
}

fn blocks(source: &str, out: &mut RichText) -> Vec<Block> {
    let lines: Vec<&str> = source.split('\n').collect();
    let mut blocks: Vec<Block> = Vec::new();
    let mut code: Option<(char, usize, String)> = None;
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        i += 1;
        if let Some((c, n, buffer)) = &mut code {
            let t = raw.trim_start();
            if fence(t).is_some_and(|(fc, fnum)| fc == *c && fnum >= *n) && t.trim_start_matches(*c).trim().is_empty() {
                let text = std::mem::take(buffer);
                code = None;
                blocks.push(Block::Code(text));
                blocks.push(Block::Break);
            } else {
                if !buffer.is_empty() {
                    buffer.push('\n');
                }
                buffer.push_str(raw);
            }
            continue;
        }
        let indent = indent_of(raw);
        let mut line = raw.trim_start();
        let mut quoted = false;
        while let Some(rest) = line.strip_prefix('>') {
            quoted = true;
            line = rest.trim_start();
        }
        if quoted {
            out.drop(Dropped::BlockQuotes);
        }
        if line.trim().is_empty() {
            blocks.push(Block::Blank);
            continue;
        }
        if let Some((c, n)) = fence(line) {
            if c == '~' || !line[n..].contains('`') {
                out.drop(Dropped::Code);
                blocks.push(Block::Break);
                code = Some((c, n, String::new()));
                continue;
            }
        }
        let hashes = line.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && (line.len() == hashes || line[hashes..].starts_with([' ', '\t'])) {
            let text = line[hashes..].trim();
            let text = text.trim_end_matches('#').trim_end();
            out.drop(Dropped::Headings);
            blocks.push(Block::Heading(text.to_string()));
            continue;
        }
        let trimmed = line.trim_end();
        // Setext heading underline: `===` or `---` right below a paragraph line.
        if !trimmed.is_empty()
            && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
            && matches!(blocks.last(), Some(Block::Para(_)))
        {
            if let Some(Block::Para(text)) = blocks.pop() {
                out.drop(Dropped::Headings);
                blocks.push(Block::Heading(text));
            }
            continue;
        }
        if thematic_break(line) {
            out.drop(Dropped::Rules);
            blocks.push(Block::Break);
            continue;
        }
        if line.contains('|') && lines.get(i).is_some_and(|next| table_delimiter(next)) {
            out.drop(Dropped::Tables);
            let mut rows = vec![table_cells(line)];
            i += 1;
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(table_cells(lines[i]));
                i += 1;
            }
            blocks.push(Block::Table(rows));
            continue;
        }
        if let Some((marker, rest)) = list_item(line) {
            if indent >= 2 && !quoted {
                out.drop(Dropped::NestedLists);
            }
            blocks.push(Block::Item { marker, text: strip_hard_break(rest).to_string() });
            continue;
        }
        let text = strip_hard_break(line);
        match blocks.last_mut() {
            Some(Block::Para(para)) | Some(Block::Item { text: para, .. }) => {
                para.push('\n');
                para.push_str(text);
            }
            _ => blocks.push(Block::Para(text.to_string())),
        }
    }
    if let Some((_, _, buffer)) = code {
        blocks.push(Block::Code(buffer));
    }
    blocks
}

// MARK: - Inline

/// Inline content after emphasis is resolved: text and the edges of styled ranges.
enum Piece {
    Text(String),
    Open(usize),
    Close(usize),
}

enum Item {
    Piece(Piece),
    Delim { ch: char, count: usize, orig: usize, open: bool, close: bool },
}

/// Style table shared by an inline block and the link text nested in it; `None` keeps text only.
type Styles = Vec<Option<FormatStyle>>;

fn inline(source: &str, out: &mut RichText) {
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
            '\\' if chars.get(i + 1).is_some_and(|next| ESCAPABLE.contains(*next)) => {
                text.push(chars[i + 1]);
                i += 2;
            }
            // An Espanso reference is copied whole, so `_` in a variable name is never emphasis.
            '{' if chars.get(i + 1) == Some(&'{') => {
                let close = match next_close {
                    Some(Some(c)) if c >= i + 2 => Some(c),
                    Some(None) => None,
                    _ => {
                        let found = (i + 2..n.saturating_sub(1)).find(|&k| chars[k] == '}' && chars[k + 1] == '}');
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
                        let inner = match inner.strip_prefix(' ').and_then(|s| s.strip_suffix(' ')) {
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
                    let left = !after.is_whitespace() && (!is_punct(after) || before.is_whitespace() || is_punct(before));
                    let right = !before.is_whitespace() && (!is_punct(before) || after.is_whitespace() || is_punct(after));
                    let (open, close) = if c == '_' {
                        (left && (!right || is_punct(before)), right && (!left || is_punct(after)))
                    } else {
                        (left, right)
                    };
                    flush(&mut text, &mut items);
                    items.push(Item::Delim { ch: c, count: run, orig: run, open, close });
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
                    && (lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
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
                } else if !tag_name.is_empty() && lower.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/') {
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
                        if let Some(pos) = html_stack.iter().rposition(|(name, _)| *name == tag_name) {
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
        while let Item::Delim { ch, count, orig, close: true, open: c_open } = items[c] {
            if count == 0 {
                break;
            }
            let found = (0..ci).rev().find(|&oi| match items[delims[oi]] {
                Item::Delim { ch: och, count: ocount, orig: oorig, open: true, close: o_close } => {
                    och == ch
                        && ocount > 0
                        && (ch != '~' || (ocount >= 2 && count >= 2))
                        && !((o_close || c_open) && (oorig + orig) % 3 == 0 && !(oorig % 3 == 0 && orig % 3 == 0))
                }
                _ => false,
            });
            let Some(oi) = found else { break };
            let o = delims[oi];
            let Item::Delim { count: ocount, .. } = items[o] else { break };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn render(source: &str) -> (String, Vec<(String, FormatStyle)>) {
        let rich = read(source).unwrap();
        let mut spans: Vec<_> = rich
            .spans
            .iter()
            .map(|(r, s)| (rich.text[r.clone()].to_string(), s.clone()))
            .collect();
        spans.sort();
        (rich.text, spans)
    }

    #[test]
    fn emphasis_rules() {
        assert_eq!(render("a * b * c").1, vec![]);
        assert_eq!(render("snake_case_name").1, vec![]);
        assert_eq!(render("**a *b* c**").1, vec![("a b c".into(), FormatStyle::Bold), ("b".into(), FormatStyle::Italic)]);
        assert_eq!(render("*a **b** c*").1, vec![("a b c".into(), FormatStyle::Italic), ("b".into(), FormatStyle::Bold)]);
        assert_eq!(render("**unclosed").0, "**unclosed");
        assert_eq!(render("2 * 3 = 6 and 4*5").0, "2 * 3 = 6 and 4*5");
        assert_eq!(render("_a_b").0, "_a_b");
        assert_eq!(render(r"\*not\*").0, "*not*");
    }

    #[test]
    fn links_and_inline_html() {
        let (text, spans) = render("[**x**](https://a.b) <u>u</u> <span>s</span><br>end");
        assert_eq!(text, "x u s\nend");
        assert_eq!(
            spans,
            vec![
                ("u".into(), FormatStyle::Underline),
                ("x".into(), FormatStyle::Bold),
                ("x".into(), FormatStyle::Link { url: "https://a.b".into() }),
            ]
        );
        assert_eq!(render("[a] (b) [c](d e)").0, "[a] (b) c");
        assert_eq!(render("a < b > c").0, "a < b > c");
    }

    #[test]
    fn code_is_literal() {
        assert_eq!(render("`**x**` ``a`b``").0, "**x** a`b");
        assert_eq!(render("```\n**x**\n```\nafter").0, "**x**\nafter");
    }
}
