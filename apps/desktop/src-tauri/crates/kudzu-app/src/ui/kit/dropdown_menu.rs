//! `components/ui/dropdown-menu.tsx` (Radix DropdownMenu), and the menu engine
//! [`super::context_menu`] shares: items, checkbox and radio items, labels, separators,
//! shortcuts and submenus, with Radix's keyboard model.
//!
//! The open menu's surface holds keyboard focus (a modal: Tab stays, a press outside closes,
//! focus returns to the trigger); the highlighted row lives in [`MenuState`], so pointer and keys
//! share it as Radix's roving focus does. Keys: Up and Down move without wrapping (Radix's
//! `loop={false}`), from nothing Down takes the first row and Up the last; Home and End; typing
//! jumps to the row whose label starts with what was typed (Radix's typeahead, 1 s);
//! Enter and Space choose; Right, Enter or Space on a sub-trigger opens its submenu on its first
//! row and Left closes it; Escape closes the whole menu. The trigger opens on a press, and from the
//! keyboard on Enter, Space or Down with the first row highlighted. Rows highlight under the
//! pointer and unhighlight when it leaves; resting 100 ms on a sub-trigger opens its submenu.
//!
//! ```ignore
//! // state: menu: MenuState
//! let trigger = DropdownMenu::new("editor-more", &s.editor.menu, |s: &mut MainUi| &mut s.editor.menu)
//!     .trigger(more_button)
//!     .align(Align::End).width(240.0)
//!     .item(MenuItem::new("Duplicate").icon(&icons::COPY).shortcut("⌘D").on_select(|s: &mut MainUi, _| ..))
//!     .sub(SubMenu::new("Move to").width(208.0).item(..))
//!     .separator()
//!     .item(MenuItem::new("Delete").icon(&icons::TRASH).destructive().on_select(..))
//!     .build(cx, &mut layer);
//! ```

use std::rc::Rc;
use std::time::{Duration, Instant};

use rune::{Align, Color, Cx, El, Icon, Key, ScrollSpec, Side, Spring, Tx, col, el, icon, row, text};

use super::popover::{self, Handler};
use crate::ui::icons;
use crate::ui::theme::{radius, text as ty, weight, Theme, Utilities, WeightOf};
use crate::ui::theme_for;

pub(crate) type Access<S> = Rc<dyn for<'a> Fn(&'a mut S) -> &'a mut MenuState>;

/// An open menu's state, owned by the screen: open, the open submenus and the highlighted row
/// at each level. Keep one per menu.
#[derive(Clone, Debug, Default)]
pub struct MenuState {
    pub open: bool,
    /// The highlighted row at each open level (root first).
    hl: Vec<Option<usize>>,
    /// The sub-trigger row whose submenu is open, at each level.
    subs: Vec<usize>,
    /// A context menu's window point.
    pub point: Option<(f32, f32)>,
    typed: String,
    typed_at: Option<Instant>,
}

impl MenuState {
    /// Opens the menu: from the keyboard the first enabled row is highlighted.
    pub fn open_with(&mut self, first: Option<usize>) {
        self.open = true;
        self.subs.clear();
        self.hl = vec![first];
        self.typed.clear();
    }
    pub fn close(&mut self) {
        self.open = false;
        self.subs.clear();
        self.hl.clear();
        self.point = None;
    }
    /// Opens the root row `row`'s submenu, its first row highlighted when `keyboard` (a sub-trigger
    /// opened with Right or Enter); galleries and tests use it to show a submenu.
    pub fn show_submenu(&mut self, row: usize, keyboard: bool) {
        self.open_sub(0, row, keyboard.then_some(0));
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    fn hl_at(&self, level: usize) -> Option<usize> {
        self.hl.get(level).copied().flatten()
    }
    fn set_hl(&mut self, level: usize, i: Option<usize>) {
        if self.hl.len() <= level {
            self.hl.resize(level + 1, None);
        }
        self.hl[level] = i;
    }
    /// The level the keys act on: the deepest with a highlighted row.
    fn active(&self) -> usize {
        (0..=self.subs.len()).rev().find(|&l| self.hl_at(l).is_some()).unwrap_or(0)
    }
    fn open_sub(&mut self, level: usize, i: usize, first: Option<usize>) {
        self.subs.truncate(level);
        self.subs.push(i);
        self.hl.truncate(level + 1);
        self.set_hl(level, Some(i));
        self.hl.push(first);
    }
    fn close_subs_from(&mut self, level: usize) {
        self.subs.truncate(level);
        self.hl.truncate(level + 1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    Plain,
    Check(bool),
    Radio(bool),
}

/// `DropdownMenuItem`, `DropdownMenuCheckboxItem` or `DropdownMenuRadioItem`.
pub struct MenuItem<S> {
    pub(crate) label: String,
    pub(crate) icon: Option<&'static Icon>,
    pub(crate) shortcut: Option<String>,
    pub(crate) kind: Kind,
    pub(crate) destructive: bool,
    pub(crate) inset: bool,
    pub(crate) disabled: bool,
    pub(crate) pad_left: Option<f32>,
    pub(crate) select: Option<Handler<S>>,
}

impl<S> MenuItem<S> {
    pub fn new(label: impl Into<String>) -> Self {
        MenuItem { label: label.into(), icon: None, shortcut: None, kind: Kind::Plain, destructive: false, inset: false, disabled: false, pad_left: None, select: None }
    }
    /// `DropdownMenuCheckboxItem checked={on}`.
    pub fn checkbox(label: impl Into<String>, on: bool) -> Self {
        MenuItem { kind: Kind::Check(on), ..MenuItem::new(label) }
    }
    /// `DropdownMenuRadioItem`, checked when it's the group's value.
    pub fn radio(label: impl Into<String>, on: bool) -> Self {
        MenuItem { kind: Kind::Radio(on), ..MenuItem::new(label) }
    }
    /// A 16 pt lucide icon before the label (`[&_svg:not([class*='size-'])]:size-4`).
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.icon = Some(i);
        self
    }
    /// `DropdownMenuShortcut`: `ml-auto text-xs tracking-widest text-muted-foreground`.
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }
    /// `variant="destructive"`.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    /// `inset` (`pl-7`).
    pub fn inset(mut self) -> Self {
        self.inset = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// `style={{ paddingLeft }}` (indented group trees).
    pub fn pad_left(mut self, p: f32) -> Self {
        self.pad_left = Some(p);
        self
    }
    /// `onSelect`: runs after the menu closes.
    pub fn on_select(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.select = Some(Rc::new(f));
        self
    }
}

/// `DropdownMenuSub` with its `SubTrigger` and `SubContent`.
pub struct SubMenu<S> {
    pub(crate) label: String,
    pub(crate) icon: Option<&'static Icon>,
    pub(crate) inset: bool,
    pub(crate) disabled: bool,
    pub(crate) width: Option<f32>,
    pub(crate) entries: Vec<Entry<S>>,
}

impl<S> SubMenu<S> {
    pub fn new(label: impl Into<String>) -> Self {
        SubMenu { label: label.into(), icon: None, inset: false, disabled: false, width: None, entries: Vec::new() }
    }
    pub fn icon(mut self, i: &'static Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn inset(mut self) -> Self {
        self.inset = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// The sub-content's width (`w-52`); otherwise it fits its rows (`min-w-[96px]`).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn item(mut self, i: MenuItem<S>) -> Self {
        self.entries.push(Entry::Item(i));
        self
    }
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.entries.push(Entry::Label(l.into(), false));
        self
    }
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
    pub fn sub(mut self, s: SubMenu<S>) -> Self {
        self.entries.push(Entry::Sub(s));
        self
    }
}

pub enum Entry<S> {
    Item(MenuItem<S>),
    /// `DropdownMenuLabel` (text, inset).
    Label(String, bool),
    Separator,
    Sub(SubMenu<S>),
}

impl<S> Entry<S> {
    fn navigable(&self) -> bool {
        match self {
            Entry::Item(i) => !i.disabled,
            Entry::Sub(s) => !s.disabled,
            _ => false,
        }
    }
    fn text(&self) -> &str {
        match self {
            Entry::Item(i) => &i.label,
            Entry::Sub(s) => &s.label,
            Entry::Label(l, _) => l,
            Entry::Separator => "",
        }
    }
}

/// The rows at the level the open submenus `path` lead to.
fn level_entries<'a, S>(root: &'a [Entry<S>], path: &[usize]) -> &'a [Entry<S>] {
    let mut list = root;
    for &i in path {
        match list.get(i) {
            Some(Entry::Sub(s)) => list = &s.entries,
            _ => return &[],
        }
    }
    list
}

fn first_enabled<S>(list: &[Entry<S>]) -> Option<usize> {
    list.iter().position(Entry::navigable)
}

/// Which surface draws: a dropdown's (`min-w-32`, ring sub-content) or a context menu's
/// (`min-w-36`, bordered sub-content).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavor {
    Dropdown,
    Context,
}

/// Radix's menu keyboard over `entries`. True when the key was taken.
fn menu_key<S>(st: &mut MenuState, entries: &[Entry<S>], key: &Key) -> KeyOutcome<S> {
    let level = st.active();
    let path: Vec<usize> = st.subs[..level.min(st.subs.len())].to_vec();
    let list = level_entries(entries, &path);
    let nav: Vec<usize> = (0..list.len()).filter(|&i| list[i].navigable()).collect();
    let cur = st.hl_at(level);
    let pos = cur.and_then(|c| nav.iter().position(|&i| i == c));
    let moved = |st: &mut MenuState, i: usize| {
        st.set_hl(level, Some(i));
        st.close_subs_from(level);
        KeyOutcome::Taken
    };
    match key {
        Key::Down => match (pos, nav.first()) {
            (Some(p), _) => moved(st, nav[(p + 1).min(nav.len() - 1)]),
            (None, Some(&f)) => moved(st, f),
            _ => KeyOutcome::Taken,
        },
        Key::Up => match (pos, nav.last()) {
            (Some(p), _) => moved(st, nav[p.saturating_sub(1)]),
            (None, Some(&l)) => moved(st, l),
            _ => KeyOutcome::Taken,
        },
        Key::Home => nav.first().map_or(KeyOutcome::Taken, |&f| moved(st, f)),
        Key::End => nav.last().map_or(KeyOutcome::Taken, |&l| moved(st, l)),
        Key::Right => {
            if let Some(c) = cur
                && let Some(Entry::Sub(sub)) = list.get(c)
            {
                st.open_sub(level, c, first_enabled(&sub.entries));
            }
            KeyOutcome::Taken
        }
        Key::Left => {
            if level > 0 {
                st.close_subs_from(level - 1);
            }
            KeyOutcome::Taken
        }
        Key::Escape => {
            st.close();
            KeyOutcome::Closed
        }
        Key::Tab => KeyOutcome::Taken,
        Key::Enter => choose(st, list, level, cur),
        Key::Text(t) if t == " " && st.typed.is_empty() => choose(st, list, level, cur),
        Key::Text(t) if !t.trim().is_empty() || (t == " " && !st.typed.is_empty()) => {
            let now = Instant::now();
            if st.typed_at.is_none_or(|at| now.duration_since(at) > Duration::from_secs(1)) {
                st.typed.clear();
            }
            st.typed_at = Some(now);
            st.typed.push_str(&t.to_lowercase());
            let typed = st.typed.clone();
            // Radix: a repeated single character cycles through the rows starting with it.
            let first = typed.chars().next().unwrap_or(' ');
            let cycle = typed.chars().all(|c| c == first);
            let search = if cycle { first.to_string() } else { typed };
            let start = pos.map_or(0, |p| if cycle { p + 1 } else { p });
            let order = (0..nav.len()).map(|k| nav[(start + k) % nav.len().max(1)]);
            let hit = order.into_iter().find(|&i| list[i].text().to_lowercase().starts_with(&search));
            if let Some(i) = hit {
                return moved(st, i);
            }
            KeyOutcome::Taken
        }
        _ => KeyOutcome::Pass,
    }
}

fn choose<S>(st: &mut MenuState, list: &[Entry<S>], level: usize, cur: Option<usize>) -> KeyOutcome<S> {
    match cur.and_then(|c| list.get(c).map(|e| (c, e))) {
        Some((c, Entry::Sub(sub))) => {
            st.open_sub(level, c, first_enabled(&sub.entries));
            KeyOutcome::Taken
        }
        Some((_, Entry::Item(item))) if !item.disabled => {
            st.close();
            KeyOutcome::Chose(item.select.clone())
        }
        _ => KeyOutcome::Taken,
    }
}

enum KeyOutcome<S> {
    Pass,
    Taken,
    Closed,
    Chose(Option<Handler<S>>),
}

/// The keyboard handler for an open menu's surface.
pub(crate) fn key_handler<S: 'static>(entries: Rc<Vec<Entry<S>>>, access: Access<S>) -> impl Fn(&mut S, &rune::KeyEvent, &mut Tx) -> bool {
    move |s: &mut S, ev, tx| {
        if ev.mods.cmd || ev.mods.ctrl || ev.mods.alt {
            return false;
        }
        let st = access(s);
        if !st.open {
            return false;
        }
        match menu_key(st, &entries, &ev.key) {
            KeyOutcome::Pass => false,
            KeyOutcome::Taken | KeyOutcome::Closed => true,
            KeyOutcome::Chose(f) => {
                if let Some(f) = f {
                    f(s, tx);
                }
                true
            }
        }
    }
}

/// Builds an open menu's root surface and its submenus into `layer`. `anchor` is the key the
/// root surface mounts against.
#[allow(clippy::too_many_arguments)]
pub(crate) fn mount_menu<S: 'static>(
    th: &'static Theme,
    key: &str,
    entries: Rc<Vec<Entry<S>>>,
    state: &MenuState,
    access: Access<S>,
    flavor: Flavor,
    width: Option<f32>,
    min_w: f32,
    placement: (&str, Side, Align, f32),
    return_to: Option<String>,
    layer: &mut Vec<El>,
) {
    let (anchor, side, align, gap) = placement;
    let close = access.clone();
    let mut root = surface(th, flavor, 0, width, min_w)
        .key(format!("{key}-content"))
        .role("menu")
        .focusable(true)
        .no_focus_ring()
        .on_key(key_handler(entries.clone(), access.clone()))
        .modal(true, move |s: &mut S| close(s).close());
    if let Some(r) = return_to {
        root = root.focus_return(r);
    }
    root = root.child(scroller(rows(th, key, &entries, &[], state, &access, flavor)));
    layer.push(popover::mount(root, anchor, side, align, gap));
    // Each open submenu, beside its trigger row.
    let mut path = Vec::new();
    for (level, &i) in state.subs.iter().enumerate() {
        let list = level_entries(&entries, &path);
        let Some(Entry::Sub(sub)) = list.get(i) else { break };
        path.push(i);
        let trigger_key = row_key(key, level, i);
        let mut surf = surface(th, flavor, level + 1, sub.width, 96.0).key(format!("{key}-sub{level}")).role("menu");
        surf = surf.child(scroller(rows(th, key, &entries, &path, state, &access, flavor)));
        // Radix's sub-content sits beside its trigger, its first row level with it (`p-1`).
        layer.push(popover::mount(surf, &trigger_key, Side::Right, Align::Start, 0.0).anchored_round());
    }
}

fn row_key(key: &str, level: usize, i: usize) -> String {
    format!("{key}-l{level}-r{i}")
}

/// `p-1` inside a viewport that scrolls when the window leaves less room than the rows need.
fn scroller(rows: El) -> El {
    col().w_full().shrink().min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(rows)
}

/// The surface: `rounded-lg bg-popover p-1 shadow-md ring-1 ring-foreground/10`; a context
/// menu's sub-content has `border` in place of the ring.
fn surface(th: &Theme, flavor: Flavor, level: usize, width: Option<f32>, min_w: f32) -> El {
    let mut s = if level > 0 && flavor == Flavor::Context {
        col().radius(radius::LG).fill(th.t.popover).border(1.0, th.t.border).elevate(&th.t.elev_float)
    } else {
        popover::surface(th)
    };
    s = s.min_w(min_w).min_h(0.0);
    if let Some(w) = width {
        s = s.w(w.max(min_w));
    }
    s
}

fn rows<S: 'static>(th: &Theme, key: &str, root: &Rc<Vec<Entry<S>>>, path: &[usize], st: &MenuState, access: &Access<S>, flavor: Flavor) -> El {
    let t = &th.t;
    let level = path.len();
    let list = level_entries(root, path);
    let hl = st.hl_at(level);
    let open_sub = st.subs.get(level).copied();
    let mut col_ = col().padding(4.0).items_stretch();
    for (i, entry) in list.iter().enumerate() {
        let e = match entry {
            Entry::Separator => el().key(format!("{key}-l{level}-sep{i}")).h(1.0).mt(4.0).mb(4.0).ml(-4.0).mr(-4.0).fill(t.border).a11y_hidden(),
            Entry::Label(l, inset) => row()
                .key(format!("{key}-l{level}-label{i}"))
                .pl(if *inset { 28.0 } else { 6.0 })
                .pr(6.0)
                .py(4.0)
                .child(text(l.clone()).font(ty::XS.weight_of(weight::MEDIUM)).color(t.muted_foreground)),
            Entry::Item(item) => {
                let highlighted = hl == Some(i);
                item_row(th, &row_key(key, level, i), item, highlighted, flavor)
                    .pipe(|r| wire_row(r, i, level, access, item.disabled, false, item.select.clone()))
            }
            Entry::Sub(sub) => {
                let open = open_sub == Some(i);
                let highlighted = hl == Some(i) || open;
                sub_row(th, &row_key(key, level, i), sub, highlighted, open).pipe(|r| wire_row(r, i, level, access, sub.disabled, true, None))
            }
        };
        col_ = col_.child(e);
    }
    col_
}

trait Pipe: Sized {
    fn pipe(self, f: impl FnOnce(Self) -> Self) -> Self {
        f(self)
    }
}
impl Pipe for El {}

/// Pointer behaviour for a row: highlight under the pointer, choose on press, and for a
/// sub-trigger open after a 100 ms rest. A sibling rested on for 100 ms closes an open submenu
/// (Radix's pointer grace while the pointer travels toward it).
fn wire_row<S: 'static>(r: El, i: usize, level: usize, access: &Access<S>, disabled: bool, sub: bool, select: Option<Handler<S>>) -> El {
    if disabled {
        return r;
    }
    let (a, b, c) = (access.clone(), access.clone(), access.clone());
    let mut r = r.on_hover_change(move |s: &mut S, inside, _| {
        let st = a(s);
        if inside {
            st.set_hl(level, Some(i));
            st.hl.truncate(level + 1);
            if st.subs.len() > level && st.subs[level] == i {
                st.hl.push(None);
            }
        } else if st.hl_at(level) == Some(i) && st.subs.get(level) != Some(&i) {
            st.set_hl(level, None);
        }
    });
    r = if sub {
        r.on_press(move |s: &mut S| {
            let st = b(s);
            st.open_sub(level, i, None);
        })
    } else {
        r.on_press_tx(move |s: &mut S, tx| {
            b(s).close();
            if let Some(f) = &select {
                f(s, tx);
            }
        })
    };
    // The delayed rest: open this row's submenu, or close a sibling's.
    el().w_full()
        .hover_delays(Duration::from_millis(100), Duration::ZERO)
        .on_hover_change(move |s: &mut S, inside, _| {
            if !inside {
                return;
            }
            let st = c(s);
            if sub {
                if st.subs.get(level) != Some(&i) {
                    st.open_sub(level, i, None);
                }
            } else if st.subs.len() > level {
                st.close_subs_from(level);
            }
        })
        .child(r.w_full())
}

/// The item's look: `flex items-center gap-1.5 rounded-md px-1.5 py-1 text-sm`, `focus:bg-accent
/// focus:text-accent-foreground`, destructive and disabled variants, the right-2 check.
fn item_row<S>(th: &Theme, key: &str, item: &MenuItem<S>, highlighted: bool, _flavor: Flavor) -> El {
    let t = &th.t;
    let destructive = item.destructive;
    let ink = if destructive {
        t.destructive
    } else if highlighted {
        t.accent_foreground
    } else {
        t.popover_foreground
    };
    let fill = match (highlighted, destructive) {
        (false, _) => Color::TRANSPARENT,
        (true, false) => t.accent,
        (true, true) => Theme::tint(t.destructive, if th.dark { 0.2 } else { 0.1 }),
    };
    let check = matches!(item.kind, Kind::Check(_) | Kind::Radio(_));
    let pl = item.pad_left.unwrap_or(if item.inset { 28.0 } else { 6.0 });
    let mut r = row()
        .key(key.to_string())
        .role(match item.kind {
            Kind::Plain => "menuitem",
            Kind::Check(_) => "menuitemcheckbox",
            Kind::Radio(_) => "menuitemradio",
        })
        .label(item.label.clone())
        .h(28.0)
        .pl(pl)
        .pr(if check { 32.0 } else { 6.0 })
        .gap(6.0)
        .center_y()
        .radius(radius::MD)
        .fill(fill)
        .motion(Spring::NONE);
    if let Kind::Check(on) | Kind::Radio(on) = item.kind {
        r = r.checked(on);
    }
    if let Some(g) = item.icon {
        r = r.child(icon(g, 16.0).key("icon").color(ink).motion(Spring::NONE));
    }
    r = r.child(text(item.label.clone()).key("label").font(ty::SM).color(ink).grow(1.0).shrink().truncate().motion(Spring::NONE).a11y_hidden());
    if let Some(sc) = &item.shortcut {
        r = r.shortcut_hint(sc.clone()).child(shortcut(th, sc, highlighted && !destructive).key("shortcut"));
    }
    if let Kind::Check(on) | Kind::Radio(on) = item.kind {
        r = r.child(
            el().key("indicator").pin(None, Some(6.0), Some(8.0), None).size(16.0, 16.0).center().a11y_hidden()
                .child(icon(&icons::CHECK, 16.0).color(ink).opacity(if on { 1.0 } else { 0.0 }).motion(Spring::NONE)),
        );
    }
    if item.disabled {
        r = r.layer_opacity(0.5).disabled(true);
    }
    r
}

/// `DropdownMenuShortcut`: `ml-auto text-xs tracking-widest text-muted-foreground`,
/// `group-focus:text-accent-foreground`.
pub fn shortcut(th: &Theme, s: &str, highlighted: bool) -> El {
    let t = &th.t;
    text(s.to_string())
        .font(ty::XS.tracking(1.2))
        .color(if highlighted { t.accent_foreground } else { t.muted_foreground })
        .motion(Spring::NONE)
        .a11y_hidden()
}

/// `DropdownMenuSubTrigger`: an item with a `ml-auto` chevron, `data-open:bg-accent`.
fn sub_row<S>(th: &Theme, key: &str, sub: &SubMenu<S>, highlighted: bool, open: bool) -> El {
    let t = &th.t;
    let ink = if highlighted { t.accent_foreground } else { t.popover_foreground };
    let mut r = row()
        .key(key.to_string())
        .role("menuitem")
        .label(sub.label.clone())
        .expanded(open)
        .h(28.0)
        .pl(if sub.inset { 28.0 } else { 6.0 })
        .pr(6.0)
        .gap(6.0)
        .center_y()
        .radius(radius::MD)
        .fill(if highlighted { t.accent } else { Color::TRANSPARENT })
        .motion(Spring::NONE);
    if let Some(g) = sub.icon {
        r = r.child(icon(g, 16.0).key("icon").color(ink).motion(Spring::NONE));
    }
    r = r
        .child(text(sub.label.clone()).key("label").font(ty::SM).color(ink).grow(1.0).shrink().truncate().motion(Spring::NONE).a11y_hidden())
        .child(icon(&icons::CHEVRON_RIGHT, 16.0).key("chevron").color(ink).motion(Spring::NONE).a11y_hidden());
    if sub.disabled {
        r = r.layer_opacity(0.5).disabled(true);
    }
    r
}

/// A DropdownMenu: Radix's root, trigger and content.
pub struct DropdownMenu<S> {
    key: String,
    state: MenuState,
    access: Access<S>,
    trigger: Option<El>,
    entries: Vec<Entry<S>>,
    side: Side,
    align: Align,
    side_offset: f32,
    width: Option<f32>,
}

impl<S: 'static> DropdownMenu<S> {
    pub fn new(key: impl Into<String>, state: &MenuState, access: impl for<'a> Fn(&'a mut S) -> &'a mut MenuState + 'static) -> Self {
        DropdownMenu {
            key: key.into(),
            state: state.clone(),
            access: Rc::new(access),
            trigger: None,
            entries: Vec::new(),
            side: Side::Bottom,
            align: Align::Start,
            side_offset: 4.0,
            width: None,
        }
    }
    /// `DropdownMenuTrigger asChild`: the element that opens it. Its own `on_press` is replaced.
    pub fn trigger(mut self, t: El) -> Self {
        self.trigger = Some(t);
        self
    }
    pub fn side(mut self, s: Side) -> Self {
        self.side = s;
        self
    }
    /// `align` (default `start`).
    pub fn align(mut self, a: Align) -> Self {
        self.align = a;
        self
    }
    pub fn side_offset(mut self, g: f32) -> Self {
        self.side_offset = g;
        self
    }
    /// `w-44` and the like; otherwise the trigger's width (`w-(--radix-dropdown-menu-trigger-width)`).
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn item(mut self, i: MenuItem<S>) -> Self {
        self.entries.push(Entry::Item(i));
        self
    }
    /// `DropdownMenuLabel`.
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.entries.push(Entry::Label(l.into(), false));
        self
    }
    /// `DropdownMenuLabel inset`.
    pub fn label_inset(mut self, l: impl Into<String>) -> Self {
        self.entries.push(Entry::Label(l.into(), true));
        self
    }
    pub fn separator(mut self) -> Self {
        self.entries.push(Entry::Separator);
        self
    }
    pub fn sub(mut self, s: SubMenu<S>) -> Self {
        self.entries.push(Entry::Sub(s));
        self
    }
    pub fn entries(mut self, e: impl IntoIterator<Item = Entry<S>>) -> Self {
        self.entries.extend(e);
        self
    }

    /// Returns the trigger; the open menu goes to `layer`.
    pub fn build(self, cx: &mut Cx<S>, layer: &mut Vec<El>) -> El {
        let th = theme_for(cx);
        let anchor = format!("{}-trigger", self.key);
        let open = self.state.open;
        let first = first_enabled(&self.entries);
        let (toggle, keys) = (self.access.clone(), self.access.clone());
        let trigger = self
            .trigger
            .unwrap_or_else(el)
            .expanded(open)
            .on_press(move |s: &mut S| {
                let st = toggle(s);
                if st.open { st.close() } else { st.open_with(None) }
            })
            .on_key(move |s: &mut S, ev, _| {
                let opens = matches!(&ev.key, Key::Enter | Key::Down) || ev.key == Key::Text(" ".into());
                if opens && !ev.mods.cmd && !ev.mods.alt {
                    keys(s).open_with(first);
                    return true;
                }
                false
            });
        let wrapped = el().key(anchor.clone()).child(trigger);
        if !open {
            return wrapped;
        }
        let width = self.width.or_else(|| cx.layout_size(&anchor).map(|s| s[0]));
        let entries = Rc::new(self.entries);
        mount_menu(th, &self.key, entries, &self.state, self.access, Flavor::Dropdown, width, 128.0, (&anchor, self.side, self.align, self.side_offset), Some(anchor.clone()), layer);
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct S;

    fn menu() -> Vec<Entry<S>> {
        vec![
            Entry::Label("Sort by".into(), false),
            Entry::Item(MenuItem::new("Name")),
            Entry::Item(MenuItem::new("Recently edited").disabled(true)),
            Entry::Item(MenuItem::new("Most used")),
            Entry::Separator,
            Entry::Sub(SubMenu::new("Move to").item(MenuItem::new("Work")).item(MenuItem::new("Personal"))),
        ]
    }

    fn press(st: &mut MenuState, e: &[Entry<S>], k: Key) {
        let _ = menu_key(st, e, &k);
    }

    #[test]
    fn arrows_skip_disabled_rows_and_stop_at_the_ends() {
        let e = menu();
        let mut st = MenuState::default();
        st.open_with(None);
        press(&mut st, &e, Key::Up);
        assert_eq!(st.hl_at(0), Some(5), "Up from nothing takes the last row");
        press(&mut st, &e, Key::Down);
        assert_eq!(st.hl_at(0), Some(5), "no wrap");
        press(&mut st, &e, Key::Home);
        assert_eq!(st.hl_at(0), Some(1));
        press(&mut st, &e, Key::Down);
        assert_eq!(st.hl_at(0), Some(3), "the disabled row is skipped");
        press(&mut st, &e, Key::Up);
        press(&mut st, &e, Key::Up);
        assert_eq!(st.hl_at(0), Some(1), "no wrap at the top");
    }

    #[test]
    fn right_opens_a_submenu_on_its_first_row_and_left_closes_it() {
        let e = menu();
        let mut st = MenuState::default();
        st.open_with(None);
        press(&mut st, &e, Key::End);
        press(&mut st, &e, Key::Right);
        assert_eq!(st.subs, vec![5]);
        assert_eq!(st.active(), 1);
        assert_eq!(st.hl_at(1), Some(0));
        press(&mut st, &e, Key::Down);
        assert_eq!(st.hl_at(1), Some(1));
        press(&mut st, &e, Key::Left);
        assert!(st.subs.is_empty());
        assert_eq!(st.hl_at(0), Some(5), "focus is back on the sub-trigger");
        press(&mut st, &e, Key::Escape);
        assert!(!st.open);
    }

    #[test]
    fn typing_jumps_to_a_matching_label() {
        let e = menu();
        let mut st = MenuState::default();
        st.open_with(None);
        press(&mut st, &e, Key::Text("m".into()));
        assert_eq!(st.hl_at(0), Some(3));
        press(&mut st, &e, Key::Text("m".into()));
        assert_eq!(st.hl_at(0), Some(5), "a repeated letter cycles");
    }

    #[test]
    fn enter_on_an_item_closes_and_chooses() {
        let e = menu();
        let mut st = MenuState::default();
        st.open_with(Some(1));
        assert!(matches!(menu_key(&mut st, &e, &Key::Enter), KeyOutcome::Chose(_)));
        assert!(!st.open);
    }
}
