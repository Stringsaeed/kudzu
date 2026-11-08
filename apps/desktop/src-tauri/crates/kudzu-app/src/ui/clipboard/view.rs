//! The main window's Clipboard pages (`clipboard-view.tsx`): the history list (search with
//! `app:` and `is:`, day sections, pinned first, multi-selection) beside a preview of the active
//! item with its actions. The kind filter comes from the sidebar's Clipboard section.
//!
//! Keys (as `useHotkeys` and the list's `onKeyDown`): ↵ copy, ⇧↵ the other format, Space a large
//! preview, ⌘P pin, ⌘S save as snippet, ⌘O open, ⌘R show in Finder, ⌘⌫ delete with Undo,
//! ⌘F search, ⌘L the list, arrows / j k / Home End / Page Up Down to move (⇧ extends), ⌘A select
//! all, ⌘C copy, Escape clears a multi-selection, / or typing searches.
//!
//! Rune can't focus a plain element from code, so the list's keyboard lives in an invisible
//! one-point text field inside it (`list_focus`): ⌘L and a press on a row focus it, and its key
//! handler is the listbox's `onKeyDown`.

use std::cell::{Ref, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use kudzu_core::client::clipboard::{self as clip, describe, has_text, is_secret};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{col, el, icon, row, text, text_input, Align, Color, Cx, Effect, El, Key, KeyEvent, LiveRegion, Mods, Shadow, Spring, SpringPair, TextEdit, Transition, Tx, Vec2};
use uuid::Uuid;

use super::list::{row_key, ClipList, Density, Look, Row, Rows};
use super::{core_filter, filter_label, preview, secret, ClipHost};
use crate::ui::icons;
use crate::ui::kit::breadcrumbs::{Breadcrumbs, Entry as Crumb};
use crate::ui::kit::button::{Button, Size, Variant};
use crate::ui::kit::checkbox::Checkbox;
use crate::ui::kit::dialog::{self, Dialog, Placement};
use crate::ui::kit::dropdown_menu::{DropdownMenu, MenuItem, MenuState, SubMenu};
use crate::ui::kit::shortcut::{accelerator_labels, Shortcut, Tone};
use crate::ui::kit::toast::Toast;
use crate::ui::kit::tooltip::Tooltip;
use crate::ui::main::{ClipFilter, MainUi, SettingsSection, View};
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

/// The page's own state (the component's `useState`s).
pub struct ClipViewState {
    pub search: TextEdit,
    /// The list's keyboard (see the module docs).
    pub list_focus: TextEdit,
    pub active: Option<Uuid>,
    /// Where the active row last was, so a removed row hands the selection to its neighbour.
    pub last_index: usize,
    pub multi: Rc<HashSet<Uuid>>,
    pub anchor: Option<Uuid>,
    pub quick_look: bool,
    pub clear_open: bool,
    pub clear_pinned: bool,
    pub menu: MenuState,
    /// The filter the multi-selection belongs to (a new filter starts a fresh one).
    pub shown_filter: ClipFilter,
    pub announce: String,
    pub rows: RefCell<Rows>,
}

impl Default for ClipViewState {
    fn default() -> Self {
        ClipViewState {
            search: TextEdit::new(""),
            list_focus: TextEdit::new(""),
            active: None,
            last_index: 0,
            multi: Rc::new(HashSet::new()),
            anchor: None,
            quick_look: false,
            clear_open: false,
            clear_pinned: false,
            menu: MenuState::default(),
            shown_filter: ClipFilter::All,
            announce: String::new(),
            rows: RefCell::new(Rows::default()),
        }
    }
}

const LIST: &str = "clip-list";

fn filter(s: &MainUi) -> ClipFilter {
    match s.view {
        View::Clipboard(f) => f,
        _ => ClipFilter::All,
    }
}

/// The rows for the current history, filter and query (`searchClips` + `buildRows`).
fn rows(s: &MainUi) -> Ref<'_, Rows> {
    let f = filter(s);
    let query = s.clip_view.search.text.clone();
    let searching = !query.trim().is_empty();
    super::list::cached(&s.clip_view.rows, &s.clip.entries, s.clip.rev, core_filter(f), &query, !searching && f != ClipFilter::Pinned, true)
}

/// The active entry: the chosen one, else the neighbour of one that disappeared, else the first.
fn current(s: &MainUi) -> Option<(usize, Uuid)> {
    let r = rows(s);
    if let Some((a, &i)) = s.clip_view.active.and_then(|a| r.pos.get(&a).map(|i| (a, i))) {
        return Some((i, a));
    }
    if r.order.is_empty() {
        return None;
    }
    let i = s.clip_view.last_index.min(r.order.len() - 1);
    r.id_at(&s.clip.entries, i).map(|id| (i, id))
}

fn active_entry(s: &MainUi) -> Option<ClipEntry> {
    current(s).and_then(|(_, id)| s.clip.get(id).cloned())
}

fn order_ids(s: &MainUi) -> Vec<Uuid> {
    let r = rows(s);
    r.order.iter().map(|&i| s.clip.entries[i].id).collect()
}

/// The multi-selection, empty once the filter changed under it.
fn multi(s: &MainUi) -> Rc<HashSet<Uuid>> {
    if s.clip_view.shown_filter == filter(s) { s.clip_view.multi.clone() } else { Rc::new(HashSet::new()) }
}

fn sync_filter(s: &mut MainUi) {
    let f = filter(s);
    if s.clip_view.shown_filter != f {
        s.clip_view.shown_filter = f;
        s.clip_view.multi = Rc::new(HashSet::new());
    }
}

fn set_multi(s: &mut MainUi, set: HashSet<Uuid>) {
    s.clip_view.multi = Rc::new(set);
}

/// `selection()`: the multi-selection, else the active item.
fn selection(s: &MainUi) -> Vec<ClipEntry> {
    let m = multi(s);
    if m.len() > 1 {
        let r = rows(s);
        return r.order.iter().map(|&i| &s.clip.entries[i]).filter(|e| m.contains(&e.id)).cloned().collect();
    }
    active_entry(s).into_iter().collect()
}

pub fn focus_search(s: &mut MainUi, _tx: &mut Tx) {
    s.clip_view.search.focused = true;
}

pub fn focus_list(s: &mut MainUi, _tx: &mut Tx) {
    s.clip_view.list_focus.focused = true;
}

/// Shows the active row (and its section's header when it's a section's first row).
fn reveal(s: &MainUi, id: Uuid, tx: &mut Tx) {
    let r = rows(s);
    let Some(i) = r.row_of(&s.clip.entries, id) else { return };
    let key = match i.checked_sub(1).and_then(|p| r.rows.get(p)) {
        Some(Row::Header { key, .. }) => format!("{LIST}-{key}"),
        _ => row_key(LIST, &s.clip.entries[match &r.rows[i] {
            Row::Entry { entry, .. } => *entry,
            Row::Header { .. } => return,
        }]),
    };
    tx.scroll_into_view(key);
}

fn set_active(s: &mut MainUi, id: Uuid, index: usize, tx: &mut Tx) {
    s.clip_view.active = Some(id);
    s.clip_view.last_index = index;
    reveal(s, id, tx);
}

/// `move`: by `delta` rows, extending the multi-selection from the anchor with Shift.
fn step(s: &mut MainUi, delta: i64, extend: bool, tx: &mut Tx) {
    sync_filter(s);
    let ids = order_ids(s);
    if ids.is_empty() {
        return;
    }
    let from = current(s).map(|(i, _)| i as i64).unwrap_or(if delta > 0 { -1 } else { ids.len() as i64 });
    let n = (from + delta).clamp(0, ids.len() as i64 - 1) as usize;
    let id = ids[n];
    if extend {
        let a = s.clip_view.anchor.or(s.clip_view.active).unwrap_or(id);
        s.clip_view.anchor = Some(a);
        let ai = ids.iter().position(|x| *x == a).unwrap_or(n);
        let (lo, hi) = if ai < n { (ai, n) } else { (n, ai) };
        set_multi(s, ids[lo..=hi].iter().copied().collect());
    } else {
        s.clip_view.anchor = Some(id);
        set_multi(s, HashSet::new());
    }
    set_active(s, id, n, tx);
}

fn say(s: &mut MainUi, text: impl Into<String>) {
    s.clip_view.announce = text.into();
}

/// `copy`: puts it back on the clipboard; formatted text goes as plain text when the setting
/// says so (⇧ flips it).
fn copy(s: &mut MainUi, e: Option<ClipEntry>, flip_plain: bool) {
    let Some(e) = e else { return };
    let plain = e.rich && s.settings.clipboard_paste_plain != flip_plain;
    match crate::backend::clipboard::clipboard_copy(e.id, plain) {
        Ok(()) => {
            s.toasts.push(Toast::success(if plain { "Copied as plain text" } else { "Copied" }).duration_ms(1600));
            say(s, "Copied to clipboard");
        }
        Err(err) => s.toasts.push(Toast::error("Couldn't copy").description(err)),
    }
}

/// `remove`: deletes with an Undo toast, then hands the keyboard back to the list.
fn remove(s: &mut MainUi, list: Vec<ClipEntry>, _tx: &mut Tx) {
    if list.is_empty() {
        return;
    }
    set_multi(s, HashSet::new());
    let removed = s.clip.remove(&list.iter().map(|e| e.id).collect::<Vec<_>>());
    let show = s.settings.clipboard_show_secrets;
    let label = if removed.len() == 1 {
        let e = &removed[0];
        if is_secret(e) && !show { "Deleted a secret".to_string() } else { format!("Deleted “{}”", super::list::js_slice(&describe(e).title, 40)) }
    } else {
        format!("Deleted {} items", removed.len())
    };
    say(s, label.clone());
    let back = removed.clone();
    s.toasts.push(Toast::new(label).duration_ms(6000).action("Undo", move |s: &mut MainUi, _| s.clip.restore(back.clone())));
    s.clip_view.list_focus.focused = true;
}

fn toggle_pin(s: &mut MainUi, list: Vec<ClipEntry>) {
    if list.is_empty() {
        return;
    }
    let pin = !list.iter().all(|e| e.pinned);
    for e in &list {
        s.clip.set_pinned(e.id, pin);
    }
    let msg = if pin { if list.len() > 1 { format!("Pinned {} items", list.len()) } else { "Pinned. It won't expire.".into() } } else { "Unpinned".into() };
    say(s, msg);
}

/// `toSnippet`: a new snippet from the full text, in All snippets.
fn to_snippet(s: &mut MainUi, e: Option<ClipEntry>) {
    let Some(e) = e.filter(has_text) else { return };
    let inline = clip::inline_full_text(&e).map(str::to_string);
    let land = |s: &mut MainUi, content: String| {
        let mut draft = serde_json::Map::new();
        draft.insert("content".into(), serde_json::json!(content));
        s.new_snippet(draft);
        s.set_view(View::All);
    };
    match inline {
        Some(t) => land(s, t),
        None => {
            let (id, fallback) = (e.id, e.text.clone().unwrap_or_default());
            s.spawn(move || crate::backend::clipboard::clipboard_text(id).unwrap_or(fallback), move |s, t, _| land(s, t));
        }
    }
}

fn open(s: &mut MainUi, e: Option<ClipEntry>) {
    let Some(e) = e.filter(|e| matches!(e.kind, ClipKind::Link | ClipKind::File)) else { return };
    if let Err(err) = crate::backend::clipboard::clipboard_open(e.id, None) {
        s.toasts.push(Toast::error("Couldn't open it").description(err));
    }
}

fn reveal_in_finder(e: Option<ClipEntry>) {
    if let Some(e) = e.filter(|e| e.kind == ClipKind::File) {
        let _ = crate::backend::clipboard::clipboard_reveal(e.id, None);
    }
}

fn toggle_recording(s: &mut MainUi) {
    let on = !s.settings.clipboard_enabled;
    s.update_settings(serde_json::json!({ "clipboardEnabled": on }));
}

fn search_focused(s: &MainUi) -> bool {
    s.clip_view.search.focused
}

/// The page's `useHotkeys` (all but ⌘⌫ work while typing in the search field). Also for the
/// window's `on_key` when nothing has focus.
pub fn on_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if !matches!(s.view, View::Clipboard(_)) || !ev.mods.cmd || ev.mods.alt {
        return false;
    }
    sync_filter(s);
    match &ev.key {
        Key::Command('f') => focus_search(s, tx),
        Key::Command('l') => focus_list(s, tx),
        Key::Command('p') => {
            let sel = selection(s);
            toggle_pin(s, sel);
        }
        Key::Command('s') => match active_entry(s) {
            Some(e) if has_text(&e) => to_snippet(s, Some(e)),
            _ => return false,
        },
        Key::Command('o') => match active_entry(s) {
            Some(e) if matches!(e.kind, ClipKind::Link | ClipKind::File) => open(s, Some(e)),
            _ => return false,
        },
        Key::Command('r') => match active_entry(s) {
            Some(e) if e.kind == ClipKind::File => reveal_in_finder(Some(e)),
            _ => return false,
        },
        Key::Backspace | Key::Delete if !search_focused(s) => {
            let sel = selection(s);
            remove(s, sel, tx);
        }
        _ => return false,
    }
    true
}

/// The listbox's `onKeyDown`.
fn list_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m = ev.mods;
    let plain = !m.cmd && !m.ctrl && !m.alt;
    sync_filter(s);
    match &ev.key {
        Key::Down | Key::Up => step(s, if ev.key == Key::Down { 1 } else { -1 }, m.shift, tx),
        Key::Text(t) if plain && (t == "j" || t == "k") => step(s, if t == "j" { 1 } else { -1 }, false, tx),
        Key::Text(t) if plain && (t == "J" || t == "K") => step(s, if t == "J" { 1 } else { -1 }, true, tx),
        Key::Home | Key::End => {
            let n = order_ids(s).len() as i64;
            step(s, if ev.key == Key::Home { -n } else { n }, m.shift, tx);
        }
        Key::PageDown | Key::PageUp => step(s, if ev.key == Key::PageDown { 8 } else { -8 }, m.shift, tx),
        Key::Enter => {
            let e = active_entry(s);
            copy(s, e, m.shift);
        }
        Key::Text(t) if t == " " && !m.cmd => {
            if active_entry(s).is_some() {
                s.clip_view.quick_look = true;
            }
        }
        Key::Backspace | Key::Delete if !m.cmd => {
            let sel = selection(s);
            remove(s, sel, tx);
        }
        Key::Command('c') => {
            let e = active_entry(s);
            copy(s, e, false);
        }
        Key::Command('a') => {
            let all = order_ids(s).into_iter().collect();
            set_multi(s, all);
        }
        Key::Escape if !multi(s).is_empty() => set_multi(s, HashSet::new()),
        Key::Text(t) if t == "/" => focus_search(s, tx),
        Key::Text(t) if plain && t.chars().count() == 1 && !t.trim().is_empty() => {
            // Type to search: the key lands in the search field.
            let mut q = s.clip_view.search.text.clone();
            q.push_str(t);
            s.clip_view.search.replace_text(q.clone());
            s.clip_view.search.set_caret(q.len());
            s.clip_view.search.focused = true;
        }
        Key::Escape => return false,
        Key::Tab => return false,
        _ if m.cmd => return on_key(s, ev, tx),
        _ => return false,
    }
    true
}

/// The search field's `onKeyDown`.
fn search_key(s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Down | Key::Up if !ev.mods.cmd => step(s, if ev.key == Key::Down { 1 } else { -1 }, ev.mods.shift, tx),
        Key::Enter if !ev.mods.cmd => {
            let e = active_entry(s);
            copy(s, e, ev.mods.shift);
        }
        Key::Escape => {
            if s.clip_view.search.text.is_empty() {
                s.clip_view.list_focus.focused = true;
            } else {
                s.clip_view.search.replace_text("");
            }
        }
        _ => return false,
    }
    true
}

/// `onPointer`: ⌘ toggles a row in the multi-selection, ⇧ extends it from the anchor, a plain
/// press selects one row. The list takes the keyboard, as the focused listbox did.
fn on_pointer(s: &mut MainUi, id: Uuid, mods: Mods, tx: &mut Tx) {
    sync_filter(s);
    let ids = order_ids(s);
    if mods.cmd || mods.ctrl {
        let mut n: HashSet<Uuid> = if s.clip_view.multi.is_empty() { s.clip_view.active.into_iter().collect() } else { (*s.clip_view.multi).clone() };
        if !n.remove(&id) {
            n.insert(id);
        }
        set_multi(s, n);
    } else if let (true, Some(a)) = (mods.shift, s.clip_view.anchor) {
        let (ai, bi) = (ids.iter().position(|x| *x == a), ids.iter().position(|x| *x == id));
        if let (Some(ai), Some(bi)) = (ai, bi) {
            let (lo, hi) = if ai < bi { (ai, bi) } else { (bi, ai) };
            set_multi(s, ids[lo..=hi].iter().copied().collect());
        }
    } else {
        s.clip_view.anchor = Some(id);
        set_multi(s, HashSet::new());
    }
    let index = ids.iter().position(|x| *x == id).unwrap_or(0);
    s.clip_view.active = Some(id);
    s.clip_view.last_index = index;
    s.clip_view.list_focus.focused = true;
    let _ = tx;
}

fn on_pick(s: &mut MainUi, id: Uuid, _tx: &mut Tx) {
    let e = s.clip.get(id).cloned();
    copy(s, e, false);
}

// MARK: Overlays

/// `animate-rise`: fades in from 4 pt below on `--motion-slow` `--spring-soft`.
pub fn rise() -> Transition {
    let spring = crate::ui::theme::motion::soft();
    Transition {
        insertion: Effect { opacity: 0.0, offset: Vec2::new(0.0, 4.0), ..Effect::IDENTITY },
        removal: Effect::IDENTITY,
        pop_layout: true,
        springs: SpringPair::new(spring, Spring::new(0.01, 0.0)),
    }
}

/// A toolbar action: an icon button with a tooltip naming it and its shortcut.
#[allow(clippy::too_many_arguments)]
fn action(th: &Theme, cx: &mut Cx<MainUi>, layer: &mut Vec<El>, key: &str, label: &str, combo: Option<&str>, glyph: &'static rune::Icon, destructive: bool, active: bool, f: impl Fn(&mut MainUi, &mut Tx) + 'static) -> El {
    let t = &th.t;
    let mut b = Button::icon_only(glyph, label)
        .variant(Variant::Ghost)
        .size(Size::IconSm)
        .id(format!("clip-action-{key}"))
        .color(if active { t.iris_foreground } else { t.muted_foreground })
        .on_press_tx(f);
    if destructive {
        b = b.hover_color(t.destructive);
    }
    let mut btn = b.build(th);
    if active {
        btn = btn.checked(true);
    }
    let mut tip = Tooltip::new(format!("clip-tip-{key}")).text(label.to_string());
    if let Some(c) = combo {
        tip = tip.shortcut(c);
    }
    tip.build(cx, btn, layer)
}

fn welcome(th: &Theme, s: &MainUi) -> El {
    let t = &th.t;
    let enabled = s.settings.clipboard_enabled;
    let tile = el()
        .size(48.0, 48.0)
        .center()
        .radius(radius::XXL)
        .fill(t.surface_raised)
        .elevate(&t.elev_raised)
        .sheen(false)
        .child(icon(&icons::CLIPBOARD_LIST, 20.0).color(t.muted_foreground));
    let words = |title: &str, body: String| {
        col()
            .gap(6.0)
            .items_center()
            .child(text(title.to_string()).font(ty::SM.weight_of(weight::MEDIUM)).color(t.foreground))
            .child(text(body).font(ty::XS.lh(19.5)).color(t.muted_foreground).wrap().align(Align::Center))
    };
    let mut c = col().key("clip-welcome").w_full().items_center().gap(16.0).px(32.0).pt(64.0).transition(rise()).child(tile);
    if enabled {
        c = c
            .child(words(
                "Everything you copy, in one place",
                format!("Copy text, links, images or files and they appear here with the time you copied them. Kudzu keeps them {}, only on this Mac.", clip::retention_label(s.settings.clipboard_retention_days)),
            ))
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(text("Paste from history anywhere").font(ty::XS).color(t.muted_foreground))
                    .child(Shortcut::keys(accelerator_labels(&s.settings.clipboard_hotkey)).tone(Tone::Accent).build(th)),
            );
    } else {
        c = c.child(words("Clipboard history is off", "Turn it on to find anything you copied, from any app.".into())).child(
            Button::new("Turn on").size(Size::Sm).icon(&icons::PLAY).on_press(|s: &mut MainUi| s.update_settings(serde_json::json!({ "clipboardEnabled": true }))).build(th),
        );
    }
    c
}

fn no_matches(th: &Theme, s: &MainUi) -> El {
    let t = &th.t;
    let query = s.clip_view.search.text.clone();
    let line = if query.is_empty() {
        text(format!("No {} yet", js_lower(filter_label(filter(s))))).font(ty::SM).color(t.muted_foreground)
    } else {
        let msg = format!("Nothing matches “{query}”");
        let start = "Nothing matches ".len();
        rune::rich(msg.clone(), vec![rune::Span::new(start, msg.len()).color(t.foreground)]).font(ty::SM).color(t.muted_foreground)
    };
    col()
        .key("clip-no-matches")
        .w_full()
        .items_center()
        .gap(12.0)
        .px(24.0)
        .pt(64.0)
        .transition(rise())
        .child(line)
        .child(
            Button::new("Show everything")
                .size(Size::Sm)
                .variant(Variant::Secondary)
                .on_press(|s: &mut MainUi| {
                    s.clip_view.search.replace_text("");
                    s.set_view(View::Clipboard(ClipFilter::All));
                })
                .build(th),
        )
}

fn js_lower(s: &str) -> String {
    kudzu_core::client::js::lower(s)
}

/// The clipped search field (`h-8 rounded-lg bg-foreground/[0.03] shadow-well pl-8 pr-14`).
fn search_field(th: &Theme, s: &MainUi) -> El {
    let t = &th.t;
    let focused = s.clip_view.search.focused;
    let input = text_input(&s.clip_view.search, ty::BASE)
        .single_line()
        .key("clip-search-input")
        .grow(1.0)
        .min_w(0.0)
        .color(t.foreground)
        .placeholder("Search history", t.faint)
        .caret(t.foreground, t.iris_soft)
        .label("Search clipboard history")
        .description("Type app: followed by an app name, or is: followed by a kind such as image or link, to narrow results.")
        .on_edit(|s: &mut MainUi| &mut s.clip_view.search)
        .on_input_key(search_key);
    let trailing = if s.clip_view.search.text.is_empty() {
        Shortcut::new("mod+f").build(th).inert()
    } else {
        el().key("clip-search-clear")
            .size(20.0, 20.0)
            .center()
            .radius(4.0)
            .role("button")
            .label("Clear search")
            .hover_group()
            .on_press(|s: &mut MainUi| s.clip_view.search.replace_text(""))
            .child(icon(&icons::X, 14.0).color(t.faint).group_color(t.foreground))
    };
    let mut field = row()
        .key("clip-search")
        .w_full()
        .h(32.0)
        .pl(32.0)
        .pr(56.0)
        .center_y()
        .radius(radius::LG)
        .child(input)
        .child(el().absolute(10.0, 9.0).child(icon(&icons::SEARCH, 14.0).color(t.faint)).inert())
        .child(el().pin(None, Some(0.0), Some(8.0), Some(0.0)).center_y().child(trailing))
        .motion(Spring::new(0.15, 0.0));
    field = if focused {
        field.fill(Color::TRANSPARENT).outline(2.0, 0.0, t.ring)
    } else {
        field.fill(Theme::tint(t.foreground, 0.03)).elevate(&t.elev_well)
    };
    el().px(12.0).pb(8.0).child(field)
}

#[allow(dead_code)] // until the frame passes its overlay layer (`view_with`)
/// The page alone, its overlays (menus, tooltips, the large preview, Clear history) drawn over
/// it inside the main card. [`view_with`] mounts them in the window's overlay layer instead.
pub fn view(s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    let mut layer = Vec::new();
    let page = view_with(s, cx, &mut layer);
    // Interim: the overlays draw inside the card (dialogs land offset by the card's origin).
    el().w_full().h_full().child(page).child(el().absolute(0.0, 0.0).w_full().h_full().children(layer))
}

#[allow(dead_code)] // until the frame calls it
/// The page, pushing its overlays into the window's `layer` (as the other main-window screens do).
pub fn view_with(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let f = filter(s);
    let window_w = cx.layout_size("main-root").map_or(1180.0, |sz| sz[0]);
    let entries = &s.clip.entries;
    let enabled = s.settings.clipboard_enabled;
    let retention = s.settings.clipboard_retention_days;
    let (results, terms) = {
        let r = rows(s);
        (r.order.len(), r.terms.clone())
    };
    let cur = current(s);
    let active = cur.and_then(|(_, id)| s.clip.get(id));
    let m = multi(s);
    let bulk: Vec<&ClipEntry> = if m.len() > 1 {
        let r = rows(s);
        r.order.iter().map(|&i| &entries[i]).filter(|e| m.contains(&e.id)).collect()
    } else {
        Vec::new()
    };

    // MARK: Header
    let title = row()
        .grow(1.0)
        .min_w(0.0)
        .center_y()
        .window_drag()
        .child(text(if f == ClipFilter::All { "Clipboard".to_string() } else { filter_label(f).to_string() }).font(ty::BASE.weight_of(weight::SEMIBOLD)).color(t.foreground).truncate().shrink())
        .child(text(results.to_string()).font(ty::BASE.features("'tnum' 1")).color(t.faint).ml(4.0));
    let pill = {
        let (fill, hover, ink, hover_ink, dot) = if enabled {
            (Color::TRANSPARENT, Theme::tint(t.foreground, 0.05), t.muted_foreground, t.foreground, t.success)
        } else {
            (Theme::tint(t.warning, 0.12), Theme::tint(t.warning, 0.06), t.warning, t.warning, t.warning)
        };
        let mut d = el().size(6.0, 6.0).radius(3.0).fill(dot);
        if enabled {
            d = d.box_shadow(Shadow { spread: 3.0, color: Theme::tint(t.success, 0.2), ..Shadow::default() });
        }
        let b = row()
            .key("clip-recording")
            .h(24.0)
            .gap(6.0)
            .px(8.0)
            .center_y()
            .radius(radius::FULL)
            .fill(fill)
            .hover(hover)
            .hover_group()
            .role("button")
            .checked(enabled)
            .label(if enabled { "Recording copies. Press to pause." } else { "Paused. Press to resume recording." })
            .focusable(true)
            .focus_ring(2.0, 0.0, t.ring)
            .on_press(toggle_recording)
            .child(d)
            .child(text(if enabled { "Recording" } else { "Paused" }).font(ty::XS2.weight_of(weight::MEDIUM)).color(ink).group_color(hover_ink));
        Tooltip::new("clip-recording").text(if enabled { "Pause recording" } else { "Resume recording" }).build(cx, b, layer)
    };
    let more = Button::icon_only(&icons::ELLIPSIS, "Clipboard options").variant(Variant::Ghost).size(Size::IconSm).color(t.muted_foreground).haspopup().expanded(s.clip_view.menu.open).id("clip-options").build(th);
    let mut keep = SubMenu::new("Keep history").icon(&icons::CLIPBOARD_LIST).width(176.0).label("Delete items after");
    for (days, label) in clip::RETENTION_OPTIONS {
        keep = keep.item(MenuItem::radio(label, retention == days).on_select(move |s: &mut MainUi, _| s.update_settings(serde_json::json!({ "clipboardRetentionDays": days }))));
    }
    let menu = DropdownMenu::new("clip-options-menu", &s.clip_view.menu, |s: &mut MainUi| &mut s.clip_view.menu)
        .trigger(more)
        .align(Align::End)
        .width(240.0)
        .item(MenuItem::new(if enabled { "Pause recording" } else { "Resume recording" }).icon(if enabled { &icons::PAUSE } else { &icons::PLAY }).on_select(|s: &mut MainUi, _| toggle_recording(s)))
        .sub(keep)
        .item(MenuItem::new("Clipboard settings…").icon(&icons::SETTINGS_2).on_select(|s: &mut MainUi, _| s.set_view(View::Settings(Some(SettingsSection::Clipboard)))))
        .separator()
        .item(MenuItem::new("Clear history…").icon(&icons::TRASH).destructive().disabled(entries.is_empty()).on_select(|s: &mut MainUi, _| s.clip_view.clear_open = true))
        .build(cx, layer);
    let menu = Tooltip::new("clip-options-tip").text("Options").build(cx, menu, layer);
    let header = row().key("clip-header").h(48.0).w_full().center_y().gap(8.0).pr(8.0).pl(20.0).window_drag().child(title).child(pill).child(menu);

    // MARK: List
    let empty = if !s.clip.ready {
        el()
    } else if entries.is_empty() {
        welcome(th, s)
    } else {
        no_matches(th, s)
    };
    let list = ClipList::<MainUi> {
        key: LIST,
        rows: Rc::new(|s: &MainUi| rows(s)),
        entries: Rc::new(|s: &MainUi| s.clip.entries.as_slice()),
        look: Rc::new(|s: &MainUi| {
            let m = multi(s);
            Look { active: current(s).map(|(_, id)| id), focused: s.clip_view.list_focus.focused || m.len() <= 1, selection: m, show_numbers: false, show_secrets: s.settings.clipboard_show_secrets }
        }),
        density: Density::Comfortable,
        numbered: false,
        label: "Clipboard history".into(),
        on_pointer: Rc::new(on_pointer),
        on_pick: Rc::new(on_pick),
    }
    .build(th, s, empty);
    // The listbox's keyboard: an invisible field the list focuses.
    let proxy = text_input(&s.clip_view.list_focus, ty::BASE)
        .single_line()
        .key("clip-list-keys")
        .size(1.0, 1.0)
        // Drawn, so it can take focus, but with nothing to see.
        .color(Color::TRANSPARENT)
        .caret(Color::TRANSPARENT, Color::TRANSPARENT)
        .no_focus_ring()
        .label("Clipboard history")
        .on_edit(|s: &mut MainUi| &mut s.clip_view.list_focus)
        .on_input_key(list_key);
    let list_box = col().key("clip-list-box").w_full().grow(1.0).min_h(0.0).child(el().absolute(0.0, 0.0).child(proxy)).child(list);

    let mut left = col()
        .key("clip-left")
        .w((window_w * 0.30).clamp(300.0, 372.0))
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 1.0, 0.0, 0.0], t.hairline)
        .role("region")
        .label("Clipboard history")
        .child(header)
        .child(search_field(th, s));
    if !enabled && !entries.is_empty() {
        left = left.child(
            row()
                .key("clip-paused")
                .mx(12.0)
                .mb(8.0)
                .gap(8.0)
                .center_y()
                .radius(radius::LG)
                .fill(Theme::tint(t.warning, 0.1))
                .px(12.0)
                .py(8.0)
                .box_shadow(Shadow { spread: 1.0, color: Theme::tint(t.warning, 0.18), inset: true, ..Shadow::default() })
                .role("status")
                .child(icon(&icons::PAUSE, 14.0).color(t.warning))
                .child(text("Recording is paused. New copies aren't saved.").font(ty::XS).color(t.warning).grow(1.0).wrap())
                .child(
                    el().role("button")
                        .label("Resume")
                        .hover_group()
                        .on_press(|s: &mut MainUi| s.update_settings(serde_json::json!({ "clipboardEnabled": true })))
                        .child(secret::underlined(text("Resume").font(ty::XS.weight_of(weight::MEDIUM)), t.warning, t.warning)),
                ),
        );
    }
    left = left.child(list_box);
    if !bulk.is_empty() {
        let all_pinned = bulk.iter().all(|e| e.pinned);
        let pin = action(th, cx, layer, "bulk-pin", if all_pinned { "Unpin" } else { "Pin" }, Some("mod+p"), if all_pinned { &icons::PIN_OFF } else { &icons::PIN }, false, false, |s, _| {
            let sel = selection(s);
            toggle_pin(s, sel);
        });
        let del = action(th, cx, layer, "bulk-delete", "Delete", Some("mod+backspace"), &icons::TRASH, true, false, |s, tx| {
            let sel = selection(s);
            remove(s, sel, tx);
        });
        left = left.child(
            row()
                .key("clip-bulk")
                .m_all(8.0)
                .gap(4.0)
                .center_y()
                .radius(radius::XL)
                .fill(t.surface_raised)
                .padding(6.0)
                .pl(12.0)
                .elevate(&t.elev_float)
                .role("toolbar")
                .label("Selection actions")
                .transition(rise())
                .child(text(format!("{} selected", bulk.len())).font(ty::XS.weight_of(weight::MEDIUM).features("'tnum' 1")).color(t.foreground).grow(1.0))
                .child(pin)
                .child(del),
        );
    } else {
        let mut kept = format!("Kept {}", clip::retention_label(retention));
        if s.clip.disk_usage > 0 {
            kept.push_str(&format!(" · {}", clip::format_bytes(s.clip.disk_usage)));
        }
        left = left.child(
            row()
                .key("clip-footer")
                .h(36.0)
                .w_full()
                .gap(4.0)
                .center_y()
                .px(16.0)
                .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
                .child(text(kept).font(ty::XS2).color(t.faint).truncate().shrink())
                .child(el().grow(1.0))
                .child(
                    el().key("clip-change")
                        .role("button")
                        .label("Change")
                        .radius(4.0)
                        .px(4.0)
                        .hover_group()
                        .focusable(true)
                        .focus_ring(2.0, 0.0, t.ring)
                        .on_press(|s: &mut MainUi| s.set_view(View::Settings(Some(SettingsSection::Clipboard))))
                        .child(text("Change").font(ty::XS2).color(t.muted_foreground).group_color(t.foreground)),
                ),
        );
    }

    // MARK: Preview
    let show = s.settings.clipboard_show_secrets;
    let pane_w = (window_w - crate::ui::main::frame::SIDEBAR_W - 8.0 - (window_w * 0.30).clamp(300.0, 372.0)).max(300.0);
    let preview_w = cx.layout_size("clip-preview-body").map_or(pane_w - 48.0, |sz| sz[0]);
    let window_h = cx.layout_size("main-root").map_or(760.0, |sz| sz[1]);
    let mut header2 = row().key("clip-preview-header").h(48.0).w_full().center_y().gap(8.0).px(24.0).border_sides([0.0, 0.0, 1.0, 0.0], t.hairline).window_drag();
    let mut right = col().key("clip-right").grow(1.0).min_w(0.0).h_full().min_h(0.0).role("region").label("Preview");
    if let Some(e) = active {
        let crumb = if is_secret(e) && !show { "Secret".to_string() } else { super::list::html_ws(&describe(e).title) };
        header2 = header2.child(
            Breadcrumbs::new(vec![
                Crumb::new(if f == ClipFilter::All { "History" } else { filter_label(f) }).on_select_tx(|s: &mut MainUi, tx| focus_list(s, tx)),
                Crumb::new(crumb),
            ])
            .label("Clipboard item breadcrumb")
            .build(th)
            .grow(1.0),
        );
        right = right.child(header2);
        let body = col()
            .key("clip-preview-body")
            .w_full()
            .gap(24.0)
            .transition(rise())
            .child(preview::heading::<MainUi>(th, e))
            .child(preview::content(s, cx, layer, e, &terms, false, preview_w, window_h))
            .child(preview::details(s, cx, layer, e, false));
        right = right.child(
            col()
                .key(format!("clip-preview-{}", e.id))
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(rune::ScrollSpec { stick_bottom: false })
                .child(el().w_full().px(24.0).py(20.0).child(body)),
        );
        // Item actions.
        let wide = window_w > 1100.0;
        let mut copy_btn = Button::new("Copy").size(Size::Sm).icon(&icons::COPY).id("clip-copy").on_press(|s: &mut MainUi| {
            let e = active_entry(s);
            copy(s, e, false);
        });
        if wide {
            copy_btn = copy_btn.shortcut("enter");
        }
        let mut bar = row()
            .key("clip-actions")
            .h(56.0)
            .w_full()
            .gap(4.0)
            .center_y()
            .px(16.0)
            .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
            .role("toolbar")
            .label("Item actions")
            .child(copy_btn.build(th).key("copy").shortcut_hint("Enter"));
        if e.rich {
            let mut b = Button::new(if s.settings.clipboard_paste_plain { "With formatting" } else { "As plain text" }).size(Size::Sm).variant(Variant::Ghost).icon(&icons::TYPE).id("clip-plain").on_press(|s: &mut MainUi| {
                let e = active_entry(s);
                copy(s, e, true);
            });
            if wide {
                b = b.child(Shortcut::new("shift+enter").tone(Tone::Ghost).build(th));
            }
            bar = bar.child(b.build(th).key("plain").shortcut_hint("Shift+Enter"));
        }
        bar = bar.child(el().key("spacer").grow(1.0));
        if has_text(e) {
            bar = bar.child(action(th, cx, layer, "snippet", "Save as snippet", Some("mod+s"), &icons::TEXT_CURSOR_INPUT, false, false, |s, _| {
                let e = active_entry(s);
                to_snippet(s, e);
            }));
        }
        if matches!(e.kind, ClipKind::Link | ClipKind::File) {
            bar = bar.child(action(th, cx, layer, "open", if e.kind == ClipKind::Link { "Open link" } else { "Open" }, Some("mod+o"), &icons::EXTERNAL_LINK, false, false, |s, _| {
                let e = active_entry(s);
                open(s, e);
            }));
        }
        if e.kind == ClipKind::File {
            bar = bar.child(action(th, cx, layer, "reveal", "Show in Finder", Some("mod+r"), &icons::FOLDER_OPEN, false, false, |s, _| reveal_in_finder(active_entry(s))));
        }
        if has_text(e) {
            let secret_now = is_secret(e);
            bar = bar.child(action(
                th,
                cx,
                layer,
                "secret",
                if secret_now { "Not a secret — show it" } else { "Hide as a secret" },
                None,
                if secret_now { &icons::SHIELD_OFF } else { &icons::SHIELD_CHECK },
                false,
                false,
                |s, _| {
                    if let Some(e) = active_entry(s) {
                        s.set_secret_with_feedback(e.id, !is_secret(&e));
                    }
                },
            ));
        }
        bar = bar.child(action(th, cx, layer, "pin", if e.pinned { "Unpin" } else { "Pin — keep forever" }, Some("mod+p"), if e.pinned { &icons::PIN_OFF } else { &icons::PIN }, false, e.pinned, |s, _| {
            let e: Vec<ClipEntry> = active_entry(s).into_iter().collect();
            toggle_pin(s, e);
        }));
        bar = bar.child(action(th, cx, layer, "delete", "Delete", Some("mod+backspace"), &icons::TRASH, true, false, |s, tx| {
            let e: Vec<ClipEntry> = active_entry(s).into_iter().collect();
            remove(s, e, tx);
        }));
        right = right.child(bar);
    } else {
        right = right.child(header2).child(
            el().grow(1.0)
                .w_full()
                .center()
                .child(text(if entries.is_empty() { "" } else { "Select an item to preview it" }).font(ty::XS).color(t.faint)),
        );
    }

    // MARK: Large preview (Space)
    if let Some(e) = active.filter(|_| s.clip_view.quick_look) {
        let title = if is_secret(e) && !show { "Secret".to_string() } else { describe(e).title };
        let width = 920.0f32.min(window_w * 0.9);
        let body = col()
            .key("clip-quicklook-body")
            .w_full()
            .max_h(window_h * 0.86)
            .focusable(true)
            .no_focus_ring()
            .on_key(|s: &mut MainUi, ev: &KeyEvent, tx: &mut Tx| match ev.key {
                Key::Text(ref t) if t == " " && !ev.mods.cmd => {
                    s.clip_view.quick_look = false;
                    true
                }
                Key::Down | Key::Up => {
                    step(s, if ev.key == Key::Down { 1 } else { -1 }, false, tx);
                    true
                }
                _ => false,
            })
            .child(el().w_full().px(24.0).py(14.0).pr(48.0).border_sides([0.0, 0.0, 1.0, 0.0], t.hairline).child(preview::heading::<MainUi>(th, e)))
            .child(
                col()
                    .key(format!("clip-quicklook-{}", e.id))
                    .w_full()
                    .shrink()
                    .min_h(0.0)
                    .scroll(rune::ScrollSpec { stick_bottom: false })
                    .child(el().w_full().padding(24.0).child(preview::content(s, cx, layer, e, &terms, true, width - 48.0, window_h))),
            );
        Dialog::new("clip-quicklook")
            .open(true)
            .on_open_change(|s: &mut MainUi, open, _| s.clip_view.quick_look = open)
            .max_width(width)
            .padding(0.0)
            .gap(0.0)
            .radius(radius::XXL)
            .ring(false)
            .label(title)
            .description("Large preview. Press Space or Escape to close.")
            .child(body.clip_rounded(radius::XXL))
            .build(cx, layer);
    }

    // MARK: Clear history
    if s.clip_view.clear_open {
        clear_dialog(s, cx, layer);
    }

    row()
        .key("clip-page")
        .w_full()
        .h_full()
        .min_h(0.0)
        .min_w(0.0)
        .on_key(on_key)
        .child(left)
        .child(right)
        .child(text(s.clip_view.announce.clone()).live_region(LiveRegion::Polite).absolute(0.0, 0.0).opacity(0.0).size(1.0, 1.0).inert())
}

/// "Clear clipboard history?": the alert dialog with its "Also delete pinned items" choice.
fn clear_dialog(s: &MainUi, cx: &mut Cx<MainUi>, layer: &mut Vec<El>) {
    let th = theme_for(cx);
    let t = &th.t;
    let pinned = s.clip.entries.iter().filter(|e| e.pinned).count();
    let close = |s: &mut MainUi| s.clip_view.clear_open = false;
    let check = Checkbox::new(s.clip_view.clear_pinned).label(format!("Also delete pinned items ({pinned})")).id("clip-clear-pinned").on_toggle(|s: &mut MainUi| s.clip_view.clear_pinned = !s.clip_view.clear_pinned).build(th).tab_index(3);
    let choice = row()
        .key("clip-clear-choice")
        .gap(10.0)
        .center_y()
        .on_press(|s: &mut MainUi| s.clip_view.clear_pinned = !s.clip_view.clear_pinned)
        .child(check)
        .child(text(format!("Also delete pinned items ({pinned})")).font(ty::BASE).color(t.popover_foreground).a11y_hidden());
    // Radix starts on Cancel; Tab goes on to the action, then the checkbox.
    let cancel = Button::new("Cancel").variant(Variant::Outline).id("clip-clear-cancel").on_press(close).build(th).tab_index(1);
    let confirm = Button::new("Clear history")
        .variant(Variant::Destructive)
        .id("clip-clear-action")
        .on_press(|s: &mut MainUi| {
            s.clip_view.clear_open = false;
            let n = s.clip.clear(s.clip_view.clear_pinned);
            s.toasts.push(Toast::success(if n == 1 { "Deleted 1 item".to_string() } else { format!("Deleted {n} items") }));
            say(s, format!("Cleared {n} items"));
        })
        .build(th)
        .tab_index(2);
    // `text-balance`, as the kit's AlertDialog does it: measured on one line, then wrapped at the
    // width that evens out the lines.
    let desc_text = "Everything Kudzu remembered is deleted from this Mac. This can't be undone.";
    let one_line = cx.layout_size("clip-clear-desc-measure").map(|sz| sz[0]);
    let inner = 384.0 - 32.0 - 2.0;
    let mut desc = text(desc_text).font(ty::SM).color(t.muted_foreground).wrap();
    if let Some(w) = one_line.filter(|w| *w > inner) {
        let lines = (w / inner).ceil();
        desc = desc.max_w((w / lines).ceil() + 8.0);
    }
    let desc = col().child(desc).child(text(desc_text).key("clip-clear-desc-measure").font(ty::SM).absolute(0.0, 0.0).opacity(0.0).inert().a11y_hidden());
    let panel = col()
        .role("alertdialog")
        .label("Clear clipboard history?")
        .description("Everything Kudzu remembered is deleted from this Mac. This can't be undone.")
        .radius(radius::XL)
        .fill(t.popover)
        .padding(16.0)
        .gap(16.0)
        .box_shadow(Shadow { spread: 1.0, color: t.foreground.alpha(0.1), ..Shadow::default() })
        .elevate(&t.elev_panel)
        .child(
            col()
                .gap(6.0)
                .child(text("Clear clipboard history?").font(ty::LG.weight_of(weight::MEDIUM)).color(t.popover_foreground).wrap())
                .child(desc),
        )
        .child(choice)
        .child(dialog::footer(cx, 16.0, radius::XL, [cancel, confirm]));
    dialog::mount(cx, layer, "clip-clear", panel, 384.0, Placement::Center, false, Some(Rc::new(close)));
}

trait Margins {
    fn m_all(self, m: f32) -> Self;
}

impl Margins for El {
    fn m_all(self, m: f32) -> El {
        self.mt(m).mb(m).ml(m).mr(m)
    }
}
