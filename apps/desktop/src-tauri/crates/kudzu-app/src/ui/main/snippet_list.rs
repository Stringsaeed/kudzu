//! The snippet list (`components/app/main/snippet-list.tsx`): the view's title and count, sort
//! and new-snippet buttons, the filter field (⌘F), and a virtualised list of 64 pt rows with
//! multi-select, keyboard navigation, type-to-filter, dragging rows onto groups, a bulk toolbar
//! and empty states.
//!
//! The list owns its keyboard (`role="listbox"`, focusable): ↑↓ or J/K move, ⇧ extends, Home,
//! End, Page Up/Down, Return or → open the editor, ⌘A selects all, Escape clears a
//! multi-selection, `/` or any printable character goes to the filter. `focus_list` (⌘L, Escape
//! in the editor, after a delete) hands the window's keys to it even before a click focuses it.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use rune::{col, el, icon, rich, row, text, text_input, Align, Color, Cx, El, GrowthSpec, Key, KeyEvent, ScrollSpec, Side, Span, Spring, TextEdit, Tx};
use uuid::Uuid;

use super::library::drag::{self, Payload};
use super::library::{self as lib, rise, STAR_FILLED};
use super::{MainUi, View};
use crate::ui::icons;
use crate::ui::kit::button::{hover_fast, layer, tween, Button, Size, Variant, TW_EASE};
use crate::ui::kit::dropdown_menu::{DropdownMenu, MenuItem, MenuState};
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;
use kudzu_core::client::groups::{build_tree, flatten, is_group_enabled};
use kudzu_core::client::search::{list_hits, new_snippet_seed, ListSort, ListView, NewSnippetSeed, SearchCache};
use kudzu_core::client::stats::format_count;
use kudzu_core::client::template::{plain_summary, SUMMARY_MAX};

/// `ROW_HEIGHT`: a 60 pt item centred in 64.
pub const ROW_HEIGHT: f32 = 64.0;

/// What the last view computed, for the virtual rows' builders and the handlers.
#[derive(Default)]
struct Memo {
    cache: SearchCache,
    ids: Vec<Uuid>,
    name_hits: HashMap<Uuid, Vec<usize>>,
    /// Row summaries by content (`summaryCache`), cleared when any name changes.
    summaries: HashMap<String, String>,
    names_stamp: u64,
}

#[derive(Default)]
pub struct ListState {
    pub filter: TextEdit,
    pub sort: ListSort,
    /// The multi-selection and the view it was made in (a new view clears it).
    multi: Vec<Uuid>,
    multi_view: Option<View>,
    anchor: Option<Uuid>,
    /// Whether the list has the keyboard (its rows draw the selection's active look).
    pub focused: bool,
    pub sort_menu: MenuState,
    pub move_menu: MenuState,
    /// The pointer is on the New snippet button (its plus turns).
    pub plus_hover: bool,
    memo: RefCell<Memo>,
}

impl ListState {
    pub fn clear_filter(&mut self) {
        self.filter.clear();
    }

    /// Hands the keyboard to the list (`listRef.current.focus()`).
    pub fn focus_list(&mut self, tx: &mut Tx) {
        self.filter.focused = false;
        tx.blur();
        self.focused = true;
    }

    pub fn focus_filter(&mut self, _tx: &mut Tx) {
        self.filter.focused = true;
        self.focused = false;
    }

    /// The selected rows (multi-select), or the selected snippet.
    pub fn selection(&self, s: &MainUi) -> Vec<Uuid> {
        let multi = self.multi_in(&s.view);
        if multi.len() > 1 { multi.to_vec() } else { s.selected.into_iter().collect() }
    }

    fn multi_in(&self, view: &View) -> &[Uuid] {
        if self.multi_view.as_ref() == Some(view) { &self.multi } else { &[] }
    }

    fn set_multi(&mut self, view: &View, ids: Vec<Uuid>) {
        self.multi = ids;
        self.multi_view = Some(view.clone());
    }

    fn ids(&self) -> Vec<Uuid> {
        self.memo.borrow().ids.clone()
    }
}

fn list_view(v: &View) -> ListView {
    match v {
        View::Favorites => ListView::Favorites,
        View::Recent => ListView::Recent,
        View::Disabled => ListView::Disabled,
        View::Group(id) => ListView::Group(*id),
        _ => ListView::All,
    }
}

/// `viewTitle`.
fn view_title(s: &MainUi) -> String {
    match &s.view {
        View::All => "All snippets".into(),
        View::Favorites => "Favorites".into(),
        View::Recent => "Recently used".into(),
        View::Disabled => "Turned off".into(),
        View::Group(id) => s.group(*id).map_or("Group".into(), |g| g.name.clone()),
        _ => String::new(),
    }
}

fn row_key(id: Uuid) -> String {
    format!("snippet-{id}")
}

// MARK: Behaviour

fn select(s: &mut MainUi, id: Uuid, tx: &mut Tx) {
    s.selected = Some(id);
    tx.scroll_into_view(row_key(id));
}

/// `move(delta, extend)`.
fn move_by(s: &mut MainUi, delta: isize, extend: bool, tx: &mut Tx) {
    let ids = s.list.ids();
    if ids.is_empty() {
        return;
    }
    let n = ids.len() as isize;
    let at = s.selected.and_then(|sel| ids.iter().position(|x| *x == sel));
    let from = match at {
        Some(i) => i as isize,
        None if delta > 0 => -1,
        None => n,
    };
    let i = (from + delta).clamp(0, n - 1) as usize;
    let id = ids[i];
    let prev = s.selected;
    select(s, id, tx);
    let view = s.view.clone();
    if extend {
        let a = s.list.anchor.or(prev).unwrap_or(id);
        s.list.anchor = Some(a);
        let ai = ids.iter().position(|x| *x == a).unwrap_or(i);
        let (lo, hi) = if ai < i { (ai, i) } else { (i, ai) };
        s.list.set_multi(&view, ids[lo..=hi].to_vec());
    } else {
        s.list.anchor = Some(id);
        s.list.set_multi(&view, Vec::new());
    }
}

/// `onOpen`: the editor takes the keyboard.
fn open(s: &mut MainUi, tx: &mut Tx) {
    s.list.focused = false;
    crate::ui::editor::focus(s, tx);
}

/// The listbox's keys. True when taken.
pub fn on_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    let plain = !m.cmd && !m.ctrl;
    s.list.focused = true;
    match &ev.key {
        Key::Down | Key::Up => move_by(s, if ev.key == Key::Down { 1 } else { -1 }, m.shift, tx),
        Key::Text(c) if plain && !m.alt && (c == "j" || c == "k") => move_by(s, if c == "j" { 1 } else { -1 }, m.shift, tx),
        Key::Home | Key::End => {
            let n = s.list.ids().len() as isize;
            move_by(s, if ev.key == Key::Home { -n } else { n }, m.shift, tx);
        }
        Key::PageDown | Key::PageUp => move_by(s, if ev.key == Key::PageDown { 8 } else { -8 }, m.shift, tx),
        Key::Enter | Key::Right if !m.cmd => open(s, tx),
        Key::Command('a') if !m.shift && !m.alt => {
            let view = s.view.clone();
            let ids = s.list.ids();
            s.list.set_multi(&view, ids);
        }
        Key::Escape if !s.list.multi_in(&s.view).is_empty() => {
            let view = s.view.clone();
            s.list.set_multi(&view, Vec::new());
        }
        Key::Text(c) if c == "/" && plain => s.list.focus_filter(tx),
        // Type-to-filter: the character lands in the filter, as the browser delivered it.
        Key::Text(c) if plain && !m.alt && c.chars().count() == 1 && !c.chars().all(char::is_whitespace) => {
            let f = &mut s.list.filter;
            let end = f.text.len();
            f.set_caret(end);
            f.insert(c);
            s.list.focus_filter(tx);
        }
        _ => return false,
    }
    true
}

/// A press on a row (`onMouseDown`): ⌘ toggles it in the selection, ⇧ extends from the anchor,
/// otherwise it's the one selected row.
fn press_row(s: &mut MainUi, id: Uuid, tx: &mut Tx) {
    let mods = tx.mods();
    let view = s.view.clone();
    s.list.focused = true;
    if mods.cmd || mods.ctrl {
        let mut n: Vec<Uuid> = if s.list.multi_in(&view).is_empty() { s.selected.into_iter().collect() } else { s.list.multi_in(&view).to_vec() };
        if let Some(i) = n.iter().position(|x| *x == id) {
            n.remove(i);
        } else {
            n.push(id);
        }
        s.list.set_multi(&view, n);
        s.selected = Some(id);
    } else if mods.shift && s.list.anchor.is_some() {
        let ids = s.list.ids();
        let a = s.list.anchor.and_then(|a| ids.iter().position(|x| *x == a));
        let b = ids.iter().position(|x| *x == id);
        if let (Some(a), Some(b)) = (a, b) {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            s.list.set_multi(&view, ids[lo..=hi].to_vec());
        }
        s.selected = Some(id);
    } else {
        s.list.anchor = Some(id);
        s.list.set_multi(&view, Vec::new());
        s.selected = Some(id);
    }
}

/// What dragging row `id` carries: the multi-selection if it's in it, else the row.
fn drag_payload(s: &MainUi, id: Uuid) -> Option<(Payload, String)> {
    let multi = s.list.multi_in(&s.view);
    let dragged: Vec<Uuid> = if multi.contains(&id) { multi.to_vec() } else { vec![id] };
    let label = if dragged.len() > 1 { format!("{} snippets", dragged.len()) } else { s.snippet(id).map_or("Snippet".into(), |x| x.name.clone()) };
    Some((Payload::Snippets(dragged), label))
}

/// `newSnippet` seeded from an unmatched filter.
fn create_from_filter(s: &mut MainUi) {
    let mut draft = serde_json::Map::new();
    match new_snippet_seed(&s.list.filter.text) {
        NewSnippetSeed::Trigger(t) => draft.insert("triggers".into(), serde_json::json!([t])),
        NewSnippetSeed::Name(n) => draft.insert("name".into(), serde_json::json!(n)),
    };
    s.new_snippet(draft);
}

// MARK: Rows

/// `Highlighted`: matched characters (UTF-16 offsets into the name) in iris, underlined.
fn highlighted(th: &Theme, name: &str, hits: &[usize], font: rune::TextStyle, color: Color) -> El {
    if hits.is_empty() {
        return text(name.to_string()).font(font).color(color);
    }
    let mut spans: Vec<Span> = Vec::new();
    let mut u16_at = 0usize;
    for (byte, ch) in name.char_indices() {
        if hits.binary_search(&u16_at).is_ok() {
            let end = byte + ch.len_utf8();
            match spans.last_mut() {
                Some(last) if last.end as usize == byte => last.end = end as u32,
                _ => spans.push(Span::new(byte, end).color(th.t.iris_foreground).underline(Theme::tint(th.t.iris, 0.5))),
            }
        }
        u16_at += ch.len_utf16();
    }
    rich(name.to_string(), spans).font(font).color(color)
}

struct RowLook {
    selected: bool,
    in_selection: bool,
    active: bool,
}

#[allow(clippy::too_many_arguments)]
fn snippet_row(th: &Theme, s: &MainUi, id: Uuid, look: RowLook, summary: String, hits: &[usize], first: bool, last: bool, trigger_w: f32) -> El {
    let t = &th.t;
    let Some(x) = s.snippet(id) else { return el().key(row_key(id)).h(ROW_HEIGHT) };
    let group_off = x.group_id.is_some_and(|g| !is_group_enabled(&s.library.groups, Some(g)));
    let off = !x.is_enabled || group_off;
    let usage = s.usage.get(&id).map_or(0, |u| u.count);
    let r = radius::LG;
    let iris = look.selected && look.active;
    let (base, hover) = if iris {
        (t.iris_soft, t.iris_soft)
    } else if look.in_selection {
        (Theme::tint(t.foreground, 0.06), Theme::tint(t.foreground, 0.07))
    } else {
        (Color::TRANSPARENT, Theme::tint(t.foreground, 0.035))
    };
    let colors = Spring::new(0.075, 0.0);

    // Title line.
    let title_font = rune::TextStyle { size: 13.0, line_height: 19.0, ..ty::BASE.weight_of(weight::MEDIUM) };
    let unnamed = x.name.is_empty();
    let title_ink = if off || unnamed { t.muted_foreground } else { t.foreground };
    let title_font = if unnamed { rune::TextStyle { italic: true, ..title_font } } else { title_font };
    let title = highlighted(th, if unnamed { "Untitled" } else { &x.name }, if unnamed { &[] } else { hits }, title_font, title_ink).key("title").truncate().grow(1.0).shrink().min_w(0.0);
    let mut line1 = row().w_full().min_w(0.0).gap(8.0).center_y().child(title);
    if x.is_favorite {
        line1 = line1.child(icon(&STAR_FILLED, 12.0).key("star").color(t.favorite).label("Favorite"));
    }
    if off {
        line1 = line1.child(icon(&icons::CIRCLE_OFF, 12.0).key("off").color(t.faint).label("Turned off"));
    }
    if let Some(first_trigger) = x.triggers.first() {
        let mono = rune::TextStyle::new(11.5, weight::NORMAL).family(rune::Family::Mono).lh(16.0);
        let ink = if off { t.faint } else { Theme::tint(t.iris_foreground, 0.9) };
        let mut label = first_trigger.clone();
        let mut spans = vec![];
        if x.triggers.len() > 1 {
            let start = label.len();
            label.push_str(&format!(" +{}", x.triggers.len() - 1));
            spans.push(Span::new(start, label.len()).color(t.faint));
        }
        // `max-w-[45%] shrink-0 truncate`.
        line1 = line1.child(rich(label, spans).key("trigger").font(mono).color(ink).truncate().max_w(trigger_w));
    }

    // Summary line.
    let desc_font = ty::XS;
    let desc = if summary.is_empty() {
        text("Empty").font(rune::TextStyle { italic: true, ..desc_font }).color(Theme::tint(t.muted_foreground, 0.8))
    } else {
        text(summary).font(desc_font).color(Theme::tint(t.muted_foreground, 0.8))
    };
    let mut line2 = row().w_full().min_w(0.0).gap(8.0).center_y().child(desc.key("summary").truncate().grow(1.0).shrink().min_w(0.0));
    if usage > 0 {
        line2 = line2.child(text(format!("{}×", format_count(usage))).key("usage").font(lib::tabular(rune::TextStyle::new(10.5, weight::NORMAL).lh(14.0))).color(t.faint));
    }

    let mut item = row()
        .key("item")
        .role("option")
        .label(if unnamed { "Untitled".to_string() } else { x.name.clone() })
        .selected_a11y(look.in_selection)
        .w_full()
        .h(60.0)
        .px(13.0)
        .gap(10.0)
        .center_y()
        .radius(r)
        .hover_group()
        .hover_motion(tween(0.075, TW_EASE), tween(0.075, TW_EASE));
    // Every layer is always present and keyed, so a selection change only eases colours and
    // opacity. (Rune matches unkeyed siblings by position: a layer appearing in front of the
    // content re-paired it with other elements and cross-faded the whole row.)
    let shown = |on: bool| if on { 1.0 } else { 0.0 };
    item = item
        // `shadow-selected`: CSS paints box shadows only outside the box, so the shadow goes on
        // an opaque layer the translucent fills then tint.
        .child(layer(0.0).key("shadow").radius(r).fill(t.background).elevate(&t.elev_selected).opacity(shown(look.in_selection)).opacity_motion(colors))
        .child(layer(0.0).key("base").radius(r).fill(base).motion(colors).group_opacity(1.0, 0.0))
        .child(layer(0.0).key("hover").radius(r).fill(hover).motion(colors).group_opacity(0.0, 1.0))
        // `after:ring-1 after:ring-iris/35 after:ring-inset`.
        .child(layer(0.0).key("ring").radius(r).border(1.0, Theme::tint(t.iris, 0.35)).opacity(shown(iris)).opacity_motion(colors))
        .child(col().key("content").grow(1.0).min_w(0.0).gap(4.0).child(line1.key("line1")).child(line2.key("line2")));

    el()
        .key(row_key(id))
        .w_full()
        .pt(2.0 + if first { 4.0 } else { 0.0 })
        .pb(2.0 + if last { 8.0 } else { 0.0 })
        .px(8.0)
        .child(item)
        .on_drag(move |s: &mut MainUi, d, tx| {
            if d.phase == rune::Phase::Down {
                press_row(s, id, tx);
            }
            drag::track(s, d, tx, move |s| drag_payload(s, id));
        })
}

/// `selected` on an option (`aria-selected`).
trait SelectedA11y {
    fn selected_a11y(self, on: bool) -> Self;
}

impl SelectedA11y for El {
    fn selected_a11y(self, on: bool) -> El {
        self.checked(on)
    }
}

// MARK: View

pub fn view(s: &MainUi, cx: &mut Cx<MainUi>, layer_out: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let filter = s.list.filter.text.clone();
    let lv = list_view(&s.view);

    // Search, scope and sort into the memo the rows read.
    let ids: Vec<Uuid> = {
        let mut m = s.list.memo.borrow_mut();
        let stamp = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            for x in &s.library.snippets {
                x.id.hash(&mut h);
                x.name.hash(&mut h);
            }
            h.finish()
        };
        if stamp != m.names_stamp {
            m.summaries.clear();
            m.names_stamp = stamp;
        }
        let now = chrono::Utc::now().timestamp_millis();
        let Memo { cache, .. } = &mut *m;
        let hits = list_hits(&s.library, lv, &filter, s.list.sort, &s.usage, cache, now);
        let ids: Vec<Uuid> = hits.iter().map(|h| h.snippet.id).collect();
        let name_hits: HashMap<Uuid, Vec<usize>> = hits.iter().filter(|h| !h.name_hits.is_empty()).map(|h| (h.snippet.id, h.name_hits.clone())).collect();
        m.ids = ids.clone();
        m.name_hits = name_hits;
        ids
    };
    let selected_index = s.selected.and_then(|sel| ids.iter().position(|x| *x == sel));
    let title = view_title(s);

    // Header: `drag flex h-12 items-center gap-2 pr-2 pl-5`.
    let sort = s.list.sort;
    let sort_button = Button::icon_only(&icons::ARROW_DOWN_UP, "Sort").variant(Variant::Ghost).size(Size::IconSm).color(t.muted_foreground).haspopup().expanded(s.list.sort_menu.open).id("list-sort").build(th);
    let radio = |label: &str, v: ListSort| MenuItem::radio(label, sort == v).on_select(move |s: &mut MainUi, _| s.list.sort = v);
    let sort_menu = DropdownMenu::new("list-sort", &s.list.sort_menu, |s: &mut MainUi| &mut s.list.sort_menu)
        .trigger(sort_button)
        .align(Align::End)
        .width(176.0)
        .label("Sort by")
        .item(radio("Best match", ListSort::Relevance))
        .item(radio("Name", ListSort::Name))
        .item(radio("Recently edited", ListSort::Edited))
        .item(radio("Most used", ListSort::Used))
        .build(cx, layer_out);
    let sort_menu = Tooltip::new("list-sort-tip").side(Side::Bottom).text("Sort").build(cx, sort_menu, layer_out);
    // The plus turns a quarter on hover (`group-hover/button:rotate-90`, slow, ease-pop).
    let plus = lib::ghost_icon(th, "list-new", &icons::PLUS, "New snippet", 28.0, t.muted_foreground, s.list.plus_hover)
        .on_hover_change(|s: &mut MainUi, inside, _| s.list.plus_hover = inside)
        .on_press(|s: &mut MainUi| {
            s.new_snippet(Default::default());
        });
    let plus = Tooltip::new("list-new-tip").side(Side::Bottom).text("New snippet").shortcut("mod+n").build(cx, plus, layer_out);
    let header = row()
        .key("list-header")
        .h(48.0)
        .gap(8.0)
        .pr(8.0)
        .pl(20.0)
        .center_y()
        .window_drag()
        .child(
            rich(
                format!("{title}  {}", ids.len()),
                vec![Span::new(title.len(), title.len() + 2 + ids.len().to_string().len()).weight(weight::NORMAL).color(t.faint)],
            )
            .font(lib::tabular(ty::BASE.weight_of(weight::SEMIBOLD)))
            .color(t.foreground)
            .role("heading")
            .truncate()
            .grow(1.0)
            .shrink()
            .min_w(0.0),
        )
        .child(sort_menu.no_window_drag())
        .child(plus.no_window_drag());

    // The filter: `h-8 rounded-lg bg-foreground/[0.03] pr-14 pl-8 text-[13px] shadow-well`,
    // focused `bg-transparent shadow-none ring-2 ring-ring`.
    let focused = s.list.filter.focused;
    let field = text_input(&s.list.filter, ty::BASE)
        .single_line()
        .key("list-filter-input")
        .label("Filter snippets")
        .placeholder("Filter by name, trigger or text", t.faint)
        .spellcheck(false)
        .color(t.foreground)
        .caret(t.foreground, t.iris_soft)
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .on_edit(|s: &mut MainUi| &mut s.list.filter)
        .on_input_key(|s: &mut MainUi, ev, tx| match ev.key {
            Key::Down | Key::Up => {
                move_by(s, if ev.key == Key::Down { 1 } else { -1 }, ev.mods.shift, tx);
                true
            }
            Key::Enter => {
                open(s, tx);
                true
            }
            Key::Escape => {
                if s.list.filter.text.is_empty() {
                    s.list.focus_list(tx);
                } else {
                    s.list.clear_filter();
                }
                true
            }
            _ => false,
        });
    let trailing = if filter.is_empty() {
        Shortcut::new("mod+f").build(th).inert()
    } else {
        hover_fast(row())
            .key("list-filter-clear")
            .role("button")
            .label("Clear filter")
            .size(20.0, 20.0)
            .center()
            .radius(radius::SM)
            .child(icon(&icons::X, 14.0).color(t.faint).group_color(t.foreground).a11y_hidden())
            .on_press(|s: &mut MainUi| s.list.clear_filter())
    };
    let mut filter_box = row()
        .key("list-filter")
        .role("search")
        .h(32.0)
        .w_full()
        .pl(10.0)
        .pr(8.0)
        .gap(8.0)
        .center_y()
        .radius(radius::LG)
        .motion(Spring::new(0.15, 0.0))
        .child(icon(&icons::SEARCH, 14.0).color(t.faint).a11y_hidden())
        // The input's box clips its placeholder (the shortcut sits in its `pr-14`).
        .child(el().key("list-filter-clip").grow(1.0).min_w(0.0).center_y().clip_rounded(0.0).child(field))
        .child(trailing);
    filter_box = if focused {
        filter_box.fill(Color::TRANSPARENT).outline(2.0, 0.0, t.ring)
    } else {
        filter_box.fill(Theme::tint(t.foreground, 0.03)).elevate(&t.elev_well)
    };

    // The rows.
    let multi = s.list.multi_in(&s.view).to_vec();
    let active = s.list.focused || multi.len() <= 1;
    let selected = s.selected;
    let count = ids.len();
    // `max-w-[45%]` of the title line: the column less the row's `mx-2` and `px-3` (and border).
    let trigger_w = ((cx.layout_size("list-pane").map_or(320.0, |sz| sz[0]) - 1.0 - 16.0 - 26.0) * 0.45).max(0.0);
    let body = if count == 0 {
        empty_list(th, s, &filter)
    } else {
        let rows = col()
            .key("list-rows")
            .w_full()
            .growth(GrowthSpec::default())
            // The web re-renders the list in place: rows a view change, sort or filter moves
            // jump to their new place rather than sliding there (colours still ease).
            .instant_changes()
            .virtual_rows(
                count,
                ROW_HEIGHT,
                |s: &MainUi, i| Cow::Owned(s.list.memo.borrow().ids.get(i).map_or_else(String::new, |id| row_key(*id))),
                move |s: &MainUi, i| {
                    let (id, hits, n) = {
                        let m = s.list.memo.borrow();
                        let Some(id) = m.ids.get(i).copied() else { return el() };
                        (id, m.name_hits.get(&id).cloned().unwrap_or_default(), m.ids.len())
                    };
                    let multi = s.list.multi_in(&s.view);
                    let in_selection = if multi.len() > 1 { multi.contains(&id) } else { Some(id) == s.selected };
                    let look = RowLook { selected: Some(id) == s.selected, in_selection, active: s.list.focused || multi.len() <= 1 };
                    let summary = summary_of(s, id);
                    snippet_row(th, s, id, look, summary, &hits, i == 0, i + 1 == n, trigger_w)
                },
            )
            .virtual_rows_version((ids.iter().fold(0u64, |h, id| h.rotate_left(5) ^ id.as_u128() as u64), multi.len(), selected, active, th.dark, s.library.snippets.len()));
        col()
            .key("snippet-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec { stick_bottom: false })
            // React swaps the list for its empty state in one render: the rows leave at once
            // instead of fading out under the empty state's entrance.
            .transition(rune::Transition::none())
            .child(rows)
    };
    let mut listbox = col()
        .key("snippet-listbox")
        .role("listbox")
        .label(format!("{title} snippets"))
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .focusable(true)
        .no_focus_ring()
        .on_key(on_key)
        .child(body);
    if let (Some(id), Some(_)) = (selected, selected_index) {
        listbox = listbox.active_descendant(row_key(id));
    }

    // Keep a valid selection inside the visible list.
    if selected_index.is_none() && !ids.is_empty() && (selected.is_none() || filter.is_empty()) {
        let keep = selected.is_some_and(|id| s.snippet(id).is_some()) && filter.is_empty() && s.view == View::All;
        if !keep {
            let first = ids[0];
            listbox = listbox.child(lib::on_mount(format!("list-keep-{first}"), move |s: &mut MainUi, _| s.selected = Some(first)));
        }
    }
    // Reveal the selected row when it changes from elsewhere (navigate, new snippet, undo).
    if let (Some(id), Some(_)) = (selected, selected_index) {
        listbox = listbox.child(lib::on_mount(format!("list-reveal-{id}"), move |_: &mut MainUi, tx| tx.scroll_into_view(row_key(id))));
    }

    let mut root = col()
        .key("snippet-list")
        .role("region")
        .label("Snippets")
        .w_full()
        .h_full()
        .min_h(0.0)
        .motion(Spring::NONE)
        .child(header)
        .child(el().w_full().px(12.0).pb(8.0).child(filter_box))
        .child(listbox);
    if multi.len() > 1 {
        root = root.child(bulk_bar(th, s, cx, &multi, layer_out));
    }
    root
}

/// `summaryOf`: the row's one-line summary, cached by content.
fn summary_of(s: &MainUi, id: Uuid) -> String {
    let Some(x) = s.snippet(id) else { return String::new() };
    if let Some(v) = s.list.memo.borrow().summaries.get(&x.content) {
        return v.clone();
    }
    let names: HashMap<String, String> = s.library.snippets.iter().map(|x| (x.id.to_string(), x.name.clone())).collect();
    let lookup = |id: &str| names.get(id).cloned();
    let v = plain_summary(&x.content, Some(&lookup), SUMMARY_MAX);
    s.list.memo.borrow_mut().summaries.insert(x.content.clone(), v.clone());
    v
}

/// The selection toolbar: `m-2 rounded-xl bg-surface-raised p-1.5 pl-3 shadow-float animate-rise`.
fn bulk_bar(th: &'static Theme, s: &MainUi, cx: &mut Cx<MainUi>, bulk: &[Uuid], layer_out: &mut Vec<El>) -> El {
    let t = &th.t;
    let ids = bulk.to_vec();
    let fav_ids = ids.clone();
    let fav = Button::icon_only(&icons::STAR, "Favorite all")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .id("bulk-fav")
        .on_press(move |s: &mut MainUi| {
            for id in &fav_ids {
                if let Some(mut x) = s.snippet(*id).cloned()
                    && !x.is_favorite
                {
                    x.is_favorite = true;
                    s.save_snippet(x);
                }
            }
        })
        .build(th);
    let fav = Tooltip::new("bulk-fav-tip").text("Add to favorites").build(cx, fav, layer_out);
    let off_ids = ids.clone();
    let off = Button::icon_only(&icons::CIRCLE_OFF, "Turn off all")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .id("bulk-off")
        .on_press(move |s: &mut MainUi| {
            for id in &off_ids {
                if let Some(mut x) = s.snippet(*id).cloned() {
                    x.is_enabled = !x.is_enabled;
                    s.save_snippet(x);
                }
            }
        })
        .build(th);
    let off = Tooltip::new("bulk-off-tip").text("Toggle on/off").build(cx, off, layer_out);

    let move_button = Button::icon_only(&icons::FOLDER_INPUT, "Move to group").variant(Variant::Ghost).size(Size::IconSm).haspopup().expanded(s.list.move_menu.open).id("bulk-move").build(th);
    let none_ids = ids.clone();
    let mut menu = DropdownMenu::new("bulk-move", &s.list.move_menu, |s: &mut MainUi| &mut s.list.move_menu)
        .trigger(move_button)
        .align(Align::End)
        .side(Side::Top)
        .width(208.0)
        .label(format!("Move {} to", ids.len()))
        .item(MenuItem::new("No group").on_select(move |s: &mut MainUi, _| s.move_snippets(&none_ids, None)));
    let tree = build_tree(&s.library.groups);
    for n in flatten(&tree, None) {
        let (gid, ids) = (n.group.id, ids.clone());
        menu = menu.item(MenuItem::new(n.group.name.clone()).pad_left(8.0 + n.depth as f32 * 14.0).on_select(move |s: &mut MainUi, _| s.move_snippets(&ids, Some(gid))));
    }
    let menu = menu.build(cx, layer_out);
    let del_ids = ids.clone();
    let delete = Button::icon_only(&icons::TRASH, "Delete selected")
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .hover_color(t.destructive)
        .id("bulk-delete")
        .on_press_tx(move |s: &mut MainUi, tx| {
            s.request_delete(del_ids.clone(), tx);
            let view = s.view.clone();
            s.list.set_multi(&view, Vec::new());
        })
        .build(th);

    row()
        .key("bulk-bar")
        .role("toolbar")
        .label("Selection actions")
        .mx(8.0)
        .my(8.0)
        .gap(4.0)
        .padding(6.0)
        .pl(12.0)
        .center_y()
        .radius(radius::XL)
        .fill(t.surface_raised)
        .elevate(&t.elev_float)
        .transition(rise())
        .child(text(format!("{} selected", ids.len())).font(lib::tabular(ty::XS.weight_of(weight::MEDIUM))).color(t.foreground).grow(1.0))
        .child(fav)
        .child(off)
        .child(menu)
        .child(delete)
}

/// `EmptyList`.
fn empty_list(th: &'static Theme, s: &MainUi, filter: &str) -> El {
    let t = &th.t;
    if !filter.is_empty() {
        return col()
            .key("list-empty-filter")
            .w_full()
            .gap(12.0)
            .px(24.0)
            .pt(64.0)
            .items_center()
            .transition(rise())
            .child(rich(format!("No snippets match “{filter}”"), vec![Span::new("No snippets match ".len(), "No snippets match ".len() + format!("“{filter}”").len()).color(t.foreground)]).font(ty::SM).color(t.muted_foreground).wrap())
            .child(
                row()
                    .gap(8.0)
                    .child(Button::new("Clear filter").size(Size::Sm).variant(Variant::Secondary).on_press(|s: &mut MainUi| s.list.clear_filter()).build(th))
                    .child(Button::new("Create it").size(Size::Sm).icon_start(&icons::PLUS).on_press(create_from_filter).build(th)),
            );
    }
    let (title, body) = match s.view {
        View::Favorites => ("No favorites yet", "Star the snippets you use most to find them faster — they also rank first in search."),
        View::Recent => ("Nothing used yet", "Snippets you expand will show up here."),
        View::Disabled => ("Everything is on", "Snippets you turn off appear here."),
        View::Group(_) => ("This group is empty", "Create a snippet here or drag snippets onto the group."),
        _ => ("Your library is empty", "Snippets turn a short trigger like ;sig into text you type often."),
    };
    let mut e = col()
        .key("list-empty")
        .w_full()
        .gap(12.0)
        .px(32.0)
        .pt(80.0)
        .items_center()
        .transition(rise())
        .child(
            col()
                .gap(4.0)
                .items_center()
                .child(text(title).font(ty::SM.weight_of(weight::MEDIUM)).color(t.foreground))
                .child(text(body).font(ty::XS.lh(19.5)).color(t.muted_foreground).wrap().align(Align::Center)),
        );
    if matches!(s.view, View::All | View::Group(_) | View::Favorites) {
        e = e.child(Button::new("New snippet").size(Size::Sm).icon_start(&icons::PLUS).on_press(create_from_filter).build(th));
    }
    e
}
