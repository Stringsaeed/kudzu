//! A CommonMark subset: emphasis, links, images, inline code, headings, lists, block quotes,
//! fenced code, rules, pipe tables and a few inline HTML tags.
//!
//! Line breaks inside a paragraph are kept as written (Kudzu content is plain text, not
//! reflowed HTML). Unsupported constructs keep their text and are named in the warning.

mod inline;

use super::{Dropped, RichText, MAX_INPUT};
use crate::formatting::FormatStyle;

use inline::inline;

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
    Item {
        marker: String,
        text: String,
    },
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
            !c.is_empty()
                && c.trim_start_matches(':')
                    .trim_end_matches(':')
                    .chars()
                    .all(|ch| ch == '-')
        })
}

fn table_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t
        .strip_suffix('|')
        .filter(|s| !s.ends_with('\\'))
        .unwrap_or(t);
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
            if fence(t).is_some_and(|(fc, fnum)| fc == *c && fnum >= *n)
                && t.trim_start_matches(*c).trim().is_empty()
            {
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
        if (1..=6).contains(&hashes)
            && (line.len() == hashes || line[hashes..].starts_with([' ', '\t']))
        {
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
            blocks.push(Block::Item {
                marker,
                text: strip_hard_break(rest).to_string(),
            });
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

#[cfg(test)]
mod tests;
