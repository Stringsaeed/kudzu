//! An RTF reader for the Cocoa RTF TextExpander stores in `extraInfo.RTF`: bold, italic,
//! underline, `HYPERLINK` fields, Cocoa list markers, line breaks, Unicode and code-page
//! characters, and `\pict` PNG/JPEG pictures.
//!
//! Fonts, sizes, colors, highlights, strikethrough, super/subscript, alignment and tables are
//! noted for the warning. A single font family and size for the whole snippet is not, since
//! every rich snippet has one and Kudzu pastes in the target app's font.

use std::collections::{BTreeSet, HashMap};

use super::{image_label, Dropped, RichText, MAX_DEPTH};
use crate::formatting::{safe_link, FormatStyle};

/// Largest RTF read for formatting; pictures are hex, so this is larger than for HTML.
const MAX_RTF: usize = 4 << 20;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dest {
    Text,
    FontTable,
    ColorTable,
    FieldInstruction,
    ListText,
    Picture,
    Attachment,
    Skip,
}

#[derive(Clone)]
struct State {
    dest: Dest,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    script: bool,
    aligned: bool,
    font: i64,
    size: i64,
    foreground: i64,
    background: i64,
    highlight: i64,
    unicode_skip: usize,
    list_level: i64,
    link: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            dest: Dest::Text,
            bold: false,
            italic: false,
            underline: false,
            strike: false,
            script: false,
            aligned: false,
            font: 0,
            size: 24,
            foreground: 0,
            background: 0,
            highlight: 0,
            unicode_skip: 1,
            list_level: 0,
            link: None,
        }
    }
}

#[derive(Default)]
struct Picture {
    kind: Option<bool>,
    goal_width: Option<i64>,
    nibbles: Vec<u8>,
    too_large: bool,
}

#[derive(Default)]
struct Reader {
    out: RichText,
    fonts: HashMap<i64, String>,
    font_definition: i64,
    font_name: String,
    colors: Vec<Option<(i64, i64, i64)>>,
    color: Option<(i64, i64, i64)>,
    default_font: i64,
    field_instruction: String,
    list_marker: String,
    picture: Picture,
    families: BTreeSet<String>,
    sizes: BTreeSet<i64>,
    skip: usize,
    high_surrogate: Option<u32>,
    skip_attachment_character: bool,
}

pub(crate) fn read(source: &str) -> Result<RichText, String> {
    if source.len() > MAX_RTF {
        return Err("its rich text is too large to read".into());
    }
    if !source.trim_start().starts_with("{\\rtf") {
        return Err("its rich text isn't RTF".into());
    }
    let chars: Vec<char> = source.trim_start().chars().collect();
    let mut reader = Reader { default_font: 0, ..Reader::default() };
    let mut stack: Vec<State> = Vec::new();
    let mut state = State::default();
    let mut star = false;
    let mut i = 0;
    let mut closed = false;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => {
                stack.push(state.clone());
                if stack.len() > MAX_DEPTH {
                    return Err("its rich text is nested too deeply".into());
                }
                reader.skip = 0;
                i += 1;
            }
            '}' => {
                let Some(parent) = stack.pop() else {
                    return Err("its rich text is malformed".into());
                };
                reader.end_group(&state, &parent);
                state = parent;
                reader.skip = 0;
                i += 1;
                if stack.is_empty() {
                    closed = true;
                    break;
                }
            }
            '\\' => {
                i += 1;
                let Some(&next) = chars.get(i) else { break };
                if next.is_ascii_alphabetic() {
                    let start = i;
                    while i < chars.len() && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word: String = chars[start..i.min(start + 32)].iter().collect();
                    let negative = chars.get(i) == Some(&'-') && chars.get(i + 1).is_some_and(char::is_ascii_digit);
                    if negative {
                        i += 1;
                    }
                    let digits = i;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    let param = (i > digits).then(|| {
                        let value: i64 = chars[digits..i.min(digits + 10)].iter().collect::<String>().parse().unwrap_or(0);
                        if negative { -value } else { value }
                    });
                    if chars.get(i) == Some(&' ') {
                        i += 1;
                    }
                    if word == "bin" {
                        let skip = usize::try_from(param.unwrap_or(0).max(0)).unwrap_or(0);
                        i = i.saturating_add(skip).min(chars.len());
                        continue;
                    }
                    let was_star = star;
                    star = false;
                    reader.word(&word, param, &mut state, was_star);
                } else {
                    i += 1;
                    match next {
                        '\'' => {
                            let hex: String = chars[i..(i + 2).min(chars.len())].iter().collect();
                            i = (i + 2).min(chars.len());
                            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                                if reader.skip > 0 {
                                    reader.skip -= 1;
                                } else {
                                    reader.emit(cp1252(byte), &state);
                                }
                            }
                        }
                        '*' => star = true,
                        '{' | '}' | '\\' => reader.character(next, &state),
                        '~' => reader.character('\u{a0}', &state),
                        '_' => reader.character('\u{2011}', &state),
                        '\n' | '\r' => reader.character('\n', &state),
                        _ => {}
                    }
                }
            }
            '\r' | '\n' => i += 1,
            _ => {
                if state.dest == Dest::Picture {
                    reader.picture_digit(c);
                } else {
                    reader.character(c, &state);
                }
                i += 1;
            }
        }
    }
    if !closed {
        return Err("its rich text is incomplete".into());
    }
    if reader.families.len() > 1 {
        reader.out.drop(Dropped::Fonts);
    }
    if reader.sizes.len() > 1 {
        reader.out.drop(Dropped::TextSizes);
    }
    Ok(reader.out)
}

fn cp1252(byte: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
        '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    if (0x80..0xA0).contains(&byte) {
        HIGH[usize::from(byte - 0x80)]
    } else {
        char::from(byte)
    }
}

/// `HYPERLINK "url"` (with optional switches) → the URL.
fn hyperlink(instruction: &str) -> Option<String> {
    let at = instruction.to_ascii_uppercase().find("HYPERLINK")?;
    let rest = instruction[at + "HYPERLINK".len()..].trim();
    let url = match rest.find('"') {
        Some(open) => {
            let after = &rest[open + 1..];
            &after[..after.find('"').unwrap_or(after.len())]
        }
        None => rest.split_whitespace().next().unwrap_or(""),
    };
    Some(url.trim().to_string())
}

/// Cocoa writes list markers as `{\listtext\t•\t}`; Kudzu keeps `• ` or `1. ` like its editor.
fn list_marker(text: &str) -> String {
    let t = text.trim();
    if t.is_empty() {
        return String::new();
    }
    let number = t.trim_end_matches(['.', ')']);
    if !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()) {
        return format!("{number}. ");
    }
    if t.chars().any(char::is_alphanumeric) {
        return format!("{t} ");
    }
    "• ".into()
}

impl Reader {
    fn word(&mut self, word: &str, param: Option<i64>, state: &mut State, ignorable: bool) {
        let on = param != Some(0);
        let value = param.unwrap_or(0);
        match word {
            "fonttbl" => state.dest = Dest::FontTable,
            "colortbl" => {
                state.dest = Dest::ColorTable;
                self.colors.clear();
                self.color = None;
            }
            "stylesheet" | "info" | "listtable" | "listoverridetable" | "revtbl" | "header" | "headerl"
            | "headerr" | "headerf" | "footer" | "footerl" | "footerr" | "footerf" | "footnote"
            | "nonshppict" | "themedata" | "colorschememapping" | "latentstyles" | "datastore"
            | "xmlnstbl" | "rsidtbl" | "generator" | "mmathPr" | "pgdsctbl" | "expandedcolortbl"
            | "fldtype" | "bkmkstart" | "bkmkend" | "annotation" | "atnid" | "atnauthor" => state.dest = Dest::Skip,
            "object" => {
                self.out.drop(Dropped::Html);
                state.dest = Dest::Skip;
            }
            "pict" => {
                state.dest = Dest::Picture;
                self.picture = Picture::default();
            }
            "pngblip" => self.picture.kind = Some(true),
            "jpegblip" => self.picture.kind = Some(false),
            "picwgoal" => self.picture.goal_width = param,
            "NeXTGraphic" => state.dest = Dest::Attachment,
            "field" => self.field_instruction.clear(),
            "fldinst" => state.dest = Dest::FieldInstruction,
            "fldrslt" => {
                state.link = hyperlink(&self.field_instruction).and_then(|url| {
                    if safe_link(&url) {
                        Some(url)
                    } else {
                        self.out.drop(Dropped::Links);
                        None
                    }
                });
            }
            "listtext" => {
                state.dest = Dest::ListText;
                self.list_marker.clear();
            }
            "par" | "line" | "sect" | "page" | "lbr" => self.character('\n', state),
            "row" => {
                self.out.drop(Dropped::Tables);
                self.character('\n', state);
            }
            "cell" | "nestcell" => {
                self.out.drop(Dropped::Tables);
                self.character('\t', state);
            }
            "trowd" => self.out.drop(Dropped::Tables),
            "tab" => self.character('\t', state),
            "emdash" => self.character('—', state),
            "endash" => self.character('–', state),
            "bullet" => self.character('•', state),
            "lquote" => self.character('‘', state),
            "rquote" => self.character('’', state),
            "ldblquote" => self.character('“', state),
            "rdblquote" => self.character('”', state),
            "emspace" | "enspace" | "qmspace" => self.character(' ', state),
            "u" => {
                let unit = u32::try_from(value.rem_euclid(65_536)).unwrap_or(0xFFFD);
                match unit {
                    0xD800..=0xDBFF => self.high_surrogate = Some(unit),
                    0xDC00..=0xDFFF => {
                        if let Some(high) = self.high_surrogate.take() {
                            let c = 0x10000 + ((high - 0xD800) << 10) + (unit - 0xDC00);
                            if let Some(c) = char::from_u32(c) {
                                self.emit(c, state);
                            }
                        }
                    }
                    _ => {
                        self.high_surrogate = None;
                        if let Some(c) = char::from_u32(unit) {
                            self.emit(c, state);
                        }
                    }
                }
                self.skip = state.unicode_skip;
            }
            "uc" => state.unicode_skip = usize::try_from(value.clamp(0, 8)).unwrap_or(1),
            "b" => state.bold = on,
            "i" => state.italic = on,
            "ulnone" => state.underline = false,
            w if w.starts_with("ul") && w != "ulc" && !w.starts_with("ulc") => state.underline = on,
            "strike" | "striked" => state.strike = on,
            "super" | "sub" => state.script = true,
            "nosupersub" => state.script = false,
            "up" | "dn" => state.script = param.unwrap_or(6) != 0,
            "plain" => {
                let keep = (state.dest, state.unicode_skip, state.link.clone(), state.aligned, state.list_level);
                *state = State {
                    dest: keep.0,
                    unicode_skip: keep.1,
                    link: keep.2,
                    aligned: keep.3,
                    list_level: keep.4,
                    font: self.default_font,
                    ..State::default()
                };
            }
            "pard" => {
                state.aligned = false;
                state.list_level = 0;
            }
            "qc" | "qr" | "qj" | "qd" => state.aligned = true,
            "ql" => state.aligned = false,
            "deff" => self.default_font = value,
            "f" => {
                if state.dest == Dest::FontTable {
                    self.font_definition = value;
                    self.font_name.clear();
                } else {
                    state.font = value;
                }
            }
            "fs" => state.size = value,
            "cf" => state.foreground = value,
            "cb" | "chcbpat" => state.background = value,
            "highlight" => state.highlight = value,
            "ilvl" => state.list_level = value,
            "red" | "green" | "blue" if state.dest == Dest::ColorTable => {
                let mut color = self.color.unwrap_or((0, 0, 0));
                match word {
                    "red" => color.0 = value,
                    "green" => color.1 = value,
                    _ => color.2 = value,
                }
                self.color = Some(color);
            }
            _ => {
                if ignorable {
                    state.dest = Dest::Skip;
                }
            }
        }
    }

    fn end_group(&mut self, child: &State, parent: &State) {
        match child.dest {
            Dest::ListText if parent.dest != Dest::ListText => {
                if child.list_level > 0 {
                    self.out.drop(Dropped::NestedLists);
                }
                let marker = list_marker(&self.list_marker);
                if parent.dest == Dest::Text {
                    self.out.push(&marker);
                }
            }
            Dest::Picture if parent.dest != Dest::Picture => {
                let picture = std::mem::take(&mut self.picture);
                let data = match picture.kind {
                    _ if picture.too_large => Err(Dropped::ImagesTooLarge),
                    Some(_) if picture.nibbles.len() >= 2 => Ok(picture
                        .nibbles
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|[high, low]| (high << 4) | low)
                        .collect()),
                    _ => Err(Dropped::ImagesUnsupported),
                };
                let width = picture
                    .goal_width
                    .filter(|w| *w > 0)
                    .and_then(|twips| u32::try_from((twips + 7) / 15).ok());
                let style = self.out.image_style(data, width);
                let label = image_label("", style.is_some());
                if parent.dest == Dest::Text && !label.is_empty() {
                    let start = self.out.len();
                    self.out.push(&label);
                    if let Some(style) = style {
                        self.out.span(start, self.out.len(), style);
                    }
                }
            }
            Dest::Attachment if parent.dest != Dest::Attachment => {
                self.out.drop(Dropped::ImagesNotEmbedded);
                self.skip_attachment_character = true;
            }
            Dest::FontTable if !self.font_name.trim().is_empty() => {
                let name = std::mem::take(&mut self.font_name);
                self.fonts.insert(self.font_definition, name.trim().to_string());
            }
            _ => {}
        }
    }

    fn picture_digit(&mut self, c: char) {
        if let Some(d) = c.to_digit(16) {
            if self.picture.nibbles.len() >= 2 * 196_608 {
                self.picture.too_large = true;
            } else {
                self.picture.nibbles.push(d as u8);
            }
        }
    }

    /// A literal character, unless it is a `\uN` fallback being skipped.
    fn character(&mut self, c: char, state: &State) {
        if self.skip > 0 {
            self.skip -= 1;
            return;
        }
        self.emit(c, state);
    }

    fn emit(&mut self, c: char, state: &State) {
        match state.dest {
            Dest::Text => {}
            Dest::FontTable => {
                if c == ';' {
                    let name = std::mem::take(&mut self.font_name);
                    self.fonts.insert(self.font_definition, name.trim().to_string());
                } else {
                    self.font_name.push(c);
                }
                return;
            }
            Dest::ColorTable => {
                if c == ';' {
                    self.colors.push(self.color.take());
                }
                return;
            }
            Dest::FieldInstruction => {
                if self.field_instruction.len() < 4096 {
                    self.field_instruction.push(c);
                }
                return;
            }
            Dest::ListText => {
                if self.list_marker.len() < 64 {
                    self.list_marker.push(c);
                }
                return;
            }
            Dest::Picture | Dest::Attachment | Dest::Skip => return,
        }
        if std::mem::take(&mut self.skip_attachment_character) && c == '¬' {
            return;
        }
        let start = self.out.len();
        self.out.push_char(c);
        let end = self.out.len();
        if !c.is_whitespace() {
            self.note_dropped(state);
        }
        if state.bold {
            self.out.span(start, end, FormatStyle::Bold);
        }
        if state.italic {
            self.out.span(start, end, FormatStyle::Italic);
        }
        match &state.link {
            Some(url) => self.out.span(start, end, FormatStyle::Link { url: url.clone() }),
            None if state.underline => self.out.span(start, end, FormatStyle::Underline),
            None => {}
        }
    }

    fn note_dropped(&mut self, state: &State) {
        let name = self.fonts.get(&state.font).map_or_else(|| state.font.to_string(), String::clone);
        let family = name.split('-').next().unwrap_or("").trim().to_string();
        self.families.insert(family);
        self.sizes.insert(state.size);
        let color = |index: i64| -> Option<(i64, i64, i64)> {
            usize::try_from(index).ok().filter(|i| *i > 0).and_then(|i| self.colors.get(i).copied().flatten())
        };
        // Link text is colored by the app that shows it, so its color isn't a loss.
        if state.link.is_none() && color(state.foreground).is_some_and(|c| c != (0, 0, 0)) {
            self.out.drop(Dropped::Colors);
        }
        if color(state.background).is_some_and(|c| c != (255, 255, 255)) || state.highlight > 0 {
            self.out.drop(Dropped::Highlights);
        }
        if state.strike {
            self.out.drop(Dropped::Strikethrough);
        }
        if state.script {
            self.out.drop(Dropped::Scripts);
        }
        if state.aligned {
            self.out.drop(Dropped::Alignment);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(source: &str) -> (String, Vec<(String, FormatStyle)>, Vec<Dropped>) {
        let rich = read(source).unwrap();
        let mut spans: Vec<_> = rich
            .spans
            .iter()
            .map(|(r, s)| (rich.text[r.clone()].to_string(), s.clone()))
            .collect();
        spans.sort();
        (rich.text, spans, rich.dropped.into_iter().collect())
    }

    #[test]
    fn characters_and_escapes() {
        let (text, _, _) = render(r"{\rtf1\ansi caf\'e9 \'93q\'94 \{x\} a\\b\tab c\line d\~e\u8364?f\uc0\u55357\u56832 }");
        assert_eq!(text, "café “q” {x} a\\b\tc\nd\u{a0}e€f😀");
    }

    #[test]
    fn styles_and_plain() {
        let (text, spans, _) = render(r"{\rtf1 \b bold \i both\plain  none {\ul u}\ul0  x}");
        assert_eq!(text, "bold both none u x");
        assert_eq!(
            spans,
            vec![
                ("bold both".into(), FormatStyle::Bold),
                ("both".into(), FormatStyle::Italic),
                ("u".into(), FormatStyle::Underline),
            ]
        );
    }

    #[test]
    fn unsafe_links_keep_their_text() {
        let (text, spans, dropped) = render(r#"{\rtf1 {\field{\*\fldinst{HYPERLINK "file:///etc"}}{\fldrslt here}}}"#);
        assert_eq!(text, "here");
        assert!(spans.is_empty());
        assert_eq!(dropped, vec![Dropped::Links]);
    }

    #[test]
    fn rejects_what_is_not_rtf() {
        assert!(read("hello").is_err());
        assert!(read(r"{\rtf1 {\b x}").is_err());
        assert!(read(r"{\rtf1 x}}").is_ok());
    }
}
