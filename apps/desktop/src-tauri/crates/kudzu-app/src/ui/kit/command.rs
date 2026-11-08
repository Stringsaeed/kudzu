//! `components/ui/command.tsx` (cmdk): a search field over a filtered, grouped list whose
//! selection moves with the arrows while the caret stays in the field.
//!
//! As cmdk: typing filters (its `command-score` fuzzy ranking, items sorted by score and groups
//! by their best item) unless `.filter(false)` hands filtering to the caller; the first visible
//! item is selected after every edit; Up and Down (and Ctrl-N/P, Ctrl-J/K) move the selection,
//! wrapping with `.wrap(true)` (`loop`); Home and End jump; Enter runs the selected item's
//! `onSelect`; the pointer selects the item under it; groups with nothing visible hide and the
//! empty state shows when nothing matches. Escape is left to the surface around it (a dialog or
//! popover closes).
//!
//! Two looks: [`CommandStyle::Shadcn`] is `command.tsx` as written (`CommandInput` in an input
//! group, `rounded-sm px-2 py-1.5` items, `data-selected:bg-muted`); [`CommandStyle::Palette`] is
//! the ⌘K menu's restyle in `command-menu.tsx` (`h-12` field over a hairline, `h-9 rounded-lg
//! px-2.5` items, `bg-foreground/[0.07]` selection, `text-[11px] text-faint` headings).
//!
//! ```ignore
//! // state: cmd: CommandState
//! Command::new("app-picker", &s.picker.cmd, |s: &mut MainUi| &mut s.picker.cmd)
//!     .placeholder("Search apps or type a bundle ID…")
//!     .wrap(true)
//!     .group(None, apps.iter().map(|a| CommandItem::new(format!("{} {}", a.name, a.bundle_id))
//!         .child(app_icon(..)).text(&a.name).checked(chosen(a))
//!         .on_select(move |s: &mut MainUi, _| toggle(s, &id))))
//!     .empty(text("No apps found."))
//!     .build(cx)
//! ```

use std::collections::HashMap;
use std::rc::Rc;

use rune::{Color, Cx, El, Key, ScrollSpec, Spring, TextEdit, Tx, col, el, icon, row, text, text_input};

use super::popover::{Handler, OpenChange};
use crate::ui::icons;
use crate::ui::theme::{radius, text as ty, weight, Theme, WeightOf};
use crate::ui::theme_for;

type Access<S> = Rc<dyn for<'a> Fn(&'a mut S) -> &'a mut CommandState>;

/// The field's text and the selected item's value, owned by the screen.
#[derive(Clone, Debug, Default)]
pub struct CommandState {
    pub search: TextEdit,
    /// The selected item's value; `None` selects the first visible item.
    pub selected: Option<String>,
}

impl CommandState {
    /// Empties the field and resets the selection (`setQuery("")` on close).
    pub fn reset(&mut self) {
        self.search.clear();
        self.selected = None;
    }
    /// What's typed.
    pub fn query(&self) -> &str {
        &self.search.text
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CommandStyle {
    #[default]
    Shadcn,
    Palette,
}

/// `CommandItem` (`cmdk` `Command.Item`).
pub struct CommandItem<S> {
    value: String,
    keywords: Vec<String>,
    children: Vec<Part>,
    disabled: bool,
    checked: Option<bool>,
    destructive: bool,
    select: Option<Handler<S>>,
}

impl<S> CommandItem<S> {
    /// `value`: what filtering matches and what identifies the item.
    pub fn new(value: impl Into<String>) -> Self {
        CommandItem { value: value.into(), keywords: Vec::new(), children: Vec::new(), disabled: false, checked: None, destructive: false, select: None }
    }
    /// `keywords`: extra words filtering matches.
    pub fn keywords(mut self, k: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords.extend(k.into_iter().map(Into::into));
        self
    }
    /// Any content, in a row (`flex items-center gap-2`).
    pub fn child(mut self, c: El) -> Self {
        self.children.push(Part::El(Box::new(c)));
        self
    }
    /// A leading 16 pt icon (`[&_svg:not([class*='size-'])]:size-4`), drawn in the style's icon ink.
    pub fn icon(mut self, i: &'static rune::Icon) -> Self {
        self.children.push(Part::Icon(i));
        self
    }
    /// The label (truncates).
    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.children.push(Part::Text(s.into()));
        self
    }
    /// `data-checked`: shadcn's trailing check (hidden when the item has a shortcut).
    pub fn checked(mut self, on: bool) -> Self {
        self.checked = Some(on);
        self
    }
    /// The ⌘K menu's `text-destructive` label (Delete).
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// `onSelect`: Enter on it, or a press.
    pub fn on_select(mut self, f: impl Fn(&mut S, &mut Tx) + 'static) -> Self {
        self.select = Some(Rc::new(f));
        self
    }
}

enum Part {
    Icon(&'static rune::Icon),
    Text(String),
    El(Box<El>),
}

struct Group<S> {
    key: String,
    heading: Option<String>,
    items: Vec<CommandItem<S>>,
}

pub struct Command<S> {
    key: String,
    state: CommandState,
    access: Access<S>,
    groups: Vec<Group<S>>,
    style: CommandStyle,
    placeholder: String,
    filter: bool,
    wrap: bool,
    empty: Option<El>,
    trailing: Option<El>,
    max_h: f32,
    in_dialog: bool,
    change: Option<OpenChange<S>>,
}

impl<S: 'static> Command<S> {
    pub fn new(key: impl Into<String>, state: &CommandState, access: impl for<'a> Fn(&'a mut S) -> &'a mut CommandState + 'static) -> Self {
        Command {
            key: key.into(),
            state: state.clone(),
            access: Rc::new(access),
            groups: Vec::new(),
            style: CommandStyle::Shadcn,
            placeholder: String::new(),
            filter: true,
            wrap: false,
            empty: None,
            trailing: None,
            max_h: 288.0,
            in_dialog: false,
            change: None,
        }
    }
    pub fn style(mut self, s: CommandStyle) -> Self {
        self.style = s;
        if s == CommandStyle::Palette {
            self.max_h = 440.0;
        }
        self
    }
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// `shouldFilter={false}`: show every item given (the caller filters).
    pub fn filter(mut self, on: bool) -> Self {
        self.filter = on;
        self
    }
    /// `loop`: the arrows wrap at the ends.
    pub fn wrap(mut self, on: bool) -> Self {
        self.wrap = on;
        self
    }
    /// `CommandList` `max-h-*` (288 for `max-h-72`; the palette's `min(440px, 60vh)` is 440,
    /// capped by the window).
    pub fn max_height(mut self, h: f32) -> Self {
        self.max_h = h;
        self
    }
    /// Inside a dialog (`in-data-[slot=dialog-content]:rounded-lg!` items).
    pub fn in_dialog(mut self) -> Self {
        self.in_dialog = true;
        self
    }
    /// After the field (the palette's `<Shortcut combo="escape" />`).
    pub fn trailing(mut self, e: El) -> Self {
        self.trailing = Some(e);
        self
    }
    /// `CommandEmpty`.
    pub fn empty(mut self, e: El) -> Self {
        self.empty = Some(e);
        self
    }
    /// `CommandGroup heading` with its items (`None`: no heading).
    pub fn group(mut self, heading: Option<&str>, items: impl IntoIterator<Item = CommandItem<S>>) -> Self {
        let ordinal = self.groups.iter().filter(|g| g.heading.as_deref() == heading).count();
        let key = format!("group-{heading:?}-{ordinal}");
        self.groups.push(Group { key, heading: heading.map(str::to_string), items: items.into_iter().collect() });
        self
    }
    /// Escape in the field closes the surface around it (for a non-modal host).
    pub fn on_escape(mut self, f: impl Fn(&mut S, bool, &mut Tx) + 'static) -> Self {
        self.change = Some(Rc::new(f));
        self
    }

    pub fn build(self, cx: &mut Cx<S>) -> El {
        let th = theme_for(cx);
        let t = &th.t;
        let palette = self.style == CommandStyle::Palette;
        let query = self.state.search.text.trim().to_string();

        // Filter and rank as cmdk does.
        let mut groups: Vec<(Group<S>, f32)> = Vec::new();
        for mut g in self.groups {
            if self.filter && !query.is_empty() {
                let mut scored: Vec<(CommandItem<S>, f32)> =
                    g.items.into_iter().map(|i| { let s = command_score(&i.value, &query, &i.keywords); (i, s) }).filter(|(_, s)| *s > 0.0).collect();
                scored.sort_by(|a, b| b.1.total_cmp(&a.1));
                let best = scored.first().map_or(0.0, |x| x.1);
                g.items = scored.into_iter().map(|x| x.0).collect();
                groups.push((g, best));
            } else {
                groups.push((g, 0.0));
            }
        }
        if self.filter && !query.is_empty() {
            groups.sort_by(|a, b| b.1.total_cmp(&a.1));
        }
        let visible: Vec<String> = groups.iter().flat_map(|(g, _)| g.items.iter().filter(|i| !i.disabled).map(|i| i.value.clone())).collect();
        let selected = self.state.selected.clone().filter(|v| visible.contains(v)).or_else(|| visible.first().cloned());
        let handlers: HashMap<String, Handler<S>> =
            groups.iter().flat_map(|(g, _)| g.items.iter().filter_map(|i| i.select.clone().map(|f| (i.value.clone(), f)))).collect();
        let item_key = {
            let k = self.key.clone();
            move |v: &str| format!("{k}-item-{}", v.replace('/', "∕"))
        };

        // The field.
        let (edit_access, key_access, change_access) = (self.access.clone(), self.access.clone(), self.access.clone());
        let (visible_k, handlers_k, wrap, item_key_k, esc) = (Rc::new(visible), Rc::new(handlers), self.wrap, item_key.clone(), self.change.clone());
        let selected_k = selected.clone();
        let field_font = if palette { ty::MD } else { ty::SM };
        let field = text_input(&self.state.search, field_font)
            .single_line()
            .key(format!("{}-input", self.key))
            .label(if self.placeholder.is_empty() { "Search".to_string() } else { self.placeholder.clone() })
            .placeholder(self.placeholder.clone(), if palette { t.faint } else { t.muted_foreground })
            .caret(t.foreground, Theme::tint(t.primary, 0.25))
            .color(t.foreground)
            .grow(1.0)
            .shrink()
            .on_edit(move |s: &mut S| &mut edit_access(s).search)
            .on_input_change(move |s: &mut S, _| change_access(s).selected = None)
            .on_input_key(move |s: &mut S, ev, tx| {
                let st = key_access(s);
                if st.search.preedit.is_some() {
                    return false;
                }
                let list = &visible_k;
                let pos = st.selected.as_ref().and_then(|v| list.iter().position(|x| x == v)).or(selected_k.as_ref().and_then(|v| list.iter().position(|x| x == v)));
                let ctrl = ev.mods.ctrl && !ev.mods.shift;
                let delta: Option<isize> = match &ev.key {
                    Key::Down if !ev.mods.cmd => Some(1),
                    Key::Up if !ev.mods.cmd => Some(-1),
                    Key::Text(c) if ctrl && (c == "n" || c == "j") => Some(1),
                    Key::Text(c) if ctrl && (c == "p" || c == "k") => Some(-1),
                    _ => None,
                };
                if let Some(d) = delta {
                    if list.is_empty() {
                        return true;
                    }
                    let n = list.len() as isize;
                    let next = match pos {
                        None => if d > 0 { 0 } else { n - 1 },
                        Some(p) => {
                            let q = p as isize + d;
                            if wrap { q.rem_euclid(n) } else { q.clamp(0, n - 1) }
                        }
                    } as usize;
                    st.selected = Some(list[next].clone());
                    tx.scroll_into_view(item_key_k(&list[next]));
                    return true;
                }
                match &ev.key {
                    Key::Enter => {
                        let v = pos.map(|p| list[p].clone());
                        if let Some(f) = v.and_then(|v| handlers_k.get(&v).cloned()) {
                            f(s, tx);
                        }
                        true
                    }
                    Key::Escape => match &esc {
                        Some(f) => {
                            f(s, false, tx);
                            true
                        }
                        None => false,
                    },
                    _ => false,
                }
            });

        let input_row = if palette {
            // `flex items-center gap-3 border-b border-hairline px-4`, the field `h-12 text-[15px]`.
            let mut r = row()
                .key(format!("{}-field", self.key))
                .h(48.0)
                .px(16.0)
                .gap(12.0)
                .center_y()
                .border_sides([0.0, 0.0, 1.0, 0.0], t.hairline)
                .child(icon(&icons::SEARCH, 16.0).color(t.muted_foreground).a11y_hidden())
                .child(field);
            if let Some(tr) = self.trailing {
                r = r.child(tr);
            }
            r
        } else {
            // `p-1 pb-0` around an InputGroup: `h-8 rounded-lg border-input/30 bg-input/30`, the
            // search addon `size-4 opacity-50` at `pl-2`.
            let mut group = row()
                .key(format!("{}-field", self.key))
                .h(32.0)
                .radius(radius::LG)
                .border(1.0, Theme::tint(t.input, 0.3))
                .fill(Theme::tint(t.input, 0.3))
                .pl(8.0)
                .pr(8.0)
                .gap(8.0)
                .center_y()
                .role("textbox")
                .child(icon(&icons::SEARCH, 16.0).color(t.foreground).opacity(0.5).a11y_hidden())
                .child(field);
            if let Some(tr) = self.trailing {
                group = group.child(tr);
            }
            el().padding(4.0).pb(0.0).child(group.w_full())
        };

        // The list.
        let mut list = col().key(format!("{}-list", self.key)).role("listbox").items_stretch();
        list = if palette { list.px(8.0).pb(8.0) } else { list };
        let mut any = false;
        for (g, _) in groups {
            if g.items.is_empty() {
                continue;
            }
            any = true;
            let mut group = col().key(format!("{}-{}", self.key, g.key)).items_stretch();
            group = if palette { group } else { group.padding(4.0) };
            if let Some(h) = g.heading {
                group = group.child((if palette {
                    row().pl(10.0).pr(10.0).pt(12.0).pb(4.0).child(text(h).font(ty::XS2.weight_of(weight::MEDIUM)).color(t.faint))
                } else {
                    row().px(8.0).py(6.0).child(text(h).font(ty::XS.weight_of(weight::MEDIUM)).color(t.muted_foreground))
                }).key("heading"));
            }
            for item in g.items {
                let on = selected.as_deref() == Some(item.value.as_str()) && !item.disabled;
                group = group.child(item_row(th, &item_key(&item.value), item, on, palette, self.in_dialog, &self.access));
            }
            list = list.child(group);
        }
        if !any {
            let e = self.empty.unwrap_or_else(|| text("No results found."));
            list = list.child((if palette {
                row().py(40.0).justify_center().child(e)
            } else {
                row().py(24.0).justify_center().child(e)
            }).key("empty"));
        }
        let win_h = super::popover::window_size(cx).1;
        let max_h = if palette { self.max_h.min(win_h * 0.6) } else { self.max_h };
        let scroller = col()
            .key(format!("{}-scroll", self.key))
            .w_full()
            .max_h(max_h)
            .min_h(0.0)
            .shrink()
            .scroll(ScrollSpec { stick_bottom: false })
            .scroll_margin(4.0)
            .child(list);
        let root = col().key(self.key.clone()).w_full().min_h(0.0).items_stretch().child(input_row).child(scroller);
        if palette { root } else { root.padding(4.0).pt(0.0) }
    }
}

fn item_row<S: 'static>(th: &Theme, key: &str, item: CommandItem<S>, on: bool, palette: bool, in_dialog: bool, access: &Access<S>) -> El {
    let t = &th.t;
    let (fill, ink, icon_ink) = if palette {
        (if on { Theme::tint(t.foreground, 0.07) } else { Color::TRANSPARENT }, if on { t.foreground } else { Theme::tint(t.foreground, 0.9) }, t.muted_foreground)
    } else {
        (if on { t.muted } else { Color::TRANSPARENT }, t.foreground, t.foreground)
    };
    let ink = if item.destructive { t.destructive } else { ink };
    let mut r = row()
        .key(key.to_string())
        .role("option")
        .label(item.value.clone())
        .center_y()
        .fill(fill)
        .motion(Spring::NONE);
    r = if palette {
        r.h(36.0).px(10.0).gap(10.0).radius(radius::LG)
    } else {
        r.min_h(32.0).px(8.0).py(6.0).gap(8.0).radius(if in_dialog { radius::LG } else { radius::SM })
    };
    let font = if palette { ty::BASE } else { ty::SM };
    for (slot, c) in item.children.into_iter().enumerate() {
        r = r.child(match c {
            Part::Icon(i) => icon(i, 16.0).key(format!("part-{slot}-icon")).color(if item.destructive && !palette { ink } else { icon_ink }).a11y_hidden(),
            Part::Text(s) => text(s).key(format!("part-{slot}-text")).font(font).color(ink).truncate().shrink().motion(Spring::NONE).a11y_hidden(),
            Part::El(e) => *e,
        });
    }
    if let Some(on) = item.checked {
        r = r.child(el().key("indicator").grow(1.0).justify_end().child(icon(&icons::CHECK, 16.0).color(t.foreground).opacity(if on { 1.0 } else { 0.0 })).a11y_hidden());
    }
    if item.disabled {
        return r.layer_opacity(0.5).disabled(true);
    }
    let value = item.value.clone();
    let hov = access.clone();
    r = r.on_hover_change(move |s: &mut S, inside, _| {
        if inside {
            hov(s).selected = Some(value.clone());
        }
    });
    if let Some(f) = item.select {
        r = r.on_press_tx(move |s: &mut S, tx| f(s, tx));
    }
    r
}

/// cmdk's `command-score`: 0 when `abbreviation` doesn't match `string` (plus its aliases),
/// otherwise higher for tighter, earlier, word-start matches.
pub fn command_score(string: &str, abbreviation: &str, aliases: &[String]) -> f32 {
    let full = if aliases.is_empty() { string.to_string() } else { format!("{string} {}", aliases.join(" ")) };
    let s: Vec<char> = full.chars().collect();
    let a: Vec<char> = abbreviation.chars().collect();
    let fmt = |v: &[char]| -> Vec<char> { v.iter().flat_map(|c| c.to_lowercase()).map(|c| if c.is_whitespace() || c == '-' { ' ' } else { c }).collect() };
    let ls = fmt(&s);
    let la = fmt(&a);
    if ls.len() != s.len() || la.len() != a.len() {
        // Lowercasing changed lengths (rare scripts): fall back to substring matching.
        return if full.to_lowercase().contains(&abbreviation.to_lowercase()) { 0.5 } else { 0.0 };
    }
    let mut memo = HashMap::new();
    score_inner(&s, &a, &ls, &la, 0, 0, &mut memo)
}

const SCORE_CONTINUE_MATCH: f32 = 1.0;
const SCORE_SPACE_WORD_JUMP: f32 = 0.9;
const SCORE_NON_SPACE_WORD_JUMP: f32 = 0.8;
const SCORE_CHARACTER_JUMP: f32 = 0.17;
const SCORE_TRANSPOSITION: f32 = 0.1;
const PENALTY_SKIPPED: f32 = 0.999;
const PENALTY_CASE_MISMATCH: f32 = 0.9999;
const PENALTY_NOT_COMPLETE: f32 = 0.99;

fn is_gap(c: char) -> bool {
    matches!(c, '\\' | '/' | '_' | '+' | '.' | '#' | '"' | '@' | '[' | '(' | '{' | '&')
}
fn is_space(c: char) -> bool {
    c.is_whitespace() || c == '-'
}

fn score_inner(s: &[char], a: &[char], ls: &[char], la: &[char], si: usize, ai: usize, memo: &mut HashMap<(usize, usize), f32>) -> f32 {
    if ai == a.len() {
        return if si == s.len() { SCORE_CONTINUE_MATCH } else { PENALTY_NOT_COMPLETE };
    }
    if let Some(&v) = memo.get(&(si, ai)) {
        return v;
    }
    let ac = la[ai];
    let find = |from: usize| (from..ls.len()).find(|&i| ls[i] == ac);
    let mut index = find(si);
    let mut high = 0.0f32;
    while let Some(i) = index {
        let mut score = score_inner(s, a, ls, la, i + 1, ai + 1, memo);
        if score > high {
            if i == si {
                score *= SCORE_CONTINUE_MATCH;
            } else if is_gap(s[i - 1]) {
                score *= SCORE_NON_SPACE_WORD_JUMP;
                let breaks = s[si..i.saturating_sub(1).max(si)].iter().filter(|c| is_gap(**c)).count();
                if breaks > 0 && si > 0 {
                    score *= PENALTY_SKIPPED.powi(breaks as i32);
                }
            } else if is_space(s[i - 1]) {
                score *= SCORE_SPACE_WORD_JUMP;
                let breaks = s[si..i.saturating_sub(1).max(si)].iter().filter(|c| is_space(**c)).count();
                if breaks > 0 && si > 0 {
                    score *= PENALTY_SKIPPED.powi(breaks as i32);
                }
            } else {
                score *= SCORE_CHARACTER_JUMP;
                if si > 0 {
                    score *= PENALTY_SKIPPED.powi((i - si) as i32);
                }
            }
            if s[i] != a[ai] {
                score *= PENALTY_CASE_MISMATCH;
            }
        }
        let next_a = la.get(ai + 1).copied();
        let prev_s = if i > 0 { Some(ls[i - 1]) } else { None };
        if (score < SCORE_TRANSPOSITION && prev_s.is_some() && prev_s == next_a) || (next_a == Some(la[ai]) && prev_s != Some(la[ai])) {
            let transposed = score_inner(s, a, ls, la, i + 1, ai + 2, memo);
            if transposed * SCORE_TRANSPOSITION > score {
                score = transposed * SCORE_TRANSPOSITION;
            }
        }
        if score > high {
            high = score;
        }
        index = find(i + 1);
    }
    memo.insert((si, ai), high);
    high
}

/// shadcn's `CommandDialog`: a [`super::dialog::Dialog`] with `top-1/3 overflow-hidden
/// rounded-xl! p-0` and no close button. Add the [`Command`] as its child.
pub fn dialog<S: 'static>(key: impl Into<String>) -> super::dialog::Dialog<S> {
    super::dialog::Dialog::new(key).top(1.0 / 3.0).padding(0.0).gap(0.0).radius(radius::XL).close_button(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_like_cmdk() {
        assert_eq!(command_score("Safari", "", &[]), PENALTY_NOT_COMPLETE);
        assert_eq!(command_score("Safari", "xyz", &[]), 0.0);
        assert!(command_score("Safari com.apple.Safari", "saf", &[]) > command_score("Messages com.apple.MobileSMS", "saf", &[]));
        // A word start beats a mid-word match.
        assert!(command_score("Visual Studio Code", "code", &[]) > command_score("Xcode", "code", &[]) * 0.5);
        assert!(command_score("New snippet", "ns", &[]) > 0.0);
        assert!(command_score("Import snippets…", "raycast", &["raycast".into()]) > 0.0);
    }
}
