//! The rich field: pills from an [`EditorDoc`], slash menu, and chip configuration
//! once a pill is already the whole selection.

use std::ops::Range;

use rune::edit::{Block, Mark};
use rune::*;
use rune_kit::{Kit, PopupItem, PopupList};

use crate::chips::{self, ChipGesture};
use crate::doc::{EditorDoc, EditorStyle};

/// Slash command id, menu label, chip label, and the tag the chip stands for.
const COMMANDS: &[(&str, &str, &str, &str)] = &[
    ("date", "Date", "Date \u{00b7} ISO date", "{{date}}"),
    (
        "time",
        "Time",
        "Time \u{00b7} 24-hour time",
        "{{date format=\"HH:mm\"}}",
    ),
    ("clipboard", "Clipboard", "Clipboard", "{{clipboard}}"),
    ("cursor", "Cursor", "Cursor", "{{cursor}}"),
];

/// How a chip's settings were asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenVia {
    DoubleClick,
    Return,
    Space,
}

pub struct SlashQuery {
    pub range: Range<usize>,
    pub query: String,
}

pub struct State {
    pub edit: TextEdit,
    pub formatted: bool,
    pub spellcheck: bool,
    pub chip_open: bool,
    pub opened_via: Option<OpenVia>,
    pub chip_text: String,
    pub gesture: ChipGesture,
    /// A fixed field width, so a chip can wrap across lines.
    pub field_width: Option<f32>,
    /// The highlighted slash row, moved by Up and Down.
    pub slash_active: usize,
    /// The `/` offset whose menu Escape closed, until that `/` goes.
    pub slash_dismissed: Option<usize>,
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
            opened_via: None,
            chip_text: String::new(),
            gesture: ChipGesture::default(),
            field_width: None,
            slash_active: 0,
            slash_dismissed: None,
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
            let id = chips::chip_id(pill.tone.name(), &pill.source);
            edit.replace_with_pill(pill.range.clone(), &id, &pill.label);
        }
        for mark in &doc.marks {
            edit.set_selection(mark.range.start, mark.range.end);
            edit.set_mark(rune_mark(&mark.style));
        }
        edit.set_caret(edit.text.len());
    });
    // Loading is not something the user did. `undo_group` leaves it as one undo step, which
    // would let the first Cmd+Z strip every chip and mark off an untouched snippet.
    edit.clear_history();
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
    let mut field = text_input(&s.edit, field_style(s.formatted))
        .key("field")
        .min_h(160.0)
        .padding(8.0)
        .spellcheck(s.spellcheck)
        .pill_style(pill_look)
        .on_edit(|s: &mut State| &mut s.edit)
        .on_input_key(field_key)
        .on_input_paste(|s: &mut State, content, _| chips::paste_own(&mut s.edit, content))
        // A press on a text field never reaches `on_press`. Rune does run this on each mouse-up,
        // after the press's selection step, which is the only pointer hook after a click.
        .on_pointer_move_local(|s: &mut State, ev: PointerEvent, tx: &mut Tx| {
            let at = (ev.window_x, ev.window_y);
            match s.gesture.pointer(at, &s.edit) {
                Some((range, id)) => open_chip(s, range, &id, OpenVia::DoubleClick),
                None => tx.live_only(),
            }
        })
        .on_submit(|s: &mut State, _| s.edit.insert("\n"));
    field = match s.field_width {
        Some(width) => field.w(width),
        None => field.w_full(),
    };
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
    if let Some(slash) = open_slash(s) {
        let ids = slash_ids(&slash.query);
        PopupList::new("slash", "caret-anchor")
            .anchored("caret-anchor", Side::Bottom, Align::Start, 4.0)
            .items(ids.iter().map(|id| slash_item(id)))
            .active(Some(s.slash_active.min(ids.len().saturating_sub(1))))
            .on_choose(|s: &mut State, id, _| choose_slash(s, id))
            .build(&kit, &mut layer);
    }
    rune_kit::root(base, layer)
}

/// The slash query whose menu is showing: Escape closes it until its `/` goes.
pub fn open_slash(s: &State) -> Option<SlashQuery> {
    slash_query(&s.edit).filter(|slash| s.slash_dismissed != Some(slash.range.start))
}

/// Command ids matching the query, in menu order.
pub fn slash_ids(query: &str) -> Vec<&'static str> {
    let query = query.to_ascii_lowercase();
    COMMANDS
        .iter()
        .filter(|(id, label, _, _)| {
            query.is_empty()
                || id.starts_with(&query)
                || label.to_ascii_lowercase().starts_with(&query)
        })
        .map(|command| command.0)
        .collect()
}

fn slash_item(id: &str) -> PopupItem {
    let label = COMMANDS
        .iter()
        .find(|command| command.0 == id)
        .map_or(id, |command| command.1);
    PopupItem::new(id, label)
}

fn choose_slash(s: &mut State, id: &str) {
    let Some(slash) = slash_query(&s.edit) else {
        return;
    };
    let Some((_, _, label, source)) = COMMANDS.iter().find(|command| command.0 == id) else {
        return;
    };
    s.edit
        .replace_with_pill(slash.range, chips::chip_id("time", source), label);
    s.chosen = Some(id.to_string());
    s.slash_active = 0;
    s.slash_dismissed = None;
}

fn open_chip(s: &mut State, range: Range<usize>, id: &str, via: OpenVia) {
    s.chip_text = format!("{id} {}", &s.edit.text[range]);
    s.chip_open = true;
    s.opened_via = Some(via);
}

/// Up, Down, Return, Tab and Escape on the slash menu. The popup takes no focus, so the field
/// hears them first, and a consumed Return does not insert a newline.
fn slash_key(s: &mut State, ev: &KeyEvent) -> bool {
    let plain = !ev.mods.cmd && !ev.mods.alt && !ev.mods.ctrl && !ev.mods.shift;
    let Some(slash) = open_slash(s) else {
        if matches!(ev.key, Key::Text(_) | Key::Backspace) && slash_query(&s.edit).is_none() {
            s.slash_dismissed = None;
        }
        return false;
    };
    let ids = slash_ids(&slash.query);
    match &ev.key {
        Key::Text(_) | Key::Backspace => s.slash_active = 0,
        _ if ids.is_empty() || !plain => {}
        // Consumed, or the field would move its caret and close the menu.
        Key::Down => {
            s.slash_active = (s.slash_active + 1) % ids.len();
            return true;
        }
        Key::Up => {
            s.slash_active = (s.slash_active + ids.len() - 1) % ids.len();
            return true;
        }
        Key::Enter | Key::Tab => {
            choose_slash(s, ids[s.slash_active.min(ids.len() - 1)]);
            return true;
        }
        Key::Escape => {
            s.slash_dismissed = Some(slash.range.start);
            return true;
        }
        _ => {}
    }
    false
}

fn field_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    s.gesture.key();
    if s.chip_open && matches!(ev.key, Key::Escape) {
        s.chip_open = false;
        return true;
    }
    if slash_key(s, ev) {
        return true;
    }
    // Copy and cut: the template source as plain text, Rune's rich copy beside it. The field's
    // own menu runs Cut and Copy without this handler (`field_menu_pick`).
    let plain = !ev.mods.shift && !ev.mods.alt;
    let copies = plain && ev.is_command('c');
    let cuts = plain && ev.is_command('x');
    if copies || cuts {
        let Some(content) = chips::copy_content(&s.edit) else {
            return false;
        };
        tx.copy_rich(content);
        if cuts {
            s.edit.insert("");
        }
        return true;
    }
    let via = match &ev.key {
        Key::Enter => OpenVia::Return,
        Key::Text(text) if text == " " => OpenVia::Space,
        _ => return false,
    };
    let Some((range, id)) = selected_pill(&s.edit) else {
        return false;
    };
    open_chip(s, range, &id, via);
    true
}

fn pill_look(id: &str) -> PillStyle {
    let tone = chips::chip_tone(id);
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
