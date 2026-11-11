//! The rich field: pills from an [`EditorDoc`], slash menu, and chip configuration
//! once a pill is already the whole selection.

use std::ops::Range;

use rune::edit::{Block, Mark};
use rune::*;
use rune_kit::{Kit, PopupItem, PopupList};

use crate::doc::{EditorDoc, EditorStyle};

const COMMANDS: &[(&str, &str, &str)] = &[
    ("date", "Date", "Date \u{00b7} ISO date"),
    ("time", "Time", "Time \u{00b7} 24-hour time"),
    ("clipboard", "Clipboard", "Clipboard"),
    ("cursor", "Cursor", "Cursor"),
];

pub struct SlashQuery {
    pub range: Range<usize>,
    pub query: String,
}

pub struct State {
    pub edit: TextEdit,
    pub formatted: bool,
    pub spellcheck: bool,
    pub chip_open: bool,
    pub chip_text: String,
    pub chosen: Option<String>,
    pub overlay: bool,
    pub overlay_at: [f32; 2],
    pub pixels: Pixels,
}

impl Default for State {
    fn default() -> Self {
        let doc = crate::doc::from_template("Hello {{date}} there", None);
        let pixels = Pixels::new();
        pixels.set(4, 4, [220u8, 40, 40, 255].repeat(16));
        Self {
            edit: load_edit(&doc),
            formatted: false,
            spellcheck: true,
            chip_open: false,
            chip_text: String::new(),
            chosen: None,
            overlay: false,
            overlay_at: [0.0, 0.0],
            pixels,
        }
    }
}

pub fn field_style(formatted: bool) -> TextStyle {
    let style = TextStyle::new(14.0, 400.0);
    if formatted {
        style.family(Family::Sans)
    } else {
        style.mono()
    }
}

pub fn load_edit(doc: &EditorDoc) -> TextEdit {
    let mut edit = TextEdit::new(&doc.text)
        .rich()
        .allow_marks([
            Mark::Bold,
            Mark::Italic,
            Mark::Underline,
            Mark::Link(String::new()),
        ])
        .allow_blocks(std::iter::empty::<Block>());
    edit.undo_group(|edit| {
        for pill in doc.pills.iter().rev() {
            edit.replace_with_pill(pill.range.clone(), &pill.id, &pill.label);
        }
        for mark in &doc.marks {
            edit.set_selection(mark.range.start, mark.range.end);
            edit.set_mark(rune_mark(&mark.style));
        }
        edit.set_caret(edit.text.len());
    });
    edit
}

fn rune_mark(style: &EditorStyle) -> Mark {
    match style {
        EditorStyle::Bold => Mark::Bold,
        EditorStyle::Italic => Mark::Italic,
        EditorStyle::Underline => Mark::Underline,
        EditorStyle::Link { url } => Mark::Link(url.clone()),
    }
}

/// The selection when it is exactly one pill and nothing else.
pub fn selected_pill(edit: &TextEdit) -> Option<(Range<usize>, String)> {
    let (start, end) = edit.range();
    if start == end {
        return None;
    }
    let mut hit = None;
    for (range, id) in edit.pills() {
        if range.start < end && range.end > start {
            if hit.is_some() {
                return None;
            }
            hit = Some((range, id.to_string()));
        }
    }
    hit.filter(|(range, _)| range.start == start && range.end == end)
}

/// `/query` when `/` begins a word: at the start, or after whitespace, `(`, `[`, or a quote.
pub fn slash_query(edit: &TextEdit) -> Option<SlashQuery> {
    if edit.anchor != edit.focus {
        return None;
    }
    let end = edit.focus.min(edit.text.len());
    if !edit.text.is_char_boundary(end) {
        return None;
    }
    let head = &edit.text[..end];
    let mut slash = None;
    for (index, ch) in head.char_indices().rev() {
        if ch == '/' {
            slash = Some(index);
            break;
        }
        if is_word_boundary(ch) {
            return None;
        }
    }
    let slash = slash?;
    if slash > 0 && !is_word_boundary(head[..slash].chars().next_back()?) {
        return None;
    }
    if edit
        .pills()
        .any(|(range, _)| range.contains(&slash) && slash != range.start)
    {
        return None;
    }
    Some(SlashQuery {
        range: slash..end,
        query: head[slash + '/'.len_utf8()..].to_string(),
    })
}

fn is_word_boundary(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '(' | '[' | '"' | '\'')
}

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let kit = Kit::neutral(true);
    let caret = cx.caret("field");
    let (ax, ay, ah) = caret
        .map(|c| (c[0], c[1], c[3].max(1.0)))
        .unwrap_or((8.0, 8.0, 1.0));
    let field = text_input(&s.edit, field_style(s.formatted))
        .key("field")
        .w_full()
        .min_h(160.0)
        .padding(8.0)
        .spellcheck(s.spellcheck)
        .pill_style(pill_look)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_input_key(field_key)
        .on_submit(|s: &mut State, _| s.edit.insert("\n"));
    let mut base = el()
        .key("editor-root")
        .w_full()
        .h_full()
        .child(field)
        .child(el().key("caret-anchor").absolute(ax, ay).w(1.0).h(ah));
    if s.chip_open {
        let (x, y) = caret
            .map(|c| (c[0], c[1] + c[3] + 4.0))
            .unwrap_or((8.0, 32.0));
        base = base.child(
            el().key("chip-config")
                .absolute(x, y)
                .padding(6.0)
                .child(text(s.chip_text.clone()).key("chip-config-label")),
        );
    }
    if s.overlay {
        base = base.child(
            el().key("bitmap")
                .absolute(s.overlay_at[0], s.overlay_at[1])
                .w(16.0)
                .h(16.0)
                .pixels(&s.pixels),
        );
    }
    let mut layer = Vec::new();
    if let Some(slash) = slash_query(&s.edit) {
        PopupList::new("slash", "caret-anchor")
            .anchored("caret-anchor", Side::Bottom, Align::Start, 4.0)
            .items(slash_items(&slash.query))
            .active(Some(0))
            .on_choose(|s: &mut State, id, _| choose_slash(s, id))
            .build(&kit, &mut layer);
    }
    rune_kit::root(base, layer)
}

fn slash_items(query: &str) -> Vec<PopupItem> {
    let query = query.to_ascii_lowercase();
    COMMANDS
        .iter()
        .filter(|(id, label, _)| {
            query.is_empty()
                || id.starts_with(&query)
                || label.to_ascii_lowercase().starts_with(&query)
        })
        .map(|(id, label, _)| PopupItem::new(*id, *label))
        .collect()
}

fn choose_slash(s: &mut State, id: &str) {
    let Some(slash) = slash_query(&s.edit) else {
        return;
    };
    let Some((_, _, label)) = COMMANDS.iter().find(|command| command.0 == id) else {
        return;
    };
    s.edit
        .replace_with_pill(slash.range, format!("time:{id}"), label);
    s.chosen = Some(id.to_string());
}

fn field_key(s: &mut State, ev: &KeyEvent, _: &mut Tx) -> bool {
    if s.chip_open && matches!(ev.key, Key::Escape) {
        s.chip_open = false;
        return true;
    }
    let opens = match &ev.key {
        Key::Enter => true,
        Key::Text(text) if text == " " => true,
        _ => false,
    };
    if !opens {
        return false;
    }
    let Some((range, id)) = selected_pill(&s.edit) else {
        return false;
    };
    s.chip_text = format!("{id} {}", &s.edit.text[range]);
    s.chip_open = true;
    true
}

fn pill_look(id: &str) -> PillStyle {
    let tone = id.split(':').next().unwrap_or("form");
    let bg = match tone {
        "time" => oklch(0.55, 0.12, 230.0),
        "clipboard" => oklch(0.55, 0.12, 150.0),
        "cursor" => oklch(0.55, 0.12, 90.0),
        "snippet" => oklch(0.55, 0.12, 300.0),
        "key" => oklch(0.55, 0.1, 40.0),
        "error" => oklch(0.55, 0.16, 25.0),
        _ => oklch(0.55, 0.04, 260.0),
    };
    PillStyle::new().bg(bg).fg(oklch(0.98, 0.0, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_group_reverts_a_pill_insert_and_a_mark_together() {
        let mut edit = TextEdit::new("Hello").rich().allow_marks([Mark::Bold]);
        edit.undo_group(|edit| {
            edit.set_caret(edit.text.len());
            edit.insert_pill("time:0", "Date \u{00b7} ISO date");
            edit.set_selection(0, edit.text.len());
            edit.set_mark(Mark::Bold);
        });
        assert!(edit.pills().any(|(_, id)| id == "time:0"));
        assert!(edit.marks().iter().any(|m| m.mark == Mark::Bold));
        edit.undo();
        assert_eq!(edit.text, "Hello");
        assert!(edit.pills().next().is_none());
        assert!(edit.marks().iter().all(|m| m.mark != Mark::Bold));
        edit.redo();
        assert!(edit.pills().any(|(_, id)| id == "time:0"));
        assert!(edit.marks().iter().any(|m| m.mark == Mark::Bold));
    }

    #[test]
    fn allow_blocks_keeps_lists_as_paragraphs_and_drops_strike() {
        let doc = crate::doc::from_template("one\ntwo", None);
        let mut edit = load_edit(&doc);
        assert!(edit.allows_mark(&Mark::Bold));
        assert!(edit.allows_mark(&Mark::Link(String::new())));
        assert!(!edit.allows_mark(&Mark::Strike));
        assert!(!edit.allows_block(Block::Bullet(0)));
        assert!(edit.allows_block(Block::Paragraph));
        edit.set_selection(0, edit.text.len());
        edit.set_block(Block::Bullet(0));
        edit.set_mark(Mark::Strike);
        assert!(edit.blocks().iter().all(|b| *b == Block::Paragraph));
        assert!(edit.marks().iter().all(|m| m.mark != Mark::Strike));
    }

    #[test]
    fn slash_opens_only_at_a_word_start() {
        let mut edit = TextEdit::new("hello /da");
        edit.set_caret(edit.text.len());
        let slash = slash_query(&edit).unwrap();
        assert_eq!(slash.query, "da");
        edit.set_caret("hello".len());
        assert!(slash_query(&edit).is_none());
        let mut mid = TextEdit::new("hello/da");
        mid.set_caret(mid.text.len());
        assert!(slash_query(&mid).is_none());
    }

    #[test]
    fn loaded_pills_match_the_doc_and_typing_does_not_extend_them() {
        let doc = crate::doc::from_template("Hello {{date}} there", None);
        let pill = doc.pills[0].range.clone();
        let mut edit = load_edit(&doc);
        let (range, _) = edit.pills().next().unwrap();
        assert_eq!(range, pill);
        assert_eq!(&edit.text[range.clone()], "Date \u{00b7} ISO date");
        edit.set_caret(range.start + 2);
        assert!(edit.focus == range.start || edit.focus == range.end);
        let before = range.end - range.start;
        edit.insert("x");
        let (after, _) = edit.pills().next().unwrap();
        assert_eq!(after.end - after.start, before);
        assert!(!edit.text[after.clone()].contains('x'));
    }

    #[test]
    fn image_pill_is_atomic_in_the_field() {
        let source = "Hello 😀 {{date}} world";
        let url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a6x8AAAAASUVORK5CYII=";
        let formatting = kudzu_core::formatting::Formatting {
            source: source.into(),
            spans: vec![kudzu_core::formatting::FormatSpan {
                start: 0,
                end: 5,
                style: kudzu_core::formatting::FormatStyle::Image {
                    data_url: url.into(),
                    width: 32,
                },
            }],
        };
        let doc = crate::doc::from_template(source, Some(&formatting));
        assert!(doc
            .pills
            .iter()
            .any(|p| p.kind == crate::doc::PillKind::Image));
        let edit = load_edit(&doc);
        assert_eq!(edit.pills().count(), 2);
    }
}
