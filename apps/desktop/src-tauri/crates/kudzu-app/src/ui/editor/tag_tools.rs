//! `tag-tools.tsx`: the two ways to work with template tags without knowing the syntax.
//!
//! - The insert menu: `/` at a word's start in the editor, or the Insert button, opens a
//!   searchable list of variables and snippets.
//! - The chip editor: pressing a chip opens a small form for that tag, applied live (each change
//!   rewrites the tag's source in place; one undo step while you keep editing it).

use kudzu_core::client::template::{build_tag, join_options, split_options, TagToken, DATE_FORMAT_PRESETS};
use kudzu_core::template::AttributeValue;
use rune::{col, el, icon, row, text, Align, Cx, El, Family, Key, KeyEvent, PopupMenu, Side, TextEdit, TextStyle, Tx};

use super::{ensure, pick, EditorState};
use crate::ui::icons;
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::command::{Command, CommandItem, CommandState};
use crate::ui::kit::input::Input;
use crate::ui::kit::popover::Popover;
use crate::ui::kit::selection_pill::SelectionPill;
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::kit::switch::Switch;
use crate::ui::kit::textarea::Textarea;
use crate::ui::kit::trigger_chip::trigger_chip;
use crate::ui::main::MainUi;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

#[derive(Clone, Default)]
pub struct TagToolsState {
    /// The insert menu, opened at a typed `/` (its offset) or from the Insert button (`None`).
    pub insert: Option<Option<usize>>,
    pub cmd: CommandState,
    pub chip: Option<ChipOpen>,
}

#[derive(Clone)]
pub struct ChipOpen {
    pub from: usize,
    pub to: usize,
    pub tag: TagToken,
    pub name: String,
    /// The tag's attributes in object order; `None` values are left out of the source.
    pub attrs: Vec<(String, Option<AttributeValue>)>,
    pub label: TextEdit,
    pub default: TextEdit,
    pub options: TextEdit,
    pub custom: bool,
    pub format: TextEdit,
    pub offset: TextEdit,
    pub locale: TextEdit,
    pub cmd: CommandState,
}

impl ChipOpen {
    fn str(&self, k: &str) -> Option<String> {
        match self.attrs.iter().find(|(key, _)| key == k).and_then(|(_, v)| v.clone()) {
            Some(AttributeValue::String(s)) => Some(s),
            Some(v) => Some(kudzu_core::client::template::value_string(&v)),
            None => None,
        }
    }
    fn truthy(&self, k: &str) -> bool {
        kudzu_core::client::template::truthy(self.attrs.iter().find(|(key, _)| key == k).and_then(|(_, v)| v.as_ref()))
    }
    fn source(&self) -> String {
        build_tag(&self.name, self.attrs.iter().map(|(k, v)| (k.as_str(), v.clone())))
    }
}

struct Variable {
    icon: &'static rune::Icon,
    label: &'static str,
    hint: &'static str,
    tag: fn() -> String,
    configure: bool,
    keywords: &'static str,
}

fn s(v: &str) -> Option<AttributeValue> {
    Some(AttributeValue::String(v.into()))
}

const VARIABLES: [Variable; 6] = [
    Variable { icon: &icons::CALENDAR_DAYS, label: "Date", hint: "Today, or any day relative to it", tag: || build_tag("date", [("format", s("MMMM d, yyyy"))]), configure: false, keywords: "today day calendar" },
    Variable { icon: &icons::CLOCK, label: "Time", hint: "The current time", tag: || build_tag("time", [("format", s("h:mm a"))]), configure: false, keywords: "clock hour now" },
    Variable { icon: &icons::CLIPBOARD, label: "Clipboard", hint: "Whatever you last copied", tag: || build_tag::<&str>("clipboard", []), configure: false, keywords: "paste copy" },
    Variable { icon: &icons::TEXT_CURSOR, label: "Cursor position", hint: "Where the caret lands after expanding", tag: || build_tag::<&str>("cursor", []), configure: false, keywords: "caret" },
    Variable { icon: &icons::TEXT_CURSOR_INPUT, label: "Fill-in field", hint: "Asks you for text each time", tag: || build_tag("field", [("name", s("Name"))]), configure: true, keywords: "form input ask prompt variable" },
    Variable { icon: &icons::LIST_CHECKS, label: "Dropdown", hint: "Pick from a list each time", tag: || build_tag("choice", [("name", s("Option")), ("options", s("Yes|No"))]), configure: true, keywords: "form select list options" },
];

const OFFSET_PRESETS: [(&str, &str); 6] = [("", "Today / now"), ("+1d", "Tomorrow"), ("-1d", "Yesterday"), ("+1w", "In a week"), ("+1m", "In a month"), ("+1h", "In an hour")];

// MARK: State changes

pub fn open_insert(s: &mut MainUi, slash: Option<usize>) {
    ensure(s);
    s.editor.tags.chip = None;
    s.editor.tags.insert = Some(slash);
    s.editor.tags.cmd.reset();
    s.editor.tags.cmd.search.focused = true;
}

fn close_insert(s: &mut MainUi) {
    s.editor.tags.insert = None;
    s.editor.template.focus();
}

pub fn open_chip(s: &mut MainUi, from: usize) {
    ensure(s);
    let Some(tag) = s.editor.template.doc.tag_at(from) else { return };
    s.editor.tags.insert = None;
    let attrs: Vec<(String, Option<AttributeValue>)> = tag.attrs.iter().map(|(k, v)| (k.to_string(), Some(v.clone()))).collect();
    let get = |k: &str| match tag.attrs.get(k) {
        Some(AttributeValue::String(s)) => s.clone(),
        Some(v) => kudzu_core::client::template::value_string(v),
        None => String::new(),
    };
    let format = match tag.attrs.get("format") {
        Some(_) => get("format"),
        None => if tag.name == "date" { "yyyy-MM-dd".into() } else { "HH:mm".into() },
    };
    let custom = !DATE_FORMAT_PRESETS.iter().any(|(f, _)| *f == format);
    let mut open = ChipOpen {
        from: tag.from,
        to: tag.to,
        name: tag.name.clone(),
        attrs,
        label: TextEdit::new(get("name")),
        default: TextEdit::new(get("default")),
        options: TextEdit::new(split_options(&get("options")).join("\n")),
        custom,
        format: TextEdit::new(format),
        offset: TextEdit::new(get("offset")),
        locale: TextEdit::new(get("locale")),
        cmd: CommandState::default(),
        tag,
    };
    // `autoFocus` on the form's first field.
    match open.name.as_str() {
        "field" | "choice" => open.label.focused = true,
        "snippet" => {
            open.cmd.search.focused = true;
            open.cmd.selected = open.str("id");
        }
        _ => {}
    }
    s.editor.tags.chip = Some(open);
}

fn close_chip(s: &mut MainUi) {
    s.editor.tags.chip = None;
    s.editor.template.focus();
}

/// `set(patch, name)`: applies attribute changes and rewrites the tag in the editor.
fn set(s: &mut MainUi, patch: Vec<(&str, Option<AttributeValue>)>, name: Option<&str>) {
    ensure(s);
    let Some(c) = s.editor.tags.chip.as_mut() else { return };
    for (k, v) in patch {
        match c.attrs.iter_mut().find(|(key, _)| key == k) {
            Some(slot) => slot.1 = v,
            None => c.attrs.push((k.to_string(), v)),
        }
    }
    if let Some(n) = name {
        c.name = n.to_string();
    }
    let source = c.source();
    let (from, to) = (c.from, c.to);
    let t = &mut s.editor.template;
    let r = t.doc.replace(from, to, &source, None, super::template::doc::Event::Input, super::template::now());
    if let Some(c) = s.editor.tags.chip.as_mut() {
        c.from = r.0;
        c.to = r.1;
    }
    super::template_changed(s);
}

fn nonempty(v: String) -> Option<AttributeValue> {
    (!v.is_empty()).then_some(AttributeValue::String(v))
}

fn bool_attr(v: bool) -> Option<AttributeValue> {
    v.then_some(AttributeValue::Bool(true))
}

fn remove(s: &mut MainUi) {
    ensure(s);
    let Some(c) = s.editor.tags.chip.take() else { return };
    let t = &mut s.editor.template;
    t.replace(c.from, c.to, "");
    super::template_changed(s);
    s.editor.template.focus();
}

// MARK: Views

/// Popovers anchored inside the editor: the menu at a typed `/` and the chip editor. Their
/// anchors are positioned in the editor surface (`kz-template`).
pub fn anchors(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let lay = st.template.layout.borrow().clone();
    let mut out = el().key("kz-anchors").absolute(0.0, 0.0).size(0.0, 0.0);
    if let Some(Some(at)) = st.tags.insert {
        let li = lay.line_of(at, false);
        if let Some(l) = lay.lines.get(li) {
            let c = lay.caret(at, false);
            let x = l.x_of(at);
            let pop = insert_popover(s, st, cx, layer, el().w_full().h_full(), "kz-insert-slash", 6.0)
                .absolute(x, c[1])
                .size(1.0, c[3])
                .inert();
            out = out.child(pop);
        }
    }
    if let Some(c) = &st.tags.chip
        && let Some(f) = lay.frag_from(c.from)
    {
        let pop = chip_popover(s, st, cx, layer).absolute(f.x + super::template::layout::CHIP_MARGIN, f.y).size(f.w - 2.0 * super::template::layout::CHIP_MARGIN, f.h).inert();
        out = out.child(pop);
    }
    out
}

/// The Insert button with its menu.
pub fn insert_button(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>, button: El) -> El {
    insert_popover(s, st, cx, layer, button, "kz-insert-button", 6.0)
}

fn insert_popover(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>, trigger: El, key: &str, offset: f32) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let slash_pop = key == "kz-insert-slash";
    let open = match st.tags.insert {
        Some(Some(_)) => slash_pop,
        Some(None) => !slash_pop,
        None => false,
    };
    let mut pop = Popover::new(key.to_string())
        .open(open)
        .on_open_change(move |s: &mut MainUi, o, _| if o { open_insert(s, None) } else { close_insert(s) })
        .trigger(trigger)
        .side(Side::Bottom)
        .align(Align::Start)
        .side_offset(offset)
        .width(320.0)
        .padding(0.0)
        .gap(0.0)
        .ring(false)
        .backdrop_blur(24.0)
        .label("Insert a variable or snippet");
    if open {
        let me = st.id;
        let vars = VARIABLES.iter().map(|v| {
            let configure = v.configure;
            let tag = v.tag;
            CommandItem::new(format!("{} {}", v.label, v.keywords))
                .child(
                    el().size(28.0, 28.0).mr(2.0).center().radius(radius::MD).fill(Theme::tint(t.foreground, 0.06)).child(icon(v.icon, 14.0).color(t.foreground)),
                )
                .child(
                    col().min_w(0.0).shrink()
                        .child(text(v.label).font(ty::SM.weight_of(weight::MEDIUM)).color(t.foreground))
                        .child(text(v.hint).font(ty::XS).color(t.muted_foreground).truncate().min_w(0.0)),
                )
                .on_select(move |s: &mut MainUi, _| {
                    let slash = s.editor.tags.insert.flatten();
                    s.editor.tags.insert = None;
                    pick(s, &tag(), configure, slash);
                })
        });
        let snippets = s.library.snippets.iter().filter(|x| Some(x.id) != me).take(400).map(|x| {
            let id = x.id;
            let mut item = CommandItem::new(format!("snippet {} {} {}", x.name, x.triggers.join(" "), x.id))
                .child(icon(&icons::CORNER_DOWN_RIGHT, 14.0).color(t.muted_foreground).mr(2.0))
                .text(if x.name.is_empty() { "Untitled".to_string() } else { x.name.clone() });
            if let Some(tr) = x.triggers.first() {
                item = item.child(row().key("trailing").grow(1.0).justify_end().child(trigger_chip(th, tr, true)));
            }
            item.on_select(move |s: &mut MainUi, _| {
                let slash = s.editor.tags.insert.flatten();
                s.editor.tags.insert = None;
                pick(s, &build_tag("snippet", [("id", Some(AttributeValue::String(id.to_string())))]), false, slash);
            })
        });
        let cmd = Command::new(format!("{key}-cmd"), &st.tags.cmd, |s: &mut MainUi| &mut s.editor.tags.cmd)
            .placeholder("Insert a variable or snippet…")
            .wrap(true)
            .max_height(320.0)
            .empty(text("Nothing matches.").font(ty::SM).color(t.foreground))
            .group(Some("Variables"), vars)
            .group(Some("Insert another snippet"), snippets)
            .build(cx);
        // The footer sits inside `<Command>`'s `p-1`, so its rule stops 4 pt from the edges.
        pop = pop.child(rune::col().w_full().shrink().min_h(0.0).child(cmd)).child(el().w_full().px(4.0).pb(4.0).child(footer(th, vec![
            row().gap(4.0).center_y().child(Shortcut::new("enter").build(th)).child(text("insert").font(ty::XS2).color(t.muted_foreground)),
            row().gap(4.0).center_y().child(Shortcut::new("escape").build(th)).child(text("close").font(ty::XS2).color(t.muted_foreground)),
        ])));
    }
    pop.build(cx, layer)
}

/// `flex items-center gap-3 border-t px-3 py-2 text-[11px] text-muted-foreground`.
fn footer(th: &Theme, parts: Vec<El>) -> El {
    row().w_full().min_h(35.0).gap(12.0).center_y().px(12.0).py(8.0).border_sides([1.0, 0.0, 0.0, 0.0], th.t.border).border_layout().children(parts)
}

/// `Field`: a label over its control, with an optional hint.
fn field(th: &Theme, label: &str, control: El, hint: Option<El>) -> El {
    let mut c = col().gap(6.0).w_full().child(text(label.to_string()).font(ty::XS.weight_of(weight::MEDIUM)).color(th.t.muted_foreground)).child(control);
    if let Some(h) = hint {
        c = c.child(h);
    }
    c
}

fn hint(th: &Theme, s: &str) -> El {
    text(s.to_string()).font(ty::XS2).color(th.t.faint).wrap()
}

const MONO_XS: TextStyle = TextStyle::new(12.0, weight::NORMAL).family(Family::Mono).lh(16.0);

/// A form field bound to one of the chip's `TextEdit`s, applying `apply` on each change.
fn input(th: &Theme, edit: &TextEdit, which: fn(&mut ChipOpen) -> &mut TextEdit, apply: fn(&mut MainUi, String), mono: bool, placeholder: &str, label: &str) -> El {
    let mut i = Input::new(edit)
        .height(32.0)
        .placeholder(placeholder.to_string())
        .label(label.to_string())
        .on_edit(move |s: &mut MainUi| {
            ensure(s);
            let c = s.editor.tags.chip.get_or_insert_with(unreachable_chip);
            which(c)
        })
        .on_change(move |s: &mut MainUi, _| {
            let v = s.editor.tags.chip.as_mut().map(|c| which(c).text.clone()).unwrap_or_default();
            apply(s, v);
        })
        .on_key(done_key);
    if mono {
        i = i.font(MONO_XS);
    }
    i.build(th)
}

/// A chip form only exists while open; the accessor never runs otherwise.
fn unreachable_chip() -> ChipOpen {
    ChipOpen {
        from: 0,
        to: 0,
        tag: kudzu_core::client::template::tokenize("{{cursor}}").into_iter().find_map(|t| if let kudzu_core::client::template::Token::Tag(t) = t { Some(t) } else { None }).expect("tag"),
        name: String::new(),
        attrs: Vec::new(),
        label: TextEdit::default(),
        default: TextEdit::default(),
        options: TextEdit::default(),
        custom: false,
        format: TextEdit::default(),
        offset: TextEdit::default(),
        locale: TextEdit::default(),
        cmd: CommandState::default(),
    }
}

/// Return closes the chip editor (outside a textarea).
fn done_key(s: &mut MainUi, ev: &KeyEvent, _tx: &mut Tx) -> bool {
    if matches!(ev.key, Key::Enter) && !ev.mods.cmd && !ev.mods.shift {
        close_chip(s);
        return true;
    }
    false
}

/// A native pop-up menu standing in for the web's `<select>`, `h-8 rounded-md border`.
fn select(th: &Theme, key: &str, label: String, chevron_only: bool, menu: impl Fn(&MainUi) -> PopupMenu + 'static, on_pick: fn(&mut MainUi, String)) -> El {
    let t = &th.t;
    let key_s = key.to_string();
    let indicator = if chevron_only {
        icon(&icons::CHEVRON_DOWN, 16.0).color(t.muted_foreground).a11y_hidden()
    } else {
        col().gap(-3.0)
            .child(icon(&icons::CHEVRON_UP, 9.0).color(t.foreground))
            .child(icon(&icons::CHEVRON_DOWN, 9.0).color(t.foreground))
            .a11y_hidden()
    };
    row()
        .key(key_s.clone())
        .w_full()
        .h(32.0)
        .pl(8.0)
        .pr(if chevron_only { 8.0 } else { 6.0 })
        .center_y()
        .radius(radius::MD)
        .border(1.0, t.border)
        .role("combobox")
        .label(label.clone())
        .focusable(true)
        .on_press_tx(move |s: &mut MainUi, tx: &mut Tx| {
            let m = menu(s);
            tx.popup_menu(m, key_s.as_str(), move |s: &mut MainUi, picked: Option<&str>, _| {
                if let Some(v) = picked {
                    on_pick(s, v.to_string());
                }
            });
        })
        .child(text(label).font(ty::SM).color(t.foreground).truncate().grow(1.0).min_w(0.0))
        .child(indicator)
}

fn chip_popover(s: &MainUi, st: &EditorState, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let Some(c) = &st.tags.chip else { return el() };
    let title = match c.name.as_str() {
        "date" => "Date",
        "time" => "Time",
        "clipboard" => "Clipboard",
        "cursor" => "Cursor position",
        "snippet" => "Nested snippet",
        "field" => "Fill-in field",
        "choice" => "Dropdown",
        _ => "Unrecognised tag",
    };
    let body: El = if c.tag.verbatim {
        col().w_full().gap(8.0)
            .child(
                row().w_full().items_start().gap(8.0).radius(radius::MD).fill(Theme::tint(t.destructive, 0.1)).padding(10.0)
                    .child(icon(&icons::TRIANGLE_ALERT, 14.0).color(t.destructive).mt(2.0))
                    .child(text("Kudzu doesn't understand this tag, so it will be typed exactly as written.").font(ty::XS).color(t.destructive).wrap().shrink()),
            )
            .child(el().radius(radius::MD).fill(Theme::tint(t.foreground, 0.05)).padding(8.0).child(text(c.tag.source.clone()).font(MONO_XS).color(t.foreground).wrap_anywhere().selectable(true)))
    } else {
        match c.name.as_str() {
            "date" | "time" => date_form(th, c, cx),
            "clipboard" => row().w_full().center_y().gap(12.0)
                .child(col().grow(1.0)
                    .child(text("Trim whitespace").font(ty::SM).color(t.foreground))
                    .child(text("Remove spaces and line breaks at both ends").font(ty::XS).color(t.muted_foreground).wrap()))
                .child(Switch::new(c.truthy("trim")).label("Trim whitespace").id("chip-trim").on_toggle(|s: &mut MainUi| {
                    let on = s.editor.tags.chip.as_ref().is_some_and(|c| c.truthy("trim"));
                    set(s, vec![("trim", bool_attr(!on))], None);
                }).build(th)),
            "cursor" => text("After the text is inserted, the caret jumps here — handy for filling in the middle of a template.").font(TextStyle::new(12.0, weight::NORMAL).lh(19.5)).color(t.muted_foreground).wrap(),
            "field" => col().w_full().gap(12.0)
                .child(field(th, "Label", input(th, &c.label, |c| &mut c.label, |s, v| set(s, vec![("name", Some(AttributeValue::String(v)))], None), false, "", "Label"), Some(hint(th, "Fields with the same label share one value."))))
                .child(field(th, "Default value", input(th, &c.default, |c| &mut c.default, |s, v| set(s, vec![("default", nonempty(v))], None), false, "Optional", "Default value"), None))
                .child(row().w_full().center_y().child(text("Multiple lines").font(ty::SM).color(t.foreground)).child(rune::spacer()).child(
                    Switch::new(c.truthy("multiline")).label("Multiple lines").id("chip-multiline").on_toggle(|s: &mut MainUi| {
                        let on = s.editor.tags.chip.as_ref().is_some_and(|c| c.truthy("multiline"));
                        set(s, vec![("multiline", bool_attr(!on))], None);
                    }).build(th),
                )),
            "choice" => {
                let options = split_options(&c.str("options").unwrap_or_default());
                let default = c.str("default").unwrap_or_default();
                let opts: Vec<String> = options.iter().filter(|o| !o.is_empty()).cloned().collect();
                let label = if default.is_empty() || !opts.contains(&default) { if default.is_empty() { "First option".to_string() } else { default.clone() } } else { default.clone() };
                let textarea = Textarea::new(&c.options)
                    .font(MONO_XS)
                    .min_height(80.0)
                    .label("Options")
                    .on_edit(|s: &mut MainUi| {
                        ensure(s);
                        &mut s.editor.tags.chip.get_or_insert_with(unreachable_chip).options
                    })
                    .on_change(|s: &mut MainUi, _| {
                        let v = s.editor.tags.chip.as_ref().map(|c| c.options.text.clone()).unwrap_or_default();
                        let lines: Vec<&str> = v.split('\n').collect();
                        set(s, vec![("options", Some(AttributeValue::String(join_options(&lines))))], None);
                    })
                    .build(th);
                col().w_full().gap(12.0)
                    .child(field(th, "Label", input(th, &c.label, |c| &mut c.label, |s, v| set(s, vec![("name", Some(AttributeValue::String(v)))], None), false, "", "Label"), None))
                    .child(field(th, "Options", textarea, Some(hint(th, "One per line."))))
                    .child(field(th, "Default", select(th, "chip-default", label, false, move |s: &MainUi| {
                        let c = s.editor.tags.chip.as_ref();
                        let opts: Vec<String> = c.map(|c| split_options(&c.str("options").unwrap_or_default())).unwrap_or_default().into_iter().filter(|o| !o.is_empty()).collect();
                        let cur = c.and_then(|c| c.str("default")).unwrap_or_default();
                        let mut items = vec![("".to_string(), "First option".to_string())];
                        items.extend(opts.into_iter().map(|o| (o.clone(), o)));
                        PopupMenu::picker(items.iter().map(|(a, b)| (a.as_str(), b.as_str())), cur.as_str())
                    }, |s, v| set(s, vec![("default", nonempty(v))], None)), None))
            }
            "snippet" => {
                let me = st.id;
                let current = c.str("id");
                let items = s.library.snippets.iter().filter(|x| Some(x.id) != me).map(|x| {
                    let id = x.id;
                    let on = current.as_deref() == Some(id.to_string().as_str());
                    let mut item = CommandItem::new(format!("{} {} {}", x.name, x.triggers.join(" "), x.id))
                        .child(icon(&icons::FILE_TEXT, 14.0).color(t.muted_foreground))
                        .text(if x.name.is_empty() { "Untitled".to_string() } else { x.name.clone() });
                    if on {
                        item = item.child(row().key("trailing").grow(1.0).justify_end().child(el().size(6.0, 6.0).radius(radius::FULL).fill(t.iris).label("selected")));
                    }
                    item.on_select(move |s: &mut MainUi, _| set(s, vec![("id", Some(AttributeValue::String(id.to_string())))], None))
                });
                el().mx(-4.0).my(-4.0).child(
                    Command::new("chip-snippets", &c.cmd, |s: &mut MainUi| &mut s.editor.tags.chip.get_or_insert_with(unreachable_chip).cmd)
                        .placeholder("Find a snippet…")
                        .max_height(224.0)
                        .empty(text("No snippets.").font(ty::SM).color(t.foreground))
                        .group(None, items)
                        .build(cx),
                )
            }
            _ => el(),
        }
    };
    let trash = Button::icon_only(&icons::TRASH, "Remove from snippet")
        .variant(Variant::Ghost)
        .size(Size::IconXs)
        .color(t.muted_foreground)
        .hover_color(t.destructive)
        .id("chip-remove")
        .on_press(remove)
        .build(th);
    // `autoFocus`: the popover moves focus to its first control as it opens; the form's first
    // field takes it back once laid out.
    let autofocus = el().key(format!("chip-autofocus-{}-{}", c.from, c.name)).absolute(0.0, 0.0).size(0.0, 0.0).on_layout_tx(|s: &mut MainUi, _, _| {
        if let Some(c) = s.editor.tags.chip.as_mut() {
            match c.name.as_str() {
                "field" | "choice" if !c.tag.verbatim => c.label.focused = true,
                "snippet" if !c.tag.verbatim => c.cmd.search.focused = true,
                _ => {}
            }
        }
    });
    let content = col()
        .key("chip-form")
        .child(autofocus)
        .w_full()
        .min_h(0.0)
        .shrink()
        .on_key(|s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx| done_key(s, ev, tx))
        .child(row().w_full().min_h(45.0).center_y().gap(8.0).px(12.0).py(10.0).border_sides([0.0, 0.0, 1.0, 0.0], t.border).border_layout()
            .child(text(title).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground))
            .child(rune::spacer())
            .child(trash))
        .child(
            // Scrolls only when the window can't fit the form (the web let it run off-screen).
            col().key("chip-form-scroll").w_full().min_h(0.0).shrink().scroll(rune::ScrollSpec { stick_bottom: false })
                .child(el().w_full().padding(12.0).child(body.w_full().min_w(0.0))),
        )
        .child(footer(th, vec![
            text("Changes apply instantly").font(ty::XS2).color(t.muted_foreground),
            rune::spacer(),
            row().gap(4.0).center_y().child(Shortcut::new("enter").build(th)).child(text("done").font(ty::XS2).color(t.muted_foreground)),
        ]));
    Popover::new("kz-chip")
        .open(true)
        .on_open_change(|s: &mut MainUi, o, _| if !o { close_chip(s) })
        .trigger(el().w_full().h_full())
        .side(Side::Bottom)
        .align(Align::Start)
        .side_offset(8.0)
        .width(320.0)
        .padding(0.0)
        .gap(0.0)
        .ring(false)
        .backdrop_blur(24.0)
        .label(title)
        .child(content)
        .build(cx, layer)
}

/// tag-tools' own `Segmented`: options share the width.
fn kind_segmented(th: &Theme, cx: &Cx<MainUi>, value: &str) -> El {
    let t = &th.t;
    let selected = format!("chip-kind-{value}");
    let mut r = row()
        .key("chip-kind")
        .w_full()
        .role("radiogroup")
        .label("Kind")
        .padding(2.0)
        .radius(radius::LG)
        .fill(Theme::tint(t.foreground, 0.04))
        .elevate(&t.elev_well)
        .focus_group()
        .child(SelectionPill::new("chip-kind", Some(selected)).radius(radius::MD).fill(t.surface_raised).shadows(&t.elev_soft).build(cx));
    for (v, label) in [("date", "Date"), ("time", "Time")] {
        let on = v == value;
        r = r.child(
            crate::ui::kit::button::hover_fast(row())
                .key(format!("chip-kind-{v}"))
                .grow(1.0)
                .h(28.0)
                .px(8.0)
                .center()
                .radius(radius::MD)
                .role("radio")
                .label(label)
                .checked(on)
                .focus_selected(on)
                .press_sink(0.97)
                .on_press(move |s: &mut MainUi| set(s, vec![], Some(v)))
                .child(text(label).font(ty::XS.weight_of(weight::MEDIUM)).color(if on { t.foreground } else { t.muted_foreground }).a11y_hidden()),
        );
    }
    r
}

fn format_now(pattern: &str, locale: Option<&str>) -> String {
    let now = chrono::Local::now().fixed_offset();
    let loc = locale.map(|l| l.replace('_', "-")).unwrap_or_else(kudzu_engine::system::locale);
    kudzu_core::template::format_icu(&now, pattern, &loc)
}

fn date_form(th: &Theme, c: &ChipOpen, cx: &mut Cx<MainUi>) -> El {
    let t = &th.t;
    let format = c.str("format").unwrap_or_else(|| if c.name == "date" { "yyyy-MM-dd".into() } else { "HH:mm".into() });
    let offset = c.str("offset").unwrap_or_default();
    let locale = c.str("locale");
    let example = format_now(&format, locale.as_deref());
    let preset_label = DATE_FORMAT_PRESETS.iter().find(|(f, _)| *f == format).map(|(f, l)| format!("{l} — {}", format_now(f, None)));
    let select_label = if c.custom { "Custom pattern…".to_string() } else { preset_label.unwrap_or_else(|| format.clone()) };
    let mut fmt = col().gap(6.0).w_full().child(select(th, "chip-format", select_label, true, |s: &MainUi| {
        let c = s.editor.tags.chip.as_ref();
        let cur = match c {
            Some(c) if c.custom => "__custom".to_string(),
            Some(c) => c.str("format").unwrap_or_default(),
            None => String::new(),
        };
        let mut items: Vec<(String, String)> = DATE_FORMAT_PRESETS.iter().map(|(f, l)| (f.to_string(), format!("{l} — {}", format_now(f, None)))).collect();
        items.push(("__custom".into(), "Custom pattern…".into()));
        PopupMenu::picker(items.iter().map(|(a, b)| (a.as_str(), b.as_str())), cur.as_str())
    }, |s, v| {
        if v == "__custom" {
            if let Some(c) = s.editor.tags.chip.as_mut() {
                c.custom = true;
                c.format.focused = true;
            }
        } else {
            if let Some(c) = s.editor.tags.chip.as_mut() {
                c.custom = false;
                c.format.replace_text(v.clone());
            }
            set(s, vec![("format", Some(AttributeValue::String(v)))], None);
        }
    }));
    if c.custom {
        fmt = fmt.child(input(th, &c.format, |c| &mut c.format, |s, v| set(s, vec![("format", Some(AttributeValue::String(v)))], None), true, "EEEE, MMM d · h:mm a", "Format pattern"));
    }
    let mut presets = row().w_full().flex_wrap().gap(4.0);
    for (v, label) in OFFSET_PRESETS {
        let on = offset == v;
        let b = row()
            .key(format!("chip-offset-{label}"))
            .h(24.0)
            .px(8.0)
            .center_y()
            .radius(radius::MD)
            .role("button")
            .label(label)
            .checked(on)
            .press_sink(0.97)
            .on_press(move |s: &mut MainUi| {
                if let Some(c) = s.editor.tags.chip.as_mut() {
                    c.offset.replace_text(v.to_string());
                }
                set(s, vec![("offset", nonempty(v.to_string()))], None);
            });
        let b = if on {
            b.fill(t.iris_soft).elevate(&t.elev_pressed).child(text(label).font(ty::XS).color(t.iris_foreground))
        } else {
            crate::ui::kit::button::hover_fast(b.fill(t.surface_raised).elevate(&t.elev_raised))
                .child(text(label).font(ty::XS).color(t.muted_foreground).group_color(t.foreground))
        };
        presets = presets.child(b);
    }
    let shift_hint = {
        let a = "Combine units, e.g. ";
        let s = format!("{a}+1w2d or -30min.");
        let i1 = a.len();
        let i2 = s.find("-30min").unwrap_or(0);
        rune::rich(s, vec![rune::Span::new(i1, i1 + 5).mono(), rune::Span::new(i2, i2 + 6).mono()]).font(ty::XS2).color(t.faint).wrap()
    };
    col().w_full().gap(12.0)
        .child(
            col().w_full().items_center().radius(radius::LG).fill(Theme::tint(t.foreground, 0.04)).px(12.0).py(10.0)
                .child(text("EXAMPLE").font(ty::XS2.tracking(0.275)).color(t.faint))
                .child(text(if example.is_empty() { "—".to_string() } else { example }).font(ty::SM.weight_of(weight::MEDIUM).features("'tnum' 1")).color(t.foreground).truncate().selectable(true)),
        )
        .child(kind_segmented(th, cx, &c.name))
        .child(field(th, "Format", fmt, None))
        .child(field(th, "Shift", col().gap(6.0).w_full().child(presets).child(
            input(th, &c.offset, |c| &mut c.offset, |s, v| set(s, vec![("offset", nonempty(v))], None), true, "+3d", "Custom shift"),
        ), Some(shift_hint)))
        .child(field(th, "Language", input(th, &c.locale, |c| &mut c.locale, |s, v| set(s, vec![("locale", nonempty(v))], None), true, "System default (e.g. en_US, fr_FR)", "Language"), None))
}
