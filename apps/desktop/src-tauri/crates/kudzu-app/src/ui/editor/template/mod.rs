//! `template-editor.tsx`: the snippet's content in an editor Kudzu draws itself. The template
//! source is the document (as CodeMirror's was); each tag shows as an atomic chip and each
//! embedded image at its display width, so what's saved round-trips byte for byte.
//!
//! Rune's `TextEdit` has pills but no inline images, so this editor lays out, paints and edits
//! the text itself ([`layout`], [`doc`]). The keyboard and the input method reach it through a
//! *proxy*: an invisible one-point `text_input` that sits at the caret and holds focus. Its key
//! hook runs CodeMirror's keymap on the document and takes every key it handles; an input
//! method composes in the proxy (its preedit is drawn here, inline at the caret) and commits
//! into the document; paste arrives through its paste hook. So focus, Tab order, ⌘E, IME
//! candidate placement and VoiceOver's text field all behave as for a native field, while the
//! drawing and editing stay CodeMirror's.

pub mod doc;
pub mod layout;

use std::cell::RefCell;
use std::rc::Rc;

use kudzu_core::client::editor_formatting::FormatChange;
use kudzu_core::client::template::ChipTone;
use kudzu_core::formatting::FormatStyle;
use rune::{el, rich, row, text, text_input, Color, Cursor, Drag, El, Key, KeyEvent, LoopSpec, Phase, Shadow, Span, TextEdit, Tx};

use self::doc::{Doc, Sel};
use self::layout::{Kind, Layout, CHIP_FONT, CHIP_FONT_MONO, CHIP_MARGIN, LINE_H, PAD_X, PAD_Y};
use crate::ui::theme::{radius, Theme};

/// What an edit asks of the snippet editor around it.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Effects {
    /// The text or formatting changed: save it.
    pub changed: bool,
    /// A `/` typed at a word's start, at this offset: open the insert menu there.
    pub slash: Option<usize>,
    /// A chip was clicked: configure the tag starting here.
    pub chip: Option<usize>,
    /// The key or press was taken.
    pub handled: bool,
    /// Escape with nothing to collapse: leave the editor for the list.
    pub escape: bool,
}

/// How a pointer drag extends the selection: by characters, words or lines.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum Unit {
    #[default]
    Char,
    Word,
    Line,
}

#[derive(Default)]
pub struct TemplateState {
    pub doc: Doc,
    /// The keyboard and input-method sink (see the module docs).
    pub proxy: TextEdit,
    /// Up and Down keep the caret's x across lines.
    goal_x: Option<f32>,
    /// At a soft wrap, the caret shows at the end of the earlier line.
    before: bool,
    drag_unit: Unit,
    /// The word or line a double or triple press started on.
    drag_origin: (usize, usize),
    /// The layout the view last drew, for hit testing and vertical motion.
    pub layout: RefCell<Rc<Layout>>,
    /// Bumped whenever the caret moves, restarting its blink.
    pub caret_gen: u64,
    /// The document revision the snippet editor last saved.
    pub saved_revision: u64,
}

impl Clone for TemplateState {
    fn clone(&self) -> Self {
        TemplateState {
            doc: self.doc.clone(),
            proxy: TextEdit::new(""),
            goal_x: None,
            before: false,
            drag_unit: Unit::Char,
            drag_origin: (0, 0),
            layout: RefCell::new(self.layout.borrow().clone()),
            caret_gen: self.caret_gen,
            saved_revision: self.saved_revision,
        }
    }
}

/// Seconds on a monotonic clock, for undo grouping.
pub fn now() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn is_word_start_before(units: &[u16], at: usize) -> bool {
    if at == 0 {
        return true;
    }
    let c = units[at - 1];
    char::from_u32(c as u32).is_some_and(|ch| ch.is_whitespace() || matches!(ch, '(' | '[' | '"' | '\''))
}

impl TemplateState {
    pub fn new(text: &str, spans: Option<Vec<kudzu_core::formatting::FormatSpan>>) -> TemplateState {
        let doc = Doc::new(text, spans);
        let saved_revision = doc.revision;
        TemplateState { doc, saved_revision, ..Default::default() }
    }

    pub fn focused(&self) -> bool {
        self.proxy.focused
    }

    pub fn focus(&mut self) {
        self.proxy.focused = true;
    }

    pub fn composing(&self) -> bool {
        self.proxy.preedit.as_ref().is_some_and(|p| !p.0.is_empty())
    }

    fn moved(&mut self) {
        self.caret_gen = self.caret_gen.wrapping_add(1);
        self.doc.break_group();
    }

    fn set_sel(&mut self, anchor: usize, head: usize, keep_goal: bool) {
        self.doc.select(anchor, head);
        if !keep_goal {
            self.goal_x = None;
            self.before = false;
        }
        self.moved();
    }

    fn layout(&self) -> Rc<Layout> {
        self.layout.borrow().clone()
    }

    /// Inserts `text` at the selection (replacing it), returning the inserted range.
    pub fn insert(&mut self, text: &str) -> (usize, usize) {
        let (from, to) = (self.doc.sel.from(), self.doc.sel.to());
        self.replace(from, to, text)
    }

    pub fn replace(&mut self, from: usize, to: usize, text: &str) -> (usize, usize) {
        let r = self.doc.replace(from, to, text, None, doc::Event::Input, now());
        self.goal_x = None;
        self.moved();
        r
    }

    // MARK: Keys (CodeMirror's defaultKeymap, historyKeymap, indentWithTab and Kudzu's ⌘B/I/U)

    pub fn key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> Effects {
        let mut fx = Effects::default();
        if self.composing() {
            return fx;
        }
        let rev = self.doc.revision;
        let m = ev.mods;
        let t = now();
        let head = self.doc.sel.head;
        let anchor = self.doc.sel.anchor;
        let lay = self.layout();
        let handled = match &ev.key {
            Key::Text(s) if !m.cmd && !m.ctrl => {
                let at = self.doc.sel.from();
                let slash = s == "/" && is_word_start_before(self.doc.units(), at);
                self.doc.type_text(s, t);
                self.goal_x = None;
                self.moved();
                if slash {
                    fx.slash = Some(at);
                }
                true
            }
            Key::Text(s) if m.ctrl && !m.cmd && !m.alt => self.emacs(s, m.shift, &lay, t),
            Key::Enter if m.cmd => {
                self.doc.insert_blank_line(t);
                self.moved();
                true
            }
            Key::Enter if !m.ctrl && !m.alt => {
                self.doc.newline_and_indent(t);
                self.moved();
                true
            }
            Key::Backspace => {
                if m.cmd {
                    let (start, _) = lay.line_bounds(head, self.before);
                    self.doc.delete_to_pos(start, t);
                } else if m.alt {
                    self.doc.delete_group(false, t);
                } else {
                    self.doc.backspace(t);
                }
                self.goal_x = None;
                self.moved();
                true
            }
            Key::Delete => {
                if m.cmd {
                    let (_, end) = lay.line_bounds(head, self.before);
                    self.doc.delete_to_pos(end, t);
                } else if m.alt {
                    self.doc.delete_group(true, t);
                } else {
                    self.doc.delete_forward(t);
                }
                self.goal_x = None;
                self.moved();
                true
            }
            Key::Left | Key::Right => {
                let fwd = matches!(ev.key, Key::Right);
                let target = if m.cmd {
                    self.line_boundary(head, fwd, &lay)
                } else if m.alt {
                    self.doc.group_stop(head, fwd)
                } else if !m.shift && !self.doc.sel.is_empty() {
                    if fwd { self.doc.sel.to() } else { self.doc.sel.from() }
                } else if fwd {
                    self.doc.next_stop(head)
                } else {
                    self.doc.prev_stop(head)
                };
                let before = m.cmd && fwd && self.is_soft_end(target, &lay);
                self.set_sel(if m.shift { anchor } else { target }, target, false);
                self.before = before;
                true
            }
            Key::Up | Key::Down => {
                let down = matches!(ev.key, Key::Down);
                if m.alt && !m.cmd {
                    if m.shift { self.doc.copy_lines(down, t) } else { self.doc.move_lines(down, t); }
                    self.moved();
                } else if m.cmd {
                    let target = if down { self.doc.len() } else { 0 };
                    self.set_sel(if m.shift { anchor } else { target }, target, false);
                } else if !m.shift && !self.doc.sel.is_empty() {
                    let target = if down { self.doc.sel.to() } else { self.doc.sel.from() };
                    self.set_sel(target, target, false);
                } else {
                    self.vertical(down, m.shift, &lay);
                }
                true
            }
            Key::Home | Key::End => {
                let fwd = matches!(ev.key, Key::End);
                let target = if m.cmd { if fwd { self.doc.len() } else { 0 } } else { self.line_boundary(head, fwd, &lay) };
                let before = fwd && !m.cmd && self.is_soft_end(target, &lay);
                self.set_sel(if m.shift { anchor } else { target }, target, false);
                self.before = before;
                true
            }
            Key::Tab if !m.cmd && !m.alt && !m.ctrl => {
                if m.shift { self.doc.indent_less(t) } else { self.doc.indent_more(t) }
                self.moved();
                true
            }
            Key::Escape => {
                if self.doc.sel.is_empty() {
                    fx.escape = true;
                    true
                } else {
                    self.set_sel(head, head, false);
                    true
                }
            }
            Key::Command(c) if m.cmd && !m.ctrl => match (*c, m.shift, m.alt) {
                ('a', false, false) => {
                    self.set_sel(0, self.doc.len(), false);
                    true
                }
                ('z', false, false) => {
                    self.doc.undo();
                    self.goal_x = None;
                    self.caret_gen += 1;
                    true
                }
                ('z', true, false) => {
                    self.doc.redo();
                    self.goal_x = None;
                    self.caret_gen += 1;
                    true
                }
                ('c', false, false) | ('x', false, false) => {
                    let (from, to) = self.copied_range();
                    tx.copy(self.doc.slice(from, to));
                    if *c == 'x' {
                        self.doc.replace(from, to, "", Some(Sel::cursor(from)), doc::Event::Delete, t);
                        self.moved();
                    }
                    true
                }
                ('b', false, false) => self.format(FormatStyle::Bold),
                ('i', false, false) => self.format(FormatStyle::Italic),
                ('u', false, false) => self.format(FormatStyle::Underline),
                (']', false, false) => {
                    self.doc.indent_more(t);
                    true
                }
                ('[', false, false) => {
                    self.doc.indent_less(t);
                    true
                }
                ('k', true, false) => {
                    self.doc.delete_lines(t);
                    self.moved();
                    true
                }
                _ => false,
            },
            _ => false,
        };
        fx.handled = handled;
        fx.changed = self.doc.revision != rev;
        if fx.changed {
            self.caret_gen = self.caret_gen.wrapping_add(1);
        }
        fx
    }

    /// The Emacs bindings macOS text views (and CodeMirror on a Mac) keep.
    fn emacs(&mut self, s: &str, shift: bool, lay: &Layout, t: f64) -> bool {
        let head = self.doc.sel.head;
        let anchor = self.doc.sel.anchor;
        let go = |me: &mut Self, target: usize| {
            me.set_sel(if shift { anchor } else { target }, target, false);
        };
        match s {
            "a" => go(self, lay.line_bounds(head, self.before).0),
            "e" => go(self, lay.line_bounds(head, self.before).1),
            "b" => go(self, self.doc.prev_stop(head)),
            "f" => go(self, self.doc.next_stop(head)),
            "p" | "n" => self.vertical(s == "n", shift, lay),
            "d" => {
                self.doc.delete_forward(t);
                self.moved();
            }
            "h" => {
                self.doc.backspace(t);
                self.moved();
            }
            "k" => {
                let end = self.doc.line_end(head);
                let target = if end == head { (end + 1).min(self.doc.len()) } else { end };
                self.doc.delete_to_pos(target, t);
                self.moved();
            }
            "t" => {
                self.doc.transpose(t);
                self.moved();
            }
            "o" => {
                self.doc.split_line(t);
                self.moved();
            }
            _ => return false,
        }
        true
    }

    fn format(&mut self, style: FormatStyle) -> bool {
        if !self.doc.formatted() {
            return false;
        }
        self.doc.change_format(&FormatChange::Style(style), now());
        true
    }

    /// CodeMirror copies the whole line when the selection is empty.
    fn copied_range(&self) -> (usize, usize) {
        if self.doc.sel.is_empty() {
            let h = self.doc.sel.head;
            let (s, e) = (self.doc.line_start(h), self.doc.line_end(h));
            (s, (e + 1).min(self.doc.len()))
        } else {
            (self.doc.sel.from(), self.doc.sel.to())
        }
    }

    fn is_soft_end(&self, pos: usize, lay: &Layout) -> bool {
        lay.lines.iter().any(|l| l.to == pos && !l.hard && l.from != l.to)
    }

    /// `moveByLineBoundary`: the visual line's end, or its start (after the indentation first).
    fn line_boundary(&self, head: usize, forward: bool, lay: &Layout) -> usize {
        let (start, end) = lay.line_bounds(head, self.before);
        if forward {
            return end;
        }
        let line_start = self.doc.line_start(head);
        if start == line_start {
            let spaces = self.doc.units()[line_start..].iter().take_while(|&&u| u == b' ' as u16 || u == b'\t' as u16).count();
            if spaces > 0 && head != line_start + spaces {
                return line_start + spaces;
            }
        }
        start
    }

    fn vertical(&mut self, down: bool, extend: bool, lay: &Layout) {
        let head = self.doc.sel.head;
        let li = lay.line_of(head, self.before);
        let goal = self.goal_x.unwrap_or_else(|| lay.lines.get(li).map_or(PAD_X, |l| l.x_of(head)));
        let target = match lay.vertical(head, goal, down, self.before) {
            Some(p) => p,
            None => if down { self.doc.len() } else { 0 },
        };
        let anchor = if extend { self.doc.sel.anchor } else { target };
        let before = self.is_soft_end(target, lay) && lay.line_of(target, true) != li;
        self.doc.select(anchor, target);
        self.goal_x = Some(goal);
        self.before = before;
        self.moved();
    }

    /// An input method committed into the proxy: move it into the document.
    pub fn take_proxy_text(&mut self) -> Effects {
        let mut fx = Effects::default();
        if self.proxy.text.is_empty() {
            return fx;
        }
        let s = std::mem::take(&mut self.proxy.text);
        self.proxy.clear();
        let at = self.doc.sel.from();
        let slash = s == "/" && is_word_start_before(self.doc.units(), at);
        self.doc.type_text(&s, now());
        self.moved();
        fx.changed = true;
        fx.handled = true;
        if slash {
            fx.slash = Some(at);
        }
        fx
    }

    pub fn paste(&mut self, s: &str) -> Effects {
        let rev = self.doc.revision;
        self.doc.paste(s, now());
        self.moved();
        Effects { changed: self.doc.revision != rev, handled: true, ..Default::default() }
    }

    // MARK: Pointer

    pub fn drag(&mut self, d: &Drag, shift: bool) -> Effects {
        let mut fx = Effects { handled: true, ..Default::default() };
        let lay = self.layout();
        match d.phase {
            Phase::Down => {
                if d.clicks <= 1
                    && !shift
                    && let Some(f) = lay.atom_at(d.x, d.y)
                {
                    {
                        match f.kind {
                            Kind::Chip { .. } => {
                                fx.chip = Some(f.from);
                                return fx;
                            }
                            Kind::Image { .. } => {
                                let (a, b) = (f.from, f.to);
                                self.focus();
                                self.set_sel(a, b, false);
                                return fx;
                            }
                            _ => {}
                        }
                    }
                }
                self.focus();
                let pos = lay.pos_at(d.x, d.y);
                let before = self.is_soft_end(pos, &lay) && lay.line_of(pos, true) == lay.lines.iter().position(|l| d.y < l.top + l.height).unwrap_or(0);
                match d.clicks {
                    0 | 1 => {
                        self.drag_unit = Unit::Char;
                        let anchor = if shift { self.doc.sel.anchor } else { pos };
                        self.set_sel(anchor, pos, false);
                        self.before = before;
                    }
                    2 => {
                        self.drag_unit = Unit::Word;
                        let (a, b) = self.word_at(pos);
                        self.drag_origin = (a, b);
                        self.set_sel(a, b, false);
                    }
                    _ => {
                        self.drag_unit = Unit::Line;
                        let (a, b) = self.line_at(pos);
                        self.drag_origin = (a, b);
                        self.set_sel(a, b, false);
                    }
                }
            }
            Phase::Move => {
                let pos = lay.pos_at(d.x, d.y);
                match self.drag_unit {
                    Unit::Char => {
                        let a = self.doc.sel.anchor;
                        self.set_sel(a, pos, false);
                    }
                    Unit::Word | Unit::Line => {
                        let (a, b) = if self.drag_unit == Unit::Word { self.word_at(pos) } else { self.line_at(pos) };
                        let (oa, ob) = self.drag_origin;
                        if pos < oa {
                            self.set_sel(ob, a, false);
                        } else {
                            self.set_sel(oa, b.max(ob), false);
                        }
                    }
                }
            }
            // A press blurs whatever had focus as it lands; take it back once it has.
            Phase::Up => {
                if !d.cancelled() {
                    self.focus();
                }
            }
        }
        fx
    }

    /// The word (or run of spaces or punctuation, or the chip) around `pos`.
    fn word_at(&self, pos: usize) -> (usize, usize) {
        let d = &self.doc;
        if let Some(&(a, b)) = d.atoms().iter().find(|&&(a, b)| a <= pos && pos < b) {
            return (a, b);
        }
        let start = d.line_start(pos);
        let end = d.line_end(pos);
        let a = if pos > start { d.group_stop(pos, false).max(start) } else { pos };
        let b = if pos < end { d.group_stop(a, true).min(end) } else { pos };
        if a == b { (d.prev_stop(pos).max(start), pos) } else { (a, b) }
    }

    fn line_at(&self, pos: usize) -> (usize, usize) {
        let d = &self.doc;
        let (s, e) = (d.line_start(pos), d.line_end(pos));
        (s, (e + 1).min(d.len()))
    }
}

// MARK: View

/// `.kz-chip-*` tone colours.
pub fn chip_color(t: &crate::ui::theme::Tokens, tone: ChipTone) -> Color {
    match tone {
        ChipTone::Time => t.chip_time,
        ChipTone::Clipboard => t.chip_clipboard,
        ChipTone::Cursor => t.chip_cursor,
        ChipTone::Form => t.chip_form,
        ChipTone::Snippet => t.chip_neutral,
        ChipTone::Error => t.chip_error,
    }
}

/// A chip as the editor draws it, `w` wide (margins excluded).
pub fn chip(th: &Theme, label: &str, tone: ChipTone, verbatim: bool, italic: bool, help: &str, w: f32) -> El {
    let t = &th.t;
    let fg = chip_color(t, tone);
    let ring = |p: f32| Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: Theme::tint(fg, p), inset: true };
    let font = if verbatim { CHIP_FONT_MONO } else { CHIP_FONT };
    let font = if italic { font.italic() } else { font };
    let hover = crate::ui::kit::button::layer(0.0).radius(6.0).box_shadow(ring(0.70)).group_opacity(0.0, 1.0);
    let e = row()
        .w(w)
        .h(layout::CHIP_H)
        .px(layout::CHIP_PAD)
        .center_y()
        .radius(6.0)
        .fill(Theme::tint(fg, 0.13))
        .box_shadow(ring(0.24))
        .cursor(Cursor::Pointer)
        .help(help.to_string())
        .label(format!("{label} — click to edit"))
        .role("button")
        .child(hover)
        .child(text(label.to_string()).font(font).color(fg));
    hover_tween(e)
}

fn hover_tween(e: El) -> El {
    use crate::ui::kit::button::tween;
    e.hover_group().hover_motion(tween(0.12, rune::Easing::EASE), tween(0.12, rune::Easing::EASE))
}

pub struct ViewParts {
    pub el: El,
}

/// The editor surface: `width` wide, at least `min_h` tall. `access` reaches this state in
/// `S`; `on_fx` hears what edits ask for.
#[allow(clippy::too_many_arguments)]
pub fn view<S: 'static>(
    th: &Theme,
    st: &TemplateState,
    width: f32,
    min_h: f32,
    placeholder: &str,
    label: &str,
    snippet_name: &dyn Fn(&str) -> Option<String>,
    access: fn(&mut S) -> &mut TemplateState,
    on_fx: fn(&mut S, Effects, &mut Tx),
) -> ViewParts {
    let t = &th.t;
    let d = &st.doc;
    let formatted = d.formatted();
    let preedit = st.proxy.preedit.as_ref().filter(|p| !p.0.is_empty());
    let lay = Rc::new(layout::layout(&layout::Input {
        text: d.text(),
        spans: d.spans(),
        width,
        snippet_name,
        preedit: preedit.map(|p| (p.0.as_str(), d.sel.head)),
    }));
    *st.layout.borrow_mut() = lay.clone();
    let height = lay.height.max(min_h);
    let focused = st.focused();
    let mut kids: Vec<El> = Vec::new();

    // Selection (`.cm-selectionBackground`, iris-soft), under the text.
    if !d.sel.is_empty() && preedit.is_none() {
        for (i, r) in lay.selection_rects(d.sel.from(), d.sel.to()).into_iter().enumerate() {
            kids.push(el().key(format!("kz-sel-{i}")).absolute(r[0], r[1]).size(r[2], r[3]).fill(t.iris_soft).a11y_hidden());
        }
    }

    // Text, chips and images.
    let fg = t.foreground;
    let mut preedit_caret: Option<[f32; 4]> = None;
    for (li, line) in lay.lines.iter().enumerate() {
        for (fi, f) in line.frags.iter().enumerate() {
            let key = format!("kz-f-{li}-{fi}");
            match &f.kind {
                Kind::Text { text: s, style, underline, link } => {
                    let color = if *link { t.iris } else { fg };
                    let mut span = Span::new(0, s.len()).color(color);
                    if *underline {
                        span = span.underline(color);
                    }
                    kids.push(rich(s.clone(), vec![span]).font(*style).color(color).key(key).absolute(f.x, f.y).a11y_hidden());
                }
                Kind::Chip { label, tone, verbatim, source, italic } => {
                    let help = if *verbatim { format!("Not recognised: {source}") } else { source.clone() };
                    kids.push(chip(th, label, *tone, *verbatim, *italic, &help, f.w - 2.0 * CHIP_MARGIN).key(key).absolute(f.x + CHIP_MARGIN, f.y));
                }
                Kind::Image { url, alt } => {
                    let mut img = el().key(key).absolute(f.x, f.y).size(f.w, f.h).radius(radius::SM).label(alt.clone()).role("img");
                    if let Some(i) = layout::image(url) {
                        img = img.pixels(&i.pixels);
                    }
                    kids.push(img);
                }
                Kind::Preedit { text: s, style } => {
                    let span = Span::new(0, s.len()).color(fg).underline(fg);
                    kids.push(rich(s.clone(), vec![span]).font(*style).color(fg).key(key).absolute(f.x, f.y).a11y_hidden());
                    let cursor = st.proxy.preedit.as_ref().and_then(|p| p.1).map_or(s.len(), |c| c.1.min(s.len()));
                    let x = f.x + layout::shape(&s[..cursor], *style).width();
                    preedit_caret = Some([x, line.baseline - layout::SIZE * 0.95, 2.0, (layout::SIZE * 1.2).round()]);
                }
                Kind::Tab => {}
            }
        }
    }

    // Placeholder (`.cm-placeholder`: faint, sans).
    if d.is_empty() && preedit.is_none() {
        kids.push(
            text(placeholder.to_string())
                .font(layout::base_font(true))
                .color(t.faint)
                .wrap()
                .key("kz-placeholder")
                .absolute(PAD_X, PAD_Y)
                .w((width - 2.0 * PAD_X).max(0.0))
                .a11y_hidden(),
        );
    }

    // The caret: 2 pt iris, blinking (`.cm-cursor`), restarted on every move.
    let caret = preedit_caret.unwrap_or_else(|| lay.caret(d.sel.head, st.before));
    if focused && (d.sel.is_empty() || preedit.is_some()) {
        kids.push(
            el().key(format!("kz-caret-{}", st.caret_gen))
                .absolute(caret[0] - 1.0, caret[1])
                .size(caret[2], caret[3])
                .fill(t.iris)
                .looping(LoopSpec::opacity(1.2, 0.5, 1.0, 0.0))
                .a11y_hidden(),
        );
    }

    // The proxy field at the caret: keyboard, input method and paste.
    let proxy = text_input(&st.proxy, layout::base_font(formatted))
        .key("kz-proxy")
        .label(label.to_string())
        .absolute(caret[0], caret[1])
        .size(1.0, LINE_H.min(caret[3].max(1.0)))
        .color(Color::TRANSPARENT)
        // Invisible but drawn: Rune only focuses a field that shows (opacity above 0).
        .caret(Color::TRANSPARENT, Color::TRANSPARENT)
        .on_edit(move |s: &mut S| &mut access(s).proxy)
        .on_input_key(move |s: &mut S, ev: &KeyEvent, tx: &mut Tx| {
            let fx = access(s).key(ev, tx);
            let handled = fx.handled;
            on_fx(s, fx, tx);
            handled
        })
        .on_input_change(move |s: &mut S, tx: &mut Tx| {
            let fx = access(s).take_proxy_text();
            if fx.handled {
                on_fx(s, fx, tx);
            }
        })
        .on_input_paste(move |s: &mut S, content, tx: &mut Tx| {
            if content.text().is_empty() {
                return true;
            }
            let fx = access(s).paste(content.text());
            on_fx(s, fx, tx);
            true
        });
    kids.push(proxy);

    let host = el()
        .key("kz-template")
        .w_full()
        .h(height)
        .cursor(Cursor::Text)
        .preserve_focus_on_press(true)
        .on_drag(move |s: &mut S, drag: &mut Drag, tx: &mut Tx| {
            let shift = tx.mods().shift;
            let fx = access(s).drag(drag, shift);
            on_fx(s, fx, tx);
        })
        .children(kids);
    ViewParts { el: host }
}

#[cfg(test)]
mod tests {
    use rune::{col, Cx, El, Env, Ime, Key, KeyEvent, Mods, Runtime, Tx};

    use super::*;

    /// The editor alone in a window, recording what it asks for.
    #[derive(Default)]
    struct Host {
        t: TemplateState,
        fx: Vec<Effects>,
    }

    fn access(s: &mut Host) -> &mut TemplateState {
        &mut s.t
    }

    fn on_fx(s: &mut Host, fx: Effects, _: &mut Tx) {
        s.fx.push(fx);
    }

    fn view(s: &Host, _cx: &mut Cx<Host>) -> El {
        let th = crate::ui::theme::theme(true);
        let parts = super::view::<Host>(th, &s.t, 400.0, 168.0, "Type…", "Snippet content", &|_| None, access, on_fx);
        col().w(400.0).child(parts.el)
    }

    fn host(text: &str, spans: Option<Vec<kudzu_core::formatting::FormatSpan>>) -> Runtime<Host> {
        let mut rt = Runtime::new(Host { t: TemplateState::new(text, spans), fx: Vec::new() }, view, (400.0, 300.0), Env::new(true, 2.0));
        rt.frame(0.0);
        rt
    }

    fn key(rt: &mut Runtime<Host>, key: Key, mods: Mods) {
        rt.key(KeyEvent { key, mods });
        rt.frame(0.016);
    }

    #[test]
    fn a_press_focuses_and_typing_reaches_the_document() {
        let mut rt = host("Hi {{cursor}}", None);
        let lay = rt.state.t.layout.borrow().clone();
        let end = lay.lines[0].x_of(13) + 30.0;
        rt.pointer_down(end, lay.lines[0].top + 5.0);
        rt.pointer_up(end, lay.lines[0].top + 5.0);
        rt.frame(0.016);
        rt.frame(0.016);
        assert!(rt.state.t.focused(), "the proxy field holds focus after a press");
        assert_eq!(rt.state.t.doc.sel.head, 13);
        key(&mut rt, Key::Text("!".into()), Mods::default());
        assert_eq!(rt.state.t.doc.text(), "Hi {{cursor}}!");
        assert!(rt.state.fx.iter().any(|f| f.changed));
        // Backspace twice: the "!" and then the whole chip.
        key(&mut rt, Key::Backspace, Mods::default());
        key(&mut rt, Key::Backspace, Mods::default());
        assert_eq!(rt.state.t.doc.text(), "Hi ");
        key(&mut rt, Key::Command('z'), Mods { cmd: true, ..Default::default() });
        assert_eq!(rt.state.t.doc.text(), "Hi {{cursor}}");
    }

    #[test]
    fn the_input_method_composes_inline_and_commits_into_the_document() {
        let mut rt = host("a", None);
        rt.update(|s, _| {
            s.t.focus();
            s.t.doc.select(1, 1);
        });
        rt.frame(0.016);
        rt.frame(0.016);
        assert!(rt.state.t.focused());
        rt.ime(Ime::Preedit("か".into(), Some((0, 3))));
        rt.frame(0.016);
        assert_eq!(rt.state.t.doc.text(), "a", "a composition isn't text yet");
        assert!(rt.state.t.composing());
        let lay = rt.state.t.layout.borrow().clone();
        assert!(lay.lines[0].frags.iter().any(|f| matches!(&f.kind, Kind::Preedit { text, .. } if text == "か")), "drawn inline at the caret");
        rt.ime(Ime::Commit("家".into()));
        rt.frame(0.016);
        rt.frame(0.016);
        assert_eq!(rt.state.t.doc.text(), "a家");
        assert!(rt.state.t.proxy.text.is_empty(), "the proxy never keeps text");
        assert!(!rt.state.t.composing());
    }

    #[test]
    fn slash_at_a_word_start_asks_for_the_menu_and_paste_makes_chips() {
        let mut rt = host("x", None);
        rt.update(|s, _| {
            s.t.focus();
            s.t.doc.select(1, 1);
        });
        rt.frame(0.016);
        rt.frame(0.016);
        key(&mut rt, Key::Text("/".into()), Mods::default());
        assert_eq!(rt.state.fx.last().and_then(|f| f.slash), None, "not after a letter");
        key(&mut rt, Key::Text(" ".into()), Mods::default());
        key(&mut rt, Key::Text("/".into()), Mods::default());
        assert_eq!(rt.state.fx.last().and_then(|f| f.slash), Some(3));
        rt.paste("{{clipboard}}");
        rt.frame(0.016);
        assert_eq!(rt.state.t.doc.text(), "x/ /{{clipboard}}");
        assert_eq!(rt.state.t.doc.tags().len(), 1);
    }

    #[test]
    fn pressing_a_chip_asks_to_configure_it_and_an_image_selects_whole() {
        const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAAJUlEQVR4nGMICLj0n5aYYdSCUQtGLRi1YNSCUQtGLRi1YGhYAACpwH6M7RqhEQAAAABJRU5ErkJggg==";
        let spans = vec![kudzu_core::formatting::FormatSpan { start: 0, end: 6, style: FormatStyle::Image { data_url: PNG.into(), width: 24 } }];
        let mut rt = host("[logo] {{cursor}}", Some(spans));
        let lay = rt.state.t.layout.borrow().clone();
        let img = lay.lines[0].frags.iter().find(|f| matches!(f.kind, Kind::Image { .. })).unwrap().clone();
        assert!((img.w - 24.0).abs() < 0.01 && (img.h - 24.0).abs() < 0.01, "shown at its display width");
        assert!(lay.lines[0].height >= 24.0, "the line grows around the image");
        rt.pointer_down(img.x + 5.0, img.y + 5.0);
        rt.pointer_up(img.x + 5.0, img.y + 5.0);
        rt.frame(0.016);
        assert_eq!((rt.state.t.doc.sel.from(), rt.state.t.doc.sel.to()), (0, 6));
        let chip = lay.lines[0].frags.iter().find(|f| matches!(f.kind, Kind::Chip { .. })).unwrap().clone();
        rt.pointer_down(chip.x + 10.0, chip.y + 5.0);
        rt.pointer_up(chip.x + 10.0, chip.y + 5.0);
        rt.frame(0.016);
        assert!(rt.state.fx.iter().any(|f| f.chip == Some(7)));
        // Typing over the selected image replaces its text (spans map as CodeMirror maps them).
        rt.update(|s, _| {
            s.t.focus();
            s.t.doc.select(0, 6);
        });
        rt.frame(0.016);
        rt.frame(0.016);
        key(&mut rt, Key::Text("A".into()), Mods::default());
        assert_eq!(rt.state.t.doc.text(), "A {{cursor}}");
        key(&mut rt, Key::Command('z'), Mods { cmd: true, ..Default::default() });
        assert_eq!(rt.state.t.doc.text(), "[logo] {{cursor}}");
        assert_eq!(rt.state.t.doc.spans().unwrap()[0].end, 6, "undo brings the image back");
    }
}
