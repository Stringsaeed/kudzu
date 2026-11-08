//! The template editor's model: the template source exactly as stored, its formatting spans,
//! the selection and an undo history, with CodeMirror's editing commands over them.
//!
//! The source *is* the document, as it was in CodeMirror: tags show as chips and image spans
//! as images, but both are only decorations over source ranges, so what is saved is always
//! byte-for-byte what the person typed. Chips and images are atomic: the caret never stops
//! inside one, a selection that reaches into one covers it, and Backspace removes it whole.
//!
//! Offsets are UTF-16 code units, as the web editor, `lib/template.ts` and the stored
//! formatting spans count them.

use kudzu_core::client::editor_formatting::{self as ef, Change, ChangeDesc, TextEdit};
use kudzu_core::client::js;
use kudzu_core::client::template::{tokenize, TagToken, Token};
use kudzu_core::formatting::{FormatSpan, FormatStyle};

/// CodeMirror's `newGroupDelay`: edits further apart than this start a new undo step.
const GROUP_DELAY: f64 = 0.5;
/// CodeMirror's `minDepth`.
const MAX_HISTORY: usize = 100;
/// CodeMirror's default `indentUnit`.
pub const INDENT: &str = "  ";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sel {
    pub anchor: usize,
    pub head: usize,
}

impl Sel {
    pub fn cursor(at: usize) -> Sel {
        Sel { anchor: at, head: at }
    }
    pub fn new(anchor: usize, head: usize) -> Sel {
        Sel { anchor, head }
    }
    pub fn from(&self) -> usize {
        self.anchor.min(self.head)
    }
    pub fn to(&self) -> usize {
        self.anchor.max(self.head)
    }
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// What made a change, as CodeMirror's user events tell its history what joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// `input.type`: typing.
    Type,
    /// `delete.*`.
    Delete,
    /// `input.paste` / `input.drop`.
    Paste,
    /// `input`: inserts from the menus and chip edits.
    Input,
    /// An isolated step (`isolateHistory: "full"`): formatting, images, lists.
    Isolated,
    /// Anything else that makes its own step.
    Other,
}

#[derive(Clone)]
struct Entry {
    text: String,
    spans: Option<Vec<FormatSpan>>,
    sel: Sel,
}

/// The last change, for joining the next into its undo step.
#[derive(Clone, Copy)]
struct Last {
    event: Event,
    time: f64,
    /// The changed range in the document after it.
    from: usize,
    to: usize,
}

#[derive(Clone, Default)]
pub struct Doc {
    text: String,
    units: Vec<u16>,
    spans: Option<Vec<FormatSpan>>,
    pub sel: Sel,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    last: Option<Last>,
    /// Bumped on every text or formatting change (not selection).
    pub revision: u64,
}

/// Whether `c` attaches to the character before it (a combining mark, variation selector,
/// zero-width joiner, emoji modifier or tag): the grapheme rules a caret needs.
fn extends(c: u32) -> bool {
    matches!(c,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670
        | 0x06D6..=0x06DC | 0x06DF..=0x06E4 | 0x0900..=0x0903 | 0x093A..=0x094F | 0x0E31 | 0x0E34..=0x0E3A
        | 0x0E47..=0x0E4E | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x200C | 0x200D | 0x20D0..=0x20FF
        | 0x302A..=0x302F | 0x3099 | 0x309A | 0xFE00..=0xFE0F | 0xFE20..=0xFE2F | 0x1F3FB..=0x1F3FF
        | 0xE0020..=0xE007F | 0xE0100..=0xE01EF)
}

fn is_regional(c: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&c)
}

/// Unicode scalars of `units` with their UTF-16 starts.
fn scalars(units: &[u16]) -> Vec<(usize, u32)> {
    let mut out = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let u = units[i] as u32;
        if (0xD800..0xDC00).contains(&u) && i + 1 < units.len() && (0xDC00..0xE000).contains(&(units[i + 1] as u32)) {
            out.push((i, 0x10000 + ((u - 0xD800) << 10) + (units[i + 1] as u32 - 0xDC00)));
            i += 2;
        } else {
            out.push((i, u));
            i += 1;
        }
    }
    out
}

/// Grapheme cluster starts in `units` (plus its length): combining sequences, ZWJ emoji,
/// flag pairs and CRLF stay together.
pub fn grapheme_stops(units: &[u16]) -> Vec<usize> {
    let sc = scalars(units);
    let mut stops = Vec::with_capacity(sc.len() + 1);
    let mut regional_run = 0;
    for (k, &(at, c)) in sc.iter().enumerate() {
        let joined = k > 0 && {
            let prev = sc[k - 1].1;
            extends(c) || prev == 0x200D || (prev == 0x0D && c == 0x0A) || (is_regional(c) && is_regional(prev) && regional_run % 2 == 1)
        };
        regional_run = if is_regional(c) { regional_run + 1 } else { 0 };
        if !joined {
            stops.push(at);
        }
    }
    stops.push(units.len());
    stops
}

/// CodeMirror's character categories for moving by group.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cat {
    Word,
    Space,
    Other,
}

fn category(c: u32) -> Cat {
    match char::from_u32(c) {
        Some(ch) if ch.is_whitespace() => Cat::Space,
        Some(ch) if ch.is_alphanumeric() || ch == '_' => Cat::Word,
        _ => Cat::Other,
    }
}

impl Doc {
    pub fn new(text: &str, spans: Option<Vec<FormatSpan>>) -> Doc {
        let mut d = Doc { text: text.to_string(), units: js::utf16(text), ..Default::default() };
        d.spans = spans.map(|s| ef::update_formatting(Some(&s), None, &[Some(s.clone())], text).unwrap_or_default());
        d
    }

    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn units(&self) -> &[u16] {
        &self.units
    }
    pub fn len(&self) -> usize {
        self.units.len()
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    /// The formatting spans, or `None` for a plain-text snippet.
    pub fn spans(&self) -> Option<&[FormatSpan]> {
        self.spans.as_deref()
    }
    pub fn formatted(&self) -> bool {
        self.spans.is_some()
    }
    pub fn slice(&self, from: usize, to: usize) -> String {
        js::from16(&self.units[from.min(self.len())..to.min(self.len())])
    }

    /// The tags (chips), in order.
    pub fn tags(&self) -> Vec<TagToken> {
        tokenize(&self.text).into_iter().filter_map(|t| if let Token::Tag(t) = t { Some(t) } else { None }).collect()
    }

    /// The tag starting at `from`.
    pub fn tag_at(&self, from: usize) -> Option<TagToken> {
        self.tags().into_iter().find(|t| t.from == from)
    }

    /// The atomic ranges: chips and images, sorted by start.
    pub fn atoms(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = self.tags().iter().map(|t| (t.from, t.to)).collect();
        for s in self.spans.iter().flatten() {
            if matches!(s.style, FormatStyle::Image { .. }) {
                out.push((s.start, s.end));
            }
        }
        out.sort_unstable();
        out
    }

    /// The atom strictly containing `pos` (`from < pos < to`).
    fn atom_around(&self, pos: usize) -> Option<(usize, usize)> {
        self.atoms().into_iter().find(|&(a, b)| a < pos && pos < b)
    }

    /// `pos` moved out of any atom: to its start when `dir < 0`, its end when `dir > 0`, the
    /// nearer edge when `dir == 0`.
    pub fn skip_atomic(&self, pos: usize, dir: i32) -> usize {
        let pos = pos.min(self.len());
        match self.atom_around(pos) {
            Some((a, b)) => match dir.signum() {
                -1 => a,
                1 => b,
                _ => if pos - a <= b - pos { a } else { b },
            },
            None => pos,
        }
    }

    /// Sets the selection, clamped and pushed out of atoms (the head away from the anchor,
    /// so a selection reaching into a chip covers it).
    pub fn select(&mut self, anchor: usize, head: usize) {
        let (anchor, head) = (anchor.min(self.len()), head.min(self.len()));
        let dir = (head as i64 - anchor as i64).signum() as i32;
        let anchor = self.skip_atomic(anchor, -dir);
        let head = self.skip_atomic(head, if dir == 0 { 0 } else { dir });
        self.sel = Sel { anchor, head };
    }

    /// The selection widened over every chip and image it touches (`selectedRange`).
    pub fn selected_range(&self) -> (usize, usize) {
        ef::selected_range(&self.text, self.sel.from(), self.sel.to(), self.spans())
    }

    // MARK: Positions

    fn stops(&self) -> Vec<usize> {
        grapheme_stops(&self.units)
    }

    /// The next caret stop after `pos` (one grapheme, or past a whole atom).
    pub fn next_stop(&self, pos: usize) -> usize {
        if pos >= self.len() {
            return self.len();
        }
        if let Some((_, b)) = self.atoms().into_iter().find(|&(a, b)| a <= pos && pos < b) {
            return b;
        }
        let stops = self.stops();
        let next = stops.into_iter().find(|&s| s > pos).unwrap_or(self.len());
        self.skip_atomic(next, 1)
    }

    /// The caret stop before `pos`.
    pub fn prev_stop(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        if let Some((a, _)) = self.atoms().into_iter().find(|&(a, b)| a < pos && pos <= b) {
            return a;
        }
        let stops = self.stops();
        let prev = stops.into_iter().rev().find(|&s| s < pos).unwrap_or(0);
        self.skip_atomic(prev, -1)
    }

    pub fn line_start(&self, pos: usize) -> usize {
        self.units[..pos.min(self.len())].iter().rposition(|&u| u == b'\n' as u16).map_or(0, |i| i + 1)
    }

    pub fn line_end(&self, pos: usize) -> usize {
        let pos = pos.min(self.len());
        self.units[pos..].iter().position(|&u| u == b'\n' as u16).map_or(self.len(), |i| pos + i)
    }

    /// CodeMirror's `moveByGroup`: across spaces, then one run of word characters or of other
    /// characters (a chip counts as one), stopping at the line's edge.
    pub fn group_stop(&self, pos: usize, forward: bool) -> usize {
        let sc = |p: usize| -> Option<u32> {
            let i = if forward { p } else { p.checked_sub(1)? };
            let u = *self.units.get(i)? as u32;
            if forward && (0xD800..0xDC00).contains(&u) {
                let lo = *self.units.get(i + 1)? as u32;
                return Some(0x10000 + ((u - 0xD800) << 10) + (lo.wrapping_sub(0xDC00)));
            }
            if !forward && (0xDC00..0xE000).contains(&u) && i > 0 {
                let hi = self.units[i - 1] as u32;
                return Some(0x10000 + ((hi.wrapping_sub(0xD800)) << 10) + (u - 0xDC00));
            }
            Some(u)
        };
        let step = |p: usize| if forward { self.next_stop(p) } else { self.prev_stop(p) };
        let edge = if forward { self.line_end(pos) } else { self.line_start(pos) };
        if pos == edge {
            return step(pos);
        }
        let mut p = pos;
        let mut cat: Option<Cat> = None;
        while p != edge {
            let next = step(p);
            // A chip or image is a group of its own.
            let atom = if forward { self.atoms().into_iter().any(|(a, _)| a == p) } else { self.atoms().into_iter().any(|(_, b)| b == p) };
            if atom {
                if cat.is_none_or(|c| c == Cat::Space) {
                    return next;
                }
                break;
            }
            let Some(c) = sc(p) else { break };
            let c = category(c);
            match cat {
                None => cat = Some(c),
                Some(Cat::Space) => cat = Some(c),
                Some(k) if k != c => break,
                _ => {}
            }
            p = next;
        }
        p
    }

    // MARK: Changes

    fn snapshot(&self) -> Entry {
        Entry { text: self.text.clone(), spans: self.spans.clone(), sel: self.sel }
    }

    fn record(&mut self, event: Event, now: f64, from: usize, to_before: usize) {
        let join = match (self.last, event) {
            (Some(l), Event::Type | Event::Delete | Event::Input) => {
                l.event == event && now - l.time < GROUP_DELAY && from <= l.to && to_before >= l.from
            }
            _ => false,
        };
        if !join {
            self.undo.push(self.snapshot());
            if self.undo.len() > MAX_HISTORY {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// Applies non-overlapping `edits` (sorted), mapping formatting through them, and sets the
    /// selection to `sel` (or maps it). Returns the range the last edit inserted.
    pub fn apply(&mut self, edits: &[TextEdit], sel: Option<Sel>, event: Event, now: f64) -> (usize, usize) {
        let Some(desc) = ef::edits_desc(&self.text, edits) else { return (self.sel.head, self.sel.head) };
        if desc.is_empty() && sel.is_none() {
            return (self.sel.head, self.sel.head);
        }
        let from = edits.first().map_or(0, |e| e.from);
        let to_before = edits.last().map_or(0, |e| e.to);
        self.record(event, now, from, to_before);
        let next = ef::apply_edits(&self.text, edits);
        let sel = sel.unwrap_or_else(|| Sel { anchor: desc.map_pos(self.sel.anchor, 1), head: desc.map_pos(self.sel.head, 1) });
        self.spans = ef::update_formatting(self.spans.as_deref(), Some(&desc), &[], &next);
        self.text = next;
        self.units = js::utf16(&self.text);
        self.revision += 1;
        let inserted = edits.last().map_or((0, 0), |e| {
            let at = desc.map_pos(e.from, -1);
            (at, at + js::len16(&e.insert))
        });
        let end = edits.last().map_or(0, |e| desc.map_pos(e.from, -1) + js::len16(&e.insert));
        self.last = Some(Last { event, time: now, from: desc.map_pos(from, -1), to: end });
        self.select(sel.anchor, sel.head);
        inserted
    }

    /// Replaces `from..to` with `insert`, the caret after it (or `sel`).
    pub fn replace(&mut self, from: usize, to: usize, insert: &str, sel: Option<Sel>, event: Event, now: f64) -> (usize, usize) {
        let (from, to) = (from.min(self.len()), to.min(self.len()));
        let end = from + js::len16(insert);
        self.apply(&[TextEdit { from, to, insert: insert.to_string() }], Some(sel.unwrap_or(Sel::cursor(end))), event, now)
    }

    /// Replaces the formatting (`setFormatting`), as its own undo step.
    pub fn set_spans(&mut self, spans: Option<Vec<FormatSpan>>, now: f64) {
        self.record(Event::Isolated, now, 0, 0);
        self.spans = ef::update_formatting(self.spans.as_deref(), None, &[spans], &self.text);
        self.revision += 1;
        self.last = Some(Last { event: Event::Isolated, time: now, from: 0, to: 0 });
    }

    /// A text and formatting change in one isolated step (inserting an image).
    pub fn replace_with_spans(&mut self, from: usize, to: usize, insert: &str, spans: Vec<FormatSpan>, now: f64) {
        self.record(Event::Isolated, now, from, to);
        let edit = [TextEdit { from, to, insert: insert.to_string() }];
        let next = ef::apply_edits(&self.text, &edit);
        self.spans = ef::update_formatting(None, None, &[Some(spans)], &next);
        self.text = next;
        self.units = js::utf16(&self.text);
        self.revision += 1;
        self.last = Some(Last { event: Event::Isolated, time: now, from, to: from });
        let end = from + js::len16(insert);
        self.select(end, end);
    }

    /// A value from outside (another window, an import): replaces everything without an undo
    /// step, keeping the selection where it can.
    pub fn set_external(&mut self, text: &str, spans: Option<Vec<FormatSpan>>) {
        if self.text != text {
            let changes = [Change { from: 0, to: self.len(), insert_len: js::len16(text) }];
            let desc = ChangeDesc::of(&changes, self.len());
            self.text = text.to_string();
            self.units = js::utf16(text);
            let sel = desc.map(|d| Sel::new(d.map_pos(self.sel.anchor, 1), d.map_pos(self.sel.head, 1))).unwrap_or_default();
            self.sel = sel;
        }
        self.spans = spans.map(|s| ef::update_formatting(None, None, &[Some(s)], text).unwrap_or_default());
        self.revision += 1;
        self.last = None;
        let s = self.sel;
        self.select(s.anchor, s.head);
    }

    pub fn undo(&mut self) -> bool {
        let Some(e) = self.undo.pop() else { return false };
        let cur = self.snapshot();
        self.redo.push(cur);
        self.restore(e);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(e) = self.redo.pop() else { return false };
        let cur = self.snapshot();
        self.undo.push(cur);
        self.restore(e);
        true
    }

    fn restore(&mut self, e: Entry) {
        self.text = e.text;
        self.units = js::utf16(&self.text);
        self.spans = e.spans;
        self.revision += 1;
        self.last = None;
        self.select(e.sel.anchor, e.sel.head);
    }

    /// Ends the current undo step, so the next edit starts a new one (a selection change).
    pub fn break_group(&mut self) {
        self.last = None;
    }

    // MARK: Commands (CodeMirror's defaultKeymap)

    /// Typing: replaces the selection with `s`.
    pub fn type_text(&mut self, s: &str, now: f64) -> (usize, usize) {
        let (from, to) = (self.sel.from(), self.sel.to());
        let event = if s.chars().count() > 1 && s.contains('\n') { Event::Paste } else { Event::Type };
        self.replace(from, to, s, None, event, now)
    }

    /// Pasting plain text (line breaks normalised to `\n`, as CodeMirror's input does).
    pub fn paste(&mut self, s: &str, now: f64) {
        let s = s.replace("\r\n", "\n").replace('\r', "\n");
        let (from, to) = (self.sel.from(), self.sel.to());
        self.replace(from, to, &s, None, Event::Paste, now);
    }

    /// Deletes the selection, or from the caret to `target` (atoms widened whole).
    fn delete_to(&mut self, target: impl Fn(&Doc, usize) -> usize, now: f64) -> bool {
        let (from, to) = if !self.sel.is_empty() {
            (self.sel.from(), self.sel.to())
        } else {
            let head = self.sel.head;
            let t = target(self, head);
            let (a, b) = if t < head { (t, head) } else { (head, t) };
            // Widen over any atom the range cuts into.
            let mut a = a;
            let mut b = b;
            for (x, y) in self.atoms() {
                if x < b && y > a {
                    a = a.min(x);
                    b = b.max(y);
                }
            }
            (a, b)
        };
        if from == to {
            return false;
        }
        self.replace(from, to, "", Some(Sel::cursor(from)), Event::Delete, now);
        true
    }

    /// `deleteCharBackward`: one grapheme (a whole chip or image), or back to the previous
    /// indent column in leading whitespace.
    pub fn backspace(&mut self, now: f64) -> bool {
        self.delete_to(
            |d, head| {
                let start = d.line_start(head);
                let before = &d.units[start..head];
                if !before.is_empty() && before.iter().all(|&u| u == b' ' as u16) {
                    let col = before.len();
                    let unit = INDENT.len();
                    let target = if col % unit == 0 { col - unit } else { col - col % unit };
                    return start + target;
                }
                d.prev_stop(head)
            },
            now,
        )
    }

    /// `deleteCharForward`.
    pub fn delete_forward(&mut self, now: f64) -> bool {
        self.delete_to(|d, head| d.next_stop(head), now)
    }

    /// `deleteGroupBackward` / `deleteGroupForward`.
    pub fn delete_group(&mut self, forward: bool, now: f64) -> bool {
        self.delete_to(move |d, head| d.group_stop(head, forward), now)
    }

    /// Deletes from the caret to `target` (a line boundary the caller found).
    pub fn delete_to_pos(&mut self, target: usize, now: f64) -> bool {
        self.delete_to(move |_, _| target, now)
    }

    /// `insertNewlineAndIndent` with no language: a line break carrying the line's indentation,
    /// trailing spaces before the caret removed.
    pub fn newline_and_indent(&mut self, now: f64) {
        let (from, to) = (self.sel.from(), self.sel.to());
        let start = self.line_start(from);
        let indent: String = self.units[start..from].iter().take_while(|&&u| u == b' ' as u16 || u == b'\t' as u16).map(|&u| u as u8 as char).collect();
        let mut cut = from;
        while cut > start + indent.len() && self.units[cut - 1] == b' ' as u16 {
            cut -= 1;
        }
        let insert = format!("\n{indent}");
        let end = cut + js::len16(&insert);
        self.replace(cut, to, &insert, Some(Sel::cursor(end)), Event::Type, now);
    }

    /// `insertBlankLine` (⌘Return): a new line after the current one.
    pub fn insert_blank_line(&mut self, now: f64) {
        let end = self.line_end(self.sel.head);
        self.replace(end, end, "\n", Some(Sel::cursor(end + 1)), Event::Other, now);
    }

    /// The lines the selection touches, as (start, end) pairs.
    fn selected_lines(&self) -> Vec<(usize, usize)> {
        let (from, to) = (self.sel.from(), self.sel.to());
        let last = if to > from && self.line_start(to) == to { to - 1 } else { to };
        let mut lines = Vec::new();
        let mut at = self.line_start(from);
        loop {
            let end = self.line_end(at);
            lines.push((at, end));
            if end >= last || end >= self.len() {
                break;
            }
            at = end + 1;
        }
        lines
    }

    /// `indentMore` (Tab, ⌘]): a unit of indentation on every selected line.
    pub fn indent_more(&mut self, now: f64) {
        let edits: Vec<TextEdit> = self.selected_lines().into_iter().map(|(s, _)| TextEdit { from: s, to: s, insert: INDENT.into() }).collect();
        let n = js::len16(INDENT);
        let lines = self.selected_lines();
        let map = |p: usize| p + n * lines.iter().filter(|(s, _)| *s <= p).count();
        let sel = Sel::new(map(self.sel.anchor), map(self.sel.head));
        self.apply(&edits, Some(sel), Event::Other, now);
    }

    /// `indentLess` (⇧Tab, ⌘[).
    pub fn indent_less(&mut self, now: f64) {
        let edits: Vec<TextEdit> = self
            .selected_lines()
            .into_iter()
            .filter_map(|(s, e)| {
                let spaces = self.units[s..e].iter().take_while(|&&u| u == b' ' as u16).count();
                let tab = self.units.get(s) == Some(&(b'\t' as u16));
                let n = if tab { 1 } else { spaces.min(INDENT.len()) };
                (n > 0).then(|| TextEdit { from: s, to: s + n, insert: String::new() })
            })
            .collect();
        if !edits.is_empty() {
            self.apply(&edits, None, Event::Other, now);
        }
    }

    /// `moveLineUp` / `moveLineDown` (⌥↑ / ⌥↓).
    pub fn move_lines(&mut self, down: bool, now: f64) -> bool {
        let lines = self.selected_lines();
        let (first, last) = (lines[0].0, lines[lines.len() - 1].1);
        let block = self.slice(first, last);
        if down {
            if last >= self.len() {
                return false;
            }
            let next_end = self.line_end(last + 1);
            let next = self.slice(last + 1, next_end);
            let text = format!("{next}\n{block}");
            let shift = js::len16(&next) + 1;
            let sel = Sel::new(self.sel.anchor + shift, self.sel.head + shift);
            self.apply(&[TextEdit { from: first, to: next_end, insert: text }], Some(sel), Event::Other, now);
        } else {
            if first == 0 {
                return false;
            }
            let prev_start = self.line_start(first - 1);
            let prev = self.slice(prev_start, first - 1);
            let text = format!("{block}\n{prev}");
            let shift = js::len16(&prev) + 1;
            let sel = Sel::new(self.sel.anchor - shift, self.sel.head - shift);
            self.apply(&[TextEdit { from: prev_start, to: last, insert: text }], Some(sel), Event::Other, now);
        }
        true
    }

    /// `copyLineUp` / `copyLineDown` (⇧⌥↑ / ⇧⌥↓).
    pub fn copy_lines(&mut self, down: bool, now: f64) {
        let lines = self.selected_lines();
        let (first, last) = (lines[0].0, lines[lines.len() - 1].1);
        let block = self.slice(first, last);
        let n = js::len16(&block) + 1;
        if down {
            let sel = Sel::new(self.sel.anchor + n, self.sel.head + n);
            self.apply(&[TextEdit { from: last, to: last, insert: format!("\n{block}") }], Some(sel), Event::Other, now);
        } else {
            let sel = self.sel;
            self.apply(&[TextEdit { from: first, to: first, insert: format!("{block}\n") }], Some(sel), Event::Other, now);
        }
    }

    /// `deleteLine` (⇧⌘K).
    pub fn delete_lines(&mut self, now: f64) {
        let lines = self.selected_lines();
        let (mut from, mut to) = (lines[0].0, lines[lines.len() - 1].1);
        if to < self.len() {
            to += 1;
        } else {
            from = from.saturating_sub(1);
        }
        let at = from.min(self.len());
        self.replace(from, to, "", Some(Sel::cursor(at)), Event::Delete, now);
    }

    /// `splitLine` (⌃O): a line break after the caret.
    pub fn split_line(&mut self, now: f64) {
        let (from, to) = (self.sel.from(), self.sel.to());
        self.replace(from, to, "\n", Some(Sel::cursor(from)), Event::Type, now);
    }

    /// `transposeChars` (⌃T).
    pub fn transpose(&mut self, now: f64) {
        if !self.sel.is_empty() {
            return;
        }
        let head = self.sel.head;
        let (start, end) = (self.line_start(head), self.line_end(head));
        if end - start < 2 || head == start {
            return;
        }
        let at = if head == end { self.prev_stop(head) } else { head };
        let before = self.prev_stop(at);
        let after = self.next_stop(at);
        if before < start || after > end {
            return;
        }
        let swapped = format!("{}{}", self.slice(at, after), self.slice(before, at));
        self.replace(before, after, &swapped, Some(Sel::cursor(after)), Event::Other, now);
    }

    /// `toggleList`: bullet or number prefixes on the selected lines.
    pub fn toggle_list(&mut self, ordered: bool, now: f64) {
        let edits = ef::toggle_list(&self.text, self.sel.from(), self.sel.to(), ordered);
        self.apply(&edits, None, Event::Isolated, now);
    }

    /// `changeFormat`: toggles a style on the selection (widened over chips and images).
    pub fn change_format(&mut self, change: &ef::FormatChange, now: f64) -> bool {
        match ef::change_format(&self.text, (self.sel.from(), self.sel.to()), self.spans(), change) {
            Some(next) => {
                self.set_spans(Some(next), now);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::client::editor_formatting::FormatChange;
    use kudzu_core::client::template::tokenize;

    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg==";

    fn image(start: usize, end: usize) -> FormatSpan {
        FormatSpan { start, end, style: FormatStyle::Image { data_url: PNG.into(), width: 24 } }
    }

    #[test]
    fn the_document_is_the_stored_source() {
        for src in ["", "Best regards,\nSaeed\n{{date format=\"MMMM d, yyyy\"}}", r"a \{{x}} \\ {{b", "😀 {{cursor}}{{wat}} é", "{{choice name=x options=\"a}}|b\"}} z"] {
            let d = Doc::new(src, None);
            assert_eq!(d.text(), src);
            // Tokens cover the source exactly, so chips and text reassemble it.
            let units = js::utf16(src);
            let mut at = 0;
            for t in tokenize(src) {
                assert_eq!(t.from(), at);
                at = t.to();
            }
            assert_eq!(at, units.len());
        }
    }

    #[test]
    fn chips_are_atomic_for_the_caret_and_backspace() {
        let mut d = Doc::new("Hi {{field name=\"Name\"}}!", None);
        let tag = d.tags()[0].clone();
        assert_eq!((tag.from, tag.to), (3, 24));
        assert_eq!(d.next_stop(3), 24);
        assert_eq!(d.prev_stop(24), 3);
        d.select(10, 10);
        assert_eq!(d.sel.head, 3, "a caret inside snaps to the nearer edge");
        d.select(1, 10);
        assert_eq!((d.sel.anchor, d.sel.head), (1, 24), "a selection reaching in covers the chip");
        d.select(24, 24);
        d.backspace(0.0);
        assert_eq!(d.text(), "Hi !");
        d.undo();
        assert_eq!(d.text(), "Hi {{field name=\"Name\"}}!");
        assert_eq!(d.sel, Sel::cursor(24));
    }

    #[test]
    fn typing_groups_into_one_undo_step_and_round_trips() {
        let src = "Dear {{field name=\"Name\"}},";
        let mut d = Doc::new(src, None);
        d.select(d.len(), d.len());
        for (i, c) in " thanks".chars().enumerate() {
            d.type_text(&c.to_string(), i as f64 * 0.1);
        }
        assert_eq!(d.text(), "Dear {{field name=\"Name\"}}, thanks");
        d.type_text("!", 5.0);
        d.undo();
        assert_eq!(d.text(), "Dear {{field name=\"Name\"}}, thanks");
        d.undo();
        assert_eq!(d.text(), src);
        d.redo();
        d.redo();
        assert_eq!(d.text(), "Dear {{field name=\"Name\"}}, thanks!");
    }

    #[test]
    fn images_are_atomic_and_formatting_follows_edits() {
        let mut d = Doc::new("A [Logo] B", Some(vec![image(2, 8), FormatSpan { start: 0, end: 1, style: FormatStyle::Bold }]));
        assert_eq!(d.atoms(), vec![(2, 8)]);
        d.select(9, 9);
        d.backspace(0.0);
        assert_eq!(d.text(), "A [Logo]B");
        d.backspace(1.0);
        assert_eq!(d.text(), "A B", "backspace into an image removes it whole");
        assert!(d.spans().unwrap().iter().all(|s| !matches!(s.style, FormatStyle::Image { .. })));
        d.undo();
        assert_eq!(d.text(), "A [Logo]B");
        assert_eq!(d.spans().unwrap().iter().filter(|s| matches!(s.style, FormatStyle::Image { .. })).count(), 1);
        // Typing beside an image stays outside it; typing after bold text extends it.
        d.select(1, 1);
        d.type_text("x", 10.0);
        assert_eq!(d.text(), "Ax [Logo]B");
        let bold = d.spans().unwrap().iter().find(|s| s.style == FormatStyle::Bold).unwrap().clone();
        assert_eq!((bold.start, bold.end), (0, 2));
        let img = d.spans().unwrap().iter().find(|s| matches!(s.style, FormatStyle::Image { .. })).unwrap().clone();
        assert_eq!((img.start, img.end), (3, 9));
    }

    #[test]
    fn format_toggle_is_its_own_step() {
        let mut d = Doc::new("Hello world", Some(vec![]));
        d.select(0, 5);
        assert!(d.change_format(&FormatChange::Style(FormatStyle::Bold), 0.0));
        assert_eq!(d.spans().unwrap().len(), 1);
        assert!(d.change_format(&FormatChange::Style(FormatStyle::Bold), 0.1));
        assert!(d.spans().unwrap().is_empty());
        d.undo();
        assert_eq!(d.spans().unwrap().len(), 1);
        d.undo();
        assert!(d.spans().unwrap().is_empty());
        // Plain text can't take formatting.
        let mut p = Doc::new("x", None);
        p.select(0, 1);
        assert!(!p.change_format(&FormatChange::Style(FormatStyle::Bold), 0.0));
    }

    #[test]
    fn switching_modes_undoes() {
        let mut d = Doc::new("Hi", None);
        d.set_spans(Some(vec![]), 0.0);
        assert!(d.formatted());
        d.undo();
        assert!(!d.formatted());
    }

    #[test]
    fn group_motion_and_graphemes() {
        let d = Doc::new("foo  bar.baz {{cursor}}x", None);
        assert_eq!(d.group_stop(0, true), 3);
        assert_eq!(d.group_stop(3, true), 8);
        assert_eq!(d.group_stop(8, true), 9);
        assert_eq!(d.group_stop(12, true), 23, "a chip is its own group after spaces");
        assert_eq!(d.group_stop(23, false), 13);
        let e = Doc::new("e\u{301}👍🏽🇬🇧x", None);
        assert_eq!(grapheme_stops(e.units()), vec![0, 2, 6, 10, 11]);
        assert_eq!(e.next_stop(0), 2);
        assert_eq!(e.prev_stop(10), 6);
    }

    #[test]
    fn lines_indent_move_and_lists() {
        let mut d = Doc::new("a\nb\nc", None);
        d.select(2, 2);
        d.move_lines(false, 0.0);
        assert_eq!(d.text(), "b\na\nc");
        assert_eq!(d.sel.head, 0);
        d.indent_more(1.0);
        assert_eq!(d.text(), "  b\na\nc");
        d.select(2, 2);
        d.backspace(2.0);
        assert_eq!(d.text(), "b\na\nc", "backspace in indentation removes a unit");
        d.select(0, 3);
        d.toggle_list(false, 3.0);
        assert_eq!(d.text(), "• b\n• a\nc");
        d.select(3, 3);
        d.newline_and_indent(4.0);
        assert_eq!(d.text(), "• b\n\n• a\nc");
        let mut i = Doc::new("  x", None);
        i.select(3, 3);
        i.newline_and_indent(0.0);
        assert_eq!(i.text(), "  x\n  ");
    }

    #[test]
    fn external_values_skip_history() {
        let mut d = Doc::new("one", None);
        d.select(3, 3);
        d.type_text("!", 0.0);
        d.set_external("two words", None);
        assert_eq!(d.text(), "two words");
        assert!(d.sel.head <= d.len());
        d.undo();
        assert_eq!(d.text(), "one");
    }
}
