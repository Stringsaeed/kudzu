//! An HTML fragment reader for the formatting Kudzu keeps: `b`/`strong`, `i`/`em`, `u`, links,
//! inline `style` weights and decorations, line breaks, paragraphs, lists and embedded images.
//!
//! Whitespace collapses as a browser would (except inside `pre`). Unknown or unclosed elements
//! are tolerated; markup that never closes its `<` is text.

use super::{image, image_label, Dropped, RichText, MAX_DEPTH, MAX_INPUT};
use crate::formatting::{safe_link, FormatStyle};

pub(crate) fn read(source: &str) -> Result<RichText, String> {
    if source.len() > MAX_INPUT {
        return Err("it's too large to read its formatting".into());
    }
    let mut reader = Reader::default();
    let mut rest = source;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.find("-->").map_or("", |end| &after[end + 3..]);
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            rest = rest.find('>').map_or("", |end| &rest[end + 1..]);
        } else if rest.starts_with('<')
            && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/')
        {
            let Some(end) = tag_end(rest) else {
                // Never closed: the rest is text.
                reader.text(&decode_entities(rest));
                break;
            };
            let tag = parse_tag(&rest[1..end]);
            rest = &rest[end + 1..];
            if tag.closing {
                reader.close(&tag.name);
            } else {
                if let Some(skip) = raw_text_end(&tag.name, rest) {
                    reader.skipped(&tag.name);
                    rest = &rest[skip..];
                    continue;
                }
                reader.open(&tag)?;
                if tag.self_closing {
                    reader.close(&tag.name);
                }
            }
        } else {
            let first = rest.chars().next().map_or(1, char::len_utf8);
            let end = rest[first..].find('<').map_or(rest.len(), |e| e + first);
            reader.text(&decode_entities(&rest[..end]));
            rest = &rest[end..];
        }
    }
    while !reader.stack.is_empty() {
        reader.pop();
    }
    Ok(reader.out)
}

/// The index of the `>` ending the tag at the start of `s`, respecting quoted attributes.
fn tag_end(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// Elements whose content is not text to keep: where their content ends (after the close tag).
fn raw_text_end(name: &str, rest: &str) -> Option<usize> {
    if !matches!(
        name,
        "script"
            | "style"
            | "head"
            | "title"
            | "template"
            | "noscript"
            | "svg"
            | "math"
            | "iframe"
            | "object"
            | "video"
            | "audio"
            | "canvas"
            | "select"
            | "textarea"
    ) {
        return None;
    }
    let close = format!("</{name}");
    let bytes = rest.as_bytes();
    let mut from = 0;
    while let Some(p) = rest[from..].find("</") {
        let at = from + p;
        if bytes
            .get(at..at + close.len())
            .is_some_and(|b| b.eq_ignore_ascii_case(close.as_bytes()))
        {
            return Some(rest[at..].find('>').map_or(rest.len(), |e| at + e + 1));
        }
        from = at + 2;
    }
    Some(rest.len())
}

struct Tag {
    name: String,
    closing: bool,
    self_closing: bool,
    attrs: Vec<(String, String)>,
}

impl Tag {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

fn parse_tag(body: &str) -> Tag {
    let closing = body.starts_with('/');
    let body = body.trim_start_matches('/');
    let self_closing = body.trim_end().ends_with('/');
    let name_len = body
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(body.len());
    let name = body[..name_len].to_ascii_lowercase();
    let mut attrs = Vec::new();
    let chars: Vec<char> = body[name_len..].chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && !matches!(chars[i], '=' | '/' | '>') {
            i += 1;
        }
        let key: String = chars[start..i]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let mut value = String::new();
        if chars.get(i) == Some(&'=') {
            i += 1;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if let Some(&q) = chars.get(i).filter(|c| **c == '"' || **c == '\'') {
                i += 1;
                let v = i;
                while i < chars.len() && chars[i] != q {
                    i += 1;
                }
                value = chars[v..i].iter().collect();
                i += 1;
            } else {
                let v = i;
                while i < chars.len() && !chars[i].is_whitespace() {
                    i += 1;
                }
                value = chars[v..i].iter().collect();
            }
        }
        if key.is_empty() {
            i += 1;
        } else {
            attrs.push((key, decode_entities(&value)));
        }
    }
    Tag {
        name,
        closing,
        self_closing,
        attrs,
    }
}

struct Open {
    name: String,
    /// Set when the element's first character is written, so leading breaks stay outside.
    start: Option<usize>,
    styles: Vec<FormatStyle>,
    breaks: usize,
    list: bool,
    pre: bool,
}

struct List {
    ordered: bool,
    next: i64,
}

#[derive(Default)]
struct Reader {
    out: RichText,
    stack: Vec<Open>,
    lists: Vec<List>,
    pending_space: bool,
    pending_break: usize,
    unstarted: usize,
    pre: usize,
    cell: usize,
}

const VOID: &[&str] = &[
    "br", "img", "hr", "input", "meta", "link", "wbr", "col", "area", "base", "embed", "source",
    "track", "param",
];

const BLOCKS: &[&str] = &[
    "div",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "main",
    "aside",
    "address",
    "figure",
    "figcaption",
    "center",
    "dl",
    "dt",
    "dd",
    "form",
    "fieldset",
    "blockquote",
    "ul",
    "ol",
    "li",
    "table",
    "tr",
    "caption",
    "pre",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "p",
];

impl Reader {
    fn at_line_start(&self) -> bool {
        self.out.text.is_empty() || self.out.text.ends_with('\n')
    }

    fn put(&mut self, c: char) {
        self.materialize();
        self.out.push_char(c);
    }

    /// Writes pending block breaks and collapsed whitespace, and starts elements that were
    /// waiting for their first character.
    fn materialize(&mut self) {
        if self.pending_break > 0 && !self.out.text.is_empty() {
            let have = self
                .out
                .text
                .chars()
                .rev()
                .take_while(|c| *c == '\n')
                .count();
            for _ in have..self.pending_break {
                self.out.push_char('\n');
            }
        }
        self.pending_break = 0;
        if self.pending_space && !self.at_line_start() && !self.out.text.ends_with([' ', '\t']) {
            self.out.push_char(' ');
        }
        self.pending_space = false;
        if self.unstarted > 0 {
            let at = self.out.len();
            for open in self.stack.iter_mut().filter(|o| o.start.is_none()) {
                open.start = Some(at);
            }
            self.unstarted = 0;
        }
    }

    fn put_str(&mut self, s: &str) {
        for c in s.chars() {
            self.put(c);
        }
    }

    fn text(&mut self, s: &str) {
        for c in s.chars() {
            if self.pre > 0 {
                self.put(c);
            } else if c.is_whitespace() && c != '\u{a0}' {
                self.pending_space = true;
            } else {
                self.put(c);
            }
        }
    }

    fn block_break(&mut self, n: usize) {
        self.pending_space = false;
        self.pending_break = self.pending_break.max(n);
    }

    fn skipped(&mut self, name: &str) {
        match name {
            "svg" | "canvas" => self.out.drop(Dropped::ImagesUnsupported),
            "iframe" | "object" | "video" | "audio" | "select" | "textarea" | "math" => {
                self.out.drop(Dropped::Html)
            }
            _ => {}
        }
    }

    fn style_attribute(&mut self, css: &str, styles: &mut Vec<FormatStyle>) {
        for declaration in css.split(';') {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let property = property.trim().to_ascii_lowercase();
            let value = value.trim().to_ascii_lowercase();
            match property.as_str() {
                "font-weight" => {
                    if value == "bold"
                        || value == "bolder"
                        || value.parse::<u32>().is_ok_and(|w| w >= 600)
                    {
                        styles.push(FormatStyle::Bold);
                    }
                }
                "font-style" if value.starts_with("italic") || value.starts_with("oblique") => {
                    styles.push(FormatStyle::Italic);
                }
                "text-decoration" | "text-decoration-line" => {
                    if value.contains("underline") {
                        styles.push(FormatStyle::Underline);
                    }
                    if value.contains("line-through") {
                        self.out.drop(Dropped::Strikethrough);
                    }
                }
                "color" => self.out.drop(Dropped::Colors),
                "background" | "background-color" => self.out.drop(Dropped::Highlights),
                "font-family" => self.out.drop(Dropped::Fonts),
                "font-size" => self.out.drop(Dropped::TextSizes),
                "font" => {
                    self.out.drop(Dropped::Fonts);
                    self.out.drop(Dropped::TextSizes);
                }
                "text-align"
                    if !matches!(value.as_str(), "left" | "start" | "initial" | "inherit") =>
                {
                    self.out.drop(Dropped::Alignment);
                }
                "vertical-align" if matches!(value.as_str(), "super" | "sub") => {
                    self.out.drop(Dropped::Scripts)
                }
                _ => {}
            }
        }
    }

    fn pop_to(&mut self, index: usize) {
        while self.stack.len() > index {
            self.pop();
        }
    }

    fn pop(&mut self) {
        let Some(open) = self.stack.pop() else { return };
        match open.start {
            Some(start) => {
                for style in open.styles {
                    self.out.span(start, self.out.len(), style);
                }
            }
            None => self.unstarted = self.unstarted.saturating_sub(1),
        }
        if open.list {
            self.lists.pop();
        }
        if open.pre {
            self.pre -= 1;
        }
        if open.breaks > 0 {
            self.block_break(open.breaks);
        }
    }

    fn close(&mut self, name: &str) {
        if let Some(index) = self.stack.iter().rposition(|o| o.name == name) {
            self.pop_to(index);
        }
    }

    /// Closes what HTML closes implicitly: an open `p` before a block, the previous `li`, cell or row.
    fn implicit_close(&mut self, name: &str) {
        let find = |stack: &[Open], target: &[&str], stop: &[&str]| {
            stack
                .iter()
                .rposition(|o| target.contains(&o.name.as_str()) || stop.contains(&o.name.as_str()))
                .filter(|&i| target.contains(&stack[i].name.as_str()))
        };
        let index = match name {
            "li" => find(&self.stack, &["li"], &["ul", "ol"]),
            "td" | "th" => find(&self.stack, &["td", "th"], &["tr", "table"]),
            "tr" => find(&self.stack, &["tr"], &["table"]),
            _ if BLOCKS.contains(&name) || name == "hr" => {
                (self.stack.last().is_some_and(|o| o.name == "p")).then(|| self.stack.len() - 1)
            }
            _ => None,
        };
        if let Some(index) = index {
            self.pop_to(index);
        }
    }

    fn open(&mut self, tag: &Tag) -> Result<(), String> {
        let name = tag.name.as_str();
        self.implicit_close(name);
        let mut styles = Vec::new();
        let mut breaks = 0;
        let mut list = false;
        let mut pre = false;
        match name {
            "b" | "strong" => styles.push(FormatStyle::Bold),
            "i" | "em" | "cite" | "var" | "dfn" => styles.push(FormatStyle::Italic),
            "u" | "ins" => styles.push(FormatStyle::Underline),
            "a" => match tag.attr("href").map(str::trim) {
                Some(url) if safe_link(url) => styles.push(FormatStyle::Link {
                    url: url.to_string(),
                }),
                Some(_) => self.out.drop(Dropped::Links),
                None => {}
            },
            "s" | "strike" | "del" => self.out.drop(Dropped::Strikethrough),
            "sup" | "sub" => self.out.drop(Dropped::Scripts),
            "code" | "kbd" | "samp" | "tt" => self.out.drop(Dropped::Code),
            "mark" => self.out.drop(Dropped::Highlights),
            "small" | "big" => self.out.drop(Dropped::TextSizes),
            "font" => {
                if tag.attr("face").is_some() {
                    self.out.drop(Dropped::Fonts);
                }
                if tag.attr("size").is_some() {
                    self.out.drop(Dropped::TextSizes);
                }
                if tag.attr("color").is_some() {
                    self.out.drop(Dropped::Colors);
                }
            }
            "br" => {
                self.pending_space = false;
                self.materialize();
                self.out.push_char('\n');
                return Ok(());
            }
            "hr" => {
                self.out.drop(Dropped::Rules);
                self.block_break(1);
                return Ok(());
            }
            "img" => {
                let data = tag
                    .attr("src")
                    .map_or(Err(Dropped::ImagesNotEmbedded), image::data_url_bytes);
                let width = tag
                    .attr("width")
                    .and_then(|w| w.trim().trim_end_matches("px").parse::<u32>().ok());
                let style = self.out.image_style(data, width);
                let label = image_label(tag.attr("alt").unwrap_or(""), style.is_some());
                if !label.is_empty() {
                    self.materialize();
                    let start = self.out.len();
                    self.out.push(&label);
                    if let Some(style) = style {
                        self.out.span(start, self.out.len(), style);
                    }
                }
                return Ok(());
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.out.drop(Dropped::Headings);
                styles.push(FormatStyle::Bold);
                breaks = 1;
            }
            "p" => breaks = 2,
            "pre" => {
                self.out.drop(Dropped::Code);
                breaks = 1;
                pre = true;
                self.pre += 1;
            }
            "blockquote" => {
                self.out.drop(Dropped::BlockQuotes);
                breaks = 1;
            }
            "center" => {
                self.out.drop(Dropped::Alignment);
                breaks = 1;
            }
            "ul" | "ol" => {
                breaks = 1;
                list = true;
                if !self.lists.is_empty() {
                    self.out.drop(Dropped::NestedLists);
                }
                let start = tag
                    .attr("start")
                    .and_then(|s| s.trim().parse::<i64>().ok())
                    .unwrap_or(1);
                self.lists.push(List {
                    ordered: name == "ol",
                    next: start,
                });
            }
            "li" => breaks = 1,
            "table" => {
                self.out.drop(Dropped::Tables);
                breaks = 1;
            }
            "tr" => {
                breaks = 1;
                self.cell = 0;
            }
            "td" | "th" => {}
            _ if VOID.contains(&name) => return Ok(()),
            _ if BLOCKS.contains(&name) => breaks = 1,
            "input" | "button" | "form" => self.out.drop(Dropped::Html),
            _ => {}
        }
        if let Some(css) = tag.attr("style") {
            let css = css.to_string();
            self.style_attribute(&css, &mut styles);
        }
        if let Some(align) = tag.attr("align") {
            if !align.eq_ignore_ascii_case("left") {
                self.out.drop(Dropped::Alignment);
            }
        }
        if breaks > 0 {
            self.block_break(breaks);
        }
        if name == "li" {
            let marker = match self.lists.last_mut() {
                Some(List {
                    ordered: true,
                    next,
                }) => {
                    *next += 1;
                    format!("{}. ", *next - 1)
                }
                _ => "• ".to_string(),
            };
            self.put_str(&marker);
        }
        if matches!(name, "td" | "th") {
            if self.cell > 0 {
                self.pending_space = false;
                self.put('\t');
            }
            self.cell += 1;
        }
        if self.stack.len() >= MAX_DEPTH {
            return Err("it's nested too deeply".into());
        }
        self.unstarted += 1;
        self.stack.push(Open {
            name: name.to_string(),
            start: None,
            styles,
            breaks,
            list,
            pre,
        });
        Ok(())
    }
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let decoded = after.find(';').filter(|&n| n <= 10).and_then(|n| {
            let entity = &after[..n];
            let named = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "trade" => Some('™'),
                "hellip" => Some('…'),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "lsquo" => Some('‘'),
                "rsquo" => Some('’'),
                "ldquo" => Some('“'),
                "rdquo" => Some('”'),
                "bull" => Some('•'),
                "middot" => Some('·'),
                "euro" => Some('€'),
                "pound" => Some('£'),
                "yen" => Some('¥'),
                "cent" => Some('¢'),
                "deg" => Some('°'),
                "times" => Some('×'),
                "divide" => Some('÷'),
                "laquo" => Some('«'),
                "raquo" => Some('»'),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32)
                    .filter(|c| *c != '\0'),
            };
            named.map(|c| (c, n))
        });
        match decoded {
            Some((c, n)) => {
                out.push(c);
                rest = &after[n + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
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
    fn whitespace_collapses_outside_pre() {
        assert_eq!(render("  a \n  <b> b </b>  c  ").0, "a b c");
        assert_eq!(render("<pre>a\n  b</pre>after").0, "a\n  b\nafter");
        assert_eq!(render("<p>one</p>\n<p>two</p>").0, "one\n\ntwo");
        assert_eq!(render("a<br>b<br/><br>c").0, "a\nb\n\nc");
    }

    #[test]
    fn spans_exclude_surrounding_breaks_and_spaces() {
        let (text, spans) = render("<p>x</p><p><b> bold </b></p>");
        assert_eq!(text, "x\n\nbold");
        assert_eq!(spans, vec![("bold".into(), FormatStyle::Bold)]);
    }

    #[test]
    fn implicit_closes_and_entities() {
        assert_eq!(render("<ul><li>a<li>b</ul>").0, "• a\n• b");
        assert_eq!(render("<p>a<p>b").0, "a\n\nb");
        assert_eq!(
            render("Tom &amp; Jerry &copy; &#x1F600; &bogus; &").0,
            "Tom & Jerry © 😀 &bogus; &"
        );
        assert_eq!(render("<!-- c --><!DOCTYPE html>x<?php ?>y").0, "xy");
        assert_eq!(render("<a href=\"javascript:x\">t</a>").1, vec![]);
    }
}
