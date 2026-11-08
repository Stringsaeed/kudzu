//! The clipboard history picker (`clipboard-picker.tsx`, `app/clipboard/page.tsx`): a floating
//! panel summoned by ⌃⌥V over any app. Type to search, see a live preview, and paste into the app
//! you came from without the mouse.
//!
//! Keys (all on the search field, which keeps focus): ↑↓ (⌃N ⌃P) and Page Up/Down move, ↵
//! pastes into the previous app, ⌘↵ copies only, ⇧ flips plain text, ⌘1–9 paste that row,
//! ⌘P pin, ⌘⌫ delete (⌘Z restores), ⌘S save as snippet, ⌘O open, ⌘, settings, Tab / ⇧Tab
//! cycle the kind filter, Escape clears the search or closes. Holding ⌘ highlights the ⌘1–9
//! hints (needs the window's modifier keys: see `set_handle`).
//!
//! The panel sits on native popover vibrancy (`.vibrant`: `bg-popover/55`, no shadow, the
//! window's 18 pt corners) and closes when it loses focus.

use std::cell::{Ref, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use kudzu_core::client::clipboard::{self as clip, has_text, ClipFilter as CoreFilter};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use kudzu_engine::AppInfo;
use rune::{col, el, icon, row, text, text_input, AppHandle, Color, Cx, El, Key, KeyEvent, Modifier, Mods, SystemMaterial, TextEdit, TextStyle, Tx};
use uuid::Uuid;

use super::list::{row_key, ClipList, Density, Look, Rows};
use super::{preview, ClipData, ClipHost};
use crate::backend::clipboard as backend;
use crate::backend::core::Event;
use crate::ui::icons;
use crate::ui::kit::selection_pill::SelectionPill;
use crate::ui::kit::shortcut::Shortcut;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities};
use crate::ui::theme_for;

/// `PICKER_FILTERS`.
const FILTERS: [(CoreFilter, &str, &str); 6] = [
    (CoreFilter::All, "all", "All"),
    (CoreFilter::Pinned, "pinned", "Pinned"),
    (CoreFilter::Kind(ClipKind::Text), "text", "Text"),
    (CoreFilter::Kind(ClipKind::Image), "image", "Images"),
    (CoreFilter::Kind(ClipKind::Link), "link", "Links"),
    (CoreFilter::Kind(ClipKind::File), "file", "Files"),
];

const LIST: &str = "picker-list";

static HANDLE: OnceLock<AppHandle<PickerUi>> = OnceLock::new();

/// The picker window's handle, for work that lands later (pasting, timed messages, long texts).
/// `ui::run` hands it over after building the window (as it does for the permissions window).
#[allow(dead_code)] // until `ui::run` calls it
pub fn set_handle(h: AppHandle<PickerUi>) {
    let wake = h.clone();
    if HANDLE.set(h).is_ok() {
        super::media::on_loaded(move || wake.update(|_, _| {}));
    }
}

pub struct PickerUi {
    pub clip: ClipData,
    pub paste_plain: bool,
    pub retention_days: u32,
    pub enabled: bool,
    pub show_secrets: bool,
    pub target: Option<AppInfo>,
    pub can_paste: bool,
    pub search: TextEdit,
    pub filter: usize,
    pub active: Option<Uuid>,
    pub show_numbers: bool,
    pub flash: Option<String>,
    flash_gen: u64,
    pub undo: Option<Vec<ClipEntry>>,
    rows: RefCell<Rows>,
}

impl PickerUi {
    /// The history and settings as the backend has them now (`init` + `getState`).
    pub fn init() -> PickerUi {
        let (entries, settings) = match crate::backend::core::try_core() {
            Some(c) => (c.clip.lock().history.entries.clone(), Some(c.store.lock().settings.clone())),
            None => (Vec::new(), None),
        };
        let mut clip = ClipData::default();
        clip.set(backend::ClipboardInfo { entries, disk_usage: 0, can_paste: true });
        let s = settings.unwrap_or_default();
        let mut search = TextEdit::new("");
        search.focused = true;
        PickerUi {
            clip,
            paste_plain: s.clipboard_paste_plain,
            retention_days: s.clipboard_retention_days,
            enabled: s.clipboard_enabled,
            show_secrets: s.clipboard_show_secrets,
            target: None,
            can_paste: true,
            search,
            filter: 0,
            active: None,
            show_numbers: false,
            flash: None,
            flash_gen: 0,
            undo: None,
            rows: RefCell::new(Rows::default()),
        }
    }
}

impl ClipHost for PickerUi {
    fn show_secrets(&self) -> bool {
        self.show_secrets
    }
    fn retention_days(&self) -> u32 {
        self.retention_days
    }
    /// The picker has no toasts: the correction just applies.
    fn set_secret_with_feedback(&mut self, id: Uuid, secret: bool) {
        if let Some(e) = self.clip.get(id) {
            let next = super::secret_correction(e, secret);
            self.clip.set_secret(id, next);
        }
    }
    fn later(&self, ms: u64, f: super::Later<Self>) {
        let Some(h) = HANDLE.get().cloned() else { return };
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(ms));
            h.update(move |s, tx| f(s, tx));
        });
    }
}

/// Bus events: history changes, settings, and the shortcut showing the picker.
pub fn apply(s: &mut PickerUi, ev: &Event, _tx: &mut Tx) -> bool {
    match ev {
        Event::Clipboard(ev) => s.clip.apply(ev),
        Event::Settings(st) => {
            s.paste_plain = st.clipboard_paste_plain;
            s.retention_days = st.clipboard_retention_days;
            s.enabled = st.clipboard_enabled;
            s.show_secrets = st.clipboard_show_secrets;
        }
        Event::ClipboardShow { target, can_paste } => {
            s.target = target.clone();
            s.can_paste = *can_paste;
            s.search.replace_text("");
            s.filter = 0;
            s.active = None;
            s.flash = None;
            s.undo = None;
            s.search.focused = true;
        }
        _ => return false,
    }
    true
}

/// Closes when it loses focus (the window's `blur`).
pub fn on_focus(s: &mut PickerUi, focused: bool, _tx: &mut Tx) {
    if !focused {
        s.show_numbers = false;
        crate::backend::windows::hide_clipboard(false);
    }
}

fn rows(s: &PickerUi) -> Ref<'_, Rows> {
    let query = s.search.text.clone();
    let searching = !query.trim().is_empty();
    // Most recent first: what you just copied is one keystroke away. Pinned have their own filter.
    super::list::cached(&s.rows, &s.clip.entries, s.clip.rev, FILTERS[s.filter].0, &query, false, !searching)
}

/// The active entry: the chosen one, else the first.
fn current(s: &PickerUi) -> Option<(usize, ClipEntry)> {
    let r = rows(s);
    let i = s.active.and_then(|a| r.pos.get(&a).copied()).unwrap_or(0);
    r.order.get(i).map(|&e| (i, s.clip.entries[e].clone()))
}

fn current_id(s: &PickerUi) -> Option<Uuid> {
    let r = rows(s);
    let i = s.active.and_then(|a| r.pos.get(&a).copied()).unwrap_or(0);
    r.id_at(&s.clip.entries, i)
}

fn nth(s: &PickerUi, n: usize) -> Option<ClipEntry> {
    let r = rows(s);
    r.order.get(n).map(|&i| s.clip.entries[i].clone())
}

fn say(s: &mut PickerUi, text: impl Into<String>, ms: u64) {
    s.flash_gen += 1;
    let generation = s.flash_gen;
    s.flash = Some(text.into());
    s.later(
        ms,
        Box::new(move |s, _| {
            if s.flash_gen == generation {
                s.flash = None;
            }
        }),
    );
}

fn plain_for(s: &PickerUi, e: &ClipEntry, shift: bool) -> bool {
    e.rich && s.paste_plain != shift
}

fn hide() {
    backend::hide_clipboard();
}

/// `paste`: into the app the user came from, on a worker (it waits for the paste and hides the
/// picker itself). Without Accessibility it only copies, and says so.
fn paste(s: &mut PickerUi, e: Option<ClipEntry>, shift: bool) {
    let Some(e) = e else { return };
    let plain = plain_for(s, &e, shift);
    let id = e.id;
    let land = move |s: &mut PickerUi, r: Result<&'static str, String>| match r {
        Ok("copied") => {
            say(s, "Copied. To paste directly, allow Kudzu in Accessibility settings.", 2600);
            s.later(1800, Box::new(|_, _| hide()));
        }
        Ok(_) => {}
        Err(err) => say(s, err, 3000),
    };
    match HANDLE.get().cloned() {
        Some(h) => {
            std::thread::spawn(move || {
                let r = backend::clipboard_paste(id, plain);
                h.update(move |s, _| land(s, r));
            });
        }
        // No handle (a headless run without the window wiring): copy only, as without
        // Accessibility.
        None => {
            let r = backend::clipboard_copy(id, plain).map(|()| "copied");
            land(s, r);
        }
    }
}

/// `copyOnly` (⌘↵).
fn copy_only(s: &mut PickerUi, e: Option<ClipEntry>, shift: bool) {
    let Some(e) = e else { return };
    let _ = backend::clipboard_copy(e.id, plain_for(s, &e, shift));
    say(s, "Copied", 1600);
    s.later(420, Box::new(|_, _| hide()));
}

fn step(s: &mut PickerUi, delta: i64, tx: &mut Tx) {
    let r = rows(s);
    if r.order.is_empty() {
        return;
    }
    let order: Vec<Uuid> = r.order.iter().map(|&i| s.clip.entries[i].id).collect();
    drop(r);
    let from = current(s).map_or(0, |(i, _)| i) as i64;
    let n = (from + delta).clamp(0, order.len() as i64 - 1) as usize;
    s.active = Some(order[n]);
    reveal(s, order[n], tx);
}

fn reveal(s: &PickerUi, id: Uuid, tx: &mut Tx) {
    let r = rows(s);
    let Some(i) = r.row_of(&s.clip.entries, id) else { return };
    let key = match i.checked_sub(1).and_then(|p| r.rows.get(p)) {
        Some(super::list::Row::Header { key, .. }) => format!("{LIST}-{key}"),
        _ => s.clip.get(id).map_or_else(String::new, |e| row_key(LIST, e)),
    };
    tx.scroll_into_view(key);
}

/// The search field's `onKeyDown`: every key of the picker.
fn on_key(s: &mut PickerUi, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let m: Mods = ev.mods;
    if let Key::Modifier { modifier, pressed } = ev.key {
        if matches!(modifier, Modifier::SuperLeft | Modifier::SuperRight) {
            s.show_numbers = pressed;
        }
        return false;
    }
    if m.cmd {
        s.show_numbers = true;
    }
    let active = current(s).map(|(_, e)| e);
    match &ev.key {
        Key::Down => step(s, 1, tx),
        Key::Up => step(s, -1, tx),
        Key::Text(t) if m.ctrl && !m.cmd && t.eq_ignore_ascii_case("n") => step(s, 1, tx),
        Key::Text(t) if m.ctrl && !m.cmd && t.eq_ignore_ascii_case("p") => step(s, -1, tx),
        Key::PageDown => step(s, 7, tx),
        Key::PageUp => step(s, -7, tx),
        Key::Enter => {
            if m.cmd {
                copy_only(s, active, m.shift);
            } else {
                paste(s, active, m.shift);
            }
        }
        Key::Command(c) if c.is_ascii_digit() && *c != '0' => {
            let e = nth(s, (*c as usize) - ('1' as usize));
            paste(s, e, m.shift);
        }
        Key::Command('p') => {
            if let Some(e) = active {
                s.clip.set_pinned(e.id, !e.pinned);
                say(s, if e.pinned { "Unpinned" } else { "Pinned. It won't expire." }, 1600);
            }
        }
        Key::Backspace | Key::Delete if m.cmd => {
            if let Some(e) = active {
                let r = rows(s);
                let order: Vec<Uuid> = r.order.iter().map(|&i| s.clip.entries[i].id).collect();
                drop(r);
                let i = order.iter().position(|x| *x == e.id).unwrap_or(0);
                if let Some(next) = order.get(i + 1).or_else(|| i.checked_sub(1).and_then(|p| order.get(p))) {
                    s.active = Some(*next);
                }
                let gone = s.clip.remove(&[e.id]);
                s.undo = Some(gone);
                say(s, "Deleted. ⌘Z to undo", 4000);
            }
        }
        Key::Command('z') if s.undo.is_some() && s.search.text.is_empty() => {
            if let Some(entries) = s.undo.take() {
                s.clip.restore(entries);
            }
            say(s, "Restored", 1600);
        }
        Key::Command('s') => {
            if let Some(e) = active.filter(has_text) {
                let _ = backend::clipboard_to_snippet(e.id);
            }
        }
        Key::Command('o') => {
            if let Some(e) = active.filter(|e| matches!(e.kind, ClipKind::Link | ClipKind::File)) {
                let _ = backend::clipboard_open(e.id, None);
                hide();
            }
        }
        Key::Command(',') => crate::backend::commands::show_main_view("settings:clipboard".into()),
        Key::Tab => {
            let n = FILTERS.len();
            s.filter = (s.filter + if m.shift { n - 1 } else { 1 }) % n;
            s.active = None;
        }
        Key::Escape => {
            if s.search.text.is_empty() {
                hide();
            } else {
                s.search.replace_text("");
                s.active = None;
            }
        }
        // Other ⌘ chords are the field's (select all, copy, paste, undo).
        _ => return false,
    }
    true
}

/// `ClipFilters` (`focusable={false}`): a chip row with one gliding pill; presses keep focus in
/// the search field.
fn filters(s: &PickerUi, cx: &mut Cx<PickerUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let selected = format!("picker-filter-{}", FILTERS[s.filter].1);
    let mut group = row()
        .key("picker-filters")
        .gap(4.0)
        .center_y()
        .radius(radius::FULL)
        .fill(Theme::tint(t.foreground, 0.035))
        .elevate(&t.elev_well)
        .padding(2.0)
        .role("radiogroup")
        .label("Show")
        .preserve_focus_on_press(true)
        .child(SelectionPill::new("picker-filters", Some(selected)).fill(t.surface_raised).radius(radius::FULL).shadows(&t.elev_soft).sheen(false).build(cx));
    for (i, (_, key, label)) in FILTERS.iter().enumerate() {
        let on = i == s.filter;
        group = group.child(
            row()
                .key(format!("picker-filter-{key}"))
                .h(24.0)
                .px(10.0)
                .center_y()
                .radius(radius::FULL)
                .role("radio")
                .label(*label)
                .checked(on)
                .hover(if on { Color::TRANSPARENT } else { Theme::tint(t.foreground, 0.05) })
                .hover_group()
                .press_scale(0.97)
                .on_press(move |s: &mut PickerUi| {
                    s.filter = i;
                    s.active = None;
                    s.search.focused = true;
                })
                .child(text(*label).font(TextStyle::new(11.5, weight::MEDIUM).lh(16.0)).color(if on { t.foreground } else { t.muted_foreground }).group_color(t.foreground)),
        );
    }
    group
}

fn footer_hint(th: &Theme, combo: &str, label: &str) -> El {
    // A disappearing hint must leave before its neighbours reflow into its space.
    row().transition(rune::Transition::none()).gap(6.0).center_y().child(Shortcut::new(combo).build(th)).child(text(label.to_string()).font(TextStyle::new(11.5, weight::NORMAL).lh(16.0)).color(th.t.muted_foreground))
}

pub fn view(s: &PickerUi, cx: &mut Cx<PickerUi>) -> El {
    let th = theme_for(cx);
    let t = &th.t;
    let mut layer: Vec<El> = Vec::new();
    let query = s.search.text.clone();
    let terms = rows(s).terms.clone();
    let active = current(s).map(|(_, e)| e);

    // Search.
    let input = text_input(&s.search, TextStyle::new(17.0, weight::NORMAL).lh(24.0).tracking(-0.17))
        .single_line()
        .key("picker-search-input")
        .grow(1.0)
        .min_w(0.0)
        .color(t.popover_foreground)
        .placeholder("Search clipboard history…", t.faint)
        .caret(t.popover_foreground, t.iris_soft)
        .label("Search clipboard history")
        .role("combobox")
        .expanded(true)
        .on_edit(|s: &mut PickerUi| &mut s.search)
        .on_input_change(|s: &mut PickerUi, _| s.active = None)
        .on_input_key(on_key);
    let search = row()
        .key("picker-search")
        .h(58.0)
        .w_full()
        .gap(12.0)
        .px(16.0)
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
        .child(icon(&icons::SEARCH, 18.0).color(t.muted_foreground))
        .child(input)
        .child(filters(s, cx));

    // Results.
    let empty = {
        let (a, b) = if !s.enabled {
            ("Clipboard history is paused.".to_string(), "Turn it on in Kudzu's settings.")
        } else if s.clip.entries.is_empty() {
            ("Nothing copied yet.".to_string(), "Copy something in any app and it shows up here.")
        } else if !query.is_empty() {
            (format!("Nothing matches “{query}”."), "Try another filter, or app: and is: to narrow it down.")
        } else {
            ("Nothing here yet.".to_string(), "Try another filter, or app: and is: to narrow it down.")
        };
        col()
            .key("picker-empty")
            .w_full()
            .h(cx.layout_size("picker-list-col").map_or(380.0, |sz| sz[1] - 6.0))
            .center()
            .gap(8.0)
            .px(32.0)
            .child(icon(&icons::CLIPBOARD_LIST, 20.0).color(t.faint))
            .child(text(a).font(ty::SM).color(t.muted_foreground).wrap())
            .child(text(b).font(ty::XS).color(t.faint).wrap())
    };
    let list = ClipList::<PickerUi> {
        key: LIST,
        rows: Rc::new(|s: &PickerUi| rows(s)),
        entries: Rc::new(|s: &PickerUi| s.clip.entries.as_slice()),
        look: Rc::new(|s: &PickerUi| Look { active: current_id(s), selection: Rc::new(HashSet::new()), focused: true, show_numbers: s.show_numbers, show_secrets: s.show_secrets }),
        density: Density::Compact,
        numbered: true,
        label: if query.is_empty() { "Recent copies".into() } else { "Matches".into() },
        on_pointer: Rc::new(|s: &mut PickerUi, id, _mods, _tx| s.active = Some(id)),
        on_pick: Rc::new(|s: &mut PickerUi, id, _tx| {
            let e = s.clip.get(id).cloned();
            paste(s, e, false);
        }),
    }
    .build(th, s, empty);
    let results = col().key("picker-list-col").w_frac(0.5).h_full().min_h(0.0).pt(6.0).child(list);

    // Preview.
    let mut aside = col()
        .key("picker-preview")
        .grow(1.0)
        .min_w(0.0)
        .h_full()
        .min_h(0.0)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.hairline)
        .fill(Theme::tint(t.foreground, 0.015))
        .role("region")
        .label("Preview");
    if let Some(e) = &active {
        let w = cx.layout_size("picker-preview").map_or(400.0, |sz| sz[0]) - 40.0;
        let h = cx.layout_size("picker-preview").map_or(420.0, |sz| sz[1]);
        aside = aside
            .child(el().w_full().px(20.0).pt(14.0).pb(10.0).child(preview::heading::<PickerUi>(th, e)))
            .child(
                col()
                    .key(format!("picker-preview-{}", e.id))
                    .w_full()
                    .grow(1.0)
                    .min_h(0.0)
                    .scroll(rune::ScrollSpec { stick_bottom: false })
                    .child(el().w_full().px(20.0).pb(16.0).child(preview::content(s, cx, &mut layer, e, &terms, false, w, h))),
            )
            .child(el().w_full().px(20.0).py(12.0).border_sides([1.0, 0.0, 0.0, 0.0], t.hairline).child(preview::details(s, cx, &mut layer, e, true)));
    }
    let body = row().key("picker-body").w_full().grow(1.0).min_h(0.0).child(results).child(aside);

    // Footer hints.
    let paste_label = if s.can_paste { s.target.as_ref().map_or("Paste".to_string(), |t| format!("Paste into {}", t.name)) } else { "Copy".to_string() };
    let enter = el()
        .size(18.0, 18.0)
        .center()
        .radius(5.0)
        .fill(t.iris)
        .elevate(&t.elev_accent)
        .sheen(true)
        .child(icon(&icons::CORNER_DOWN_LEFT, 12.0).color(t.iris_contrast));
    let hint_style = TextStyle::new(11.5, weight::NORMAL).lh(16.0);
    let mut footer = row()
        .key("picker-footer")
        .h(40.0)
        .w_full()
        .gap(16.0)
        .px(16.0)
        .center_y()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.hairline)
        .child(
            row()
                .gap(6.0)
                .center_y()
                .min_w(0.0)
                .shrink()
                .child(enter)
                .child(text(s.flash.clone().unwrap_or(paste_label)).font(hint_style).color(if s.flash.is_some() { t.foreground } else { t.muted_foreground }).truncate().shrink()),
        );
    if s.flash.is_none() {
        if s.can_paste {
            footer = footer.child(footer_hint(th, "mod+enter", "Copy").key("copy"));
        }
        if active.as_ref().is_some_and(|e| e.rich) {
            footer = footer.child(footer_hint(th, "shift+enter", if s.paste_plain { "Formatted" } else { "Plain text" }).key("plain"));
        }
        footer = footer
            .child(footer_hint(th, "mod+p", if active.as_ref().is_some_and(|e| e.pinned) { "Unpin" } else { "Pin" }).key("pin"))
            .child(footer_hint(th, "mod+backspace", "Delete").key("delete"));
    }
    footer = footer.child(el().key("spacer").grow(1.0)).child(footer_hint(th, "tab", "Filter").key("filter")).child(Shortcut::new("escape").build(th).key("escape"));

    let spoken = active.as_ref().map_or(String::new(), |e| if clip::is_secret(e) && !s.show_secrets { "Hidden secret".into() } else { clip::describe(e).title });
    let panel = col()
        .key("picker-panel")
        .w_full()
        .h_full()
        .radius(18.0)
        .clip_rounded(18.0)
        .system_material(SystemMaterial::Popover)
        .fill(t.popover)
        .role("dialog")
        .label("Clipboard history")
        .preserve_focus_on_press(true)
        .child(el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(Theme::tint(t.popover, 0.55)).inert())
        .child(search)
        .child(body)
        .child(footer)
        .child(text(spoken).live_region(rune::LiveRegion::Polite).absolute(0.0, 0.0).size(1.0, 1.0).opacity(0.0).inert());
    crate::ui::kit::root(panel, el().key("picker-overlays").children(layer), cx)
}
