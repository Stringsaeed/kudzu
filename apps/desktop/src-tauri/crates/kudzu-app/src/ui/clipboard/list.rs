//! `components/app/clipboard/clip-list.tsx`: the virtualized history list, rows grouped by day.
//! Selection and keys belong to the window; this draws rows and reports pointer input.
//!
//! Rows come from [`Rows`], which each window rebuilds when the history, filter or query changes
//! and keeps in its state, so the list's `virtual_rows` builder (which only sees the state) can
//! build any row by index. Only the rows in view are built, and each is a `memo`, so moving the
//! selection rebuilds the two rows whose look changed.

use std::borrow::Cow;
use std::cell::Ref;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::rc::Rc;

use kudzu_core::client::clipboard::{self as clip, ClipFilter, ClipRow, HaystackCache, RowOptions};
use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{col, el, icon, memo, rich, row, text, El, Family, GrowthSpec, Icon, Mods, Phase, ScrollSpec, Shadow, Span, TextStyle, Tx};
use uuid::Uuid;

use super::secret;
use super::tile::tile;
use crate::ui::kit::shortcut::{Shortcut, Tone};
use crate::ui::theme::{weight, Theme, Utilities};

/// `density`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Density {
    Comfortable,
    Compact,
}

impl Density {
    /// `SIZES`: row, header and tile heights.
    pub fn sizes(self) -> (f32, f32, f32) {
        match self {
            Density::Comfortable => (58.0, 30.0, 36.0),
            Density::Compact => (50.0, 28.0, 32.0),
        }
    }
}

/// One list row, owned (entries by index into the window's history at `Rows::rev`).
#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Header { key: String, label: String, count: usize },
    Entry { entry: usize, index: usize, dated: bool },
}

/// What a list shows: `searchClips` + `buildRows` over the history, cached by its inputs.
#[derive(Default)]
pub struct Rows {
    pub key: Option<(u64, ClipFilter, String, bool, bool, i64)>,
    pub rows: Vec<Row>,
    /// Selectable entries in row order (`order`), as indices into the history.
    pub order: Vec<usize>,
    pub terms: Vec<String>,
    /// Each selectable entry's place in `order`, by id.
    pub pos: HashMap<Uuid, usize>,
    pub haystacks: HaystackCache,
}

/// The local day number, so day sections refresh at midnight.
fn day() -> i64 {
    chrono::Local::now().date_naive().num_days_from_ce() as i64
}

use chrono::Datelike;

/// The rows for these inputs, rebuilt only when one changed (the window's `RefCell` is borrowed
/// mutably only then, so rows can be read while others are held).
pub fn cached<'a>(cell: &'a std::cell::RefCell<Rows>, entries: &[ClipEntry], rev: u64, filter: ClipFilter, query: &str, pinned_first: bool, sections: bool) -> Ref<'a, Rows> {
    let stale = cell.borrow().key.as_ref().is_none_or(|k| k.0 != rev || k.1 != filter || k.2 != query || k.3 != pinned_first || k.4 != sections || k.5 != day());
    if stale {
        cell.borrow_mut().refresh(entries, rev, filter, query, pinned_first, sections);
    }
    cell.borrow()
}

impl Rows {
    /// Rebuilds for these inputs. `pinned_first` and `sections` are `buildRows`' options.
    pub fn refresh(&mut self, entries: &[ClipEntry], rev: u64, filter: ClipFilter, query: &str, pinned_first: bool, sections: bool) {
        let key = (rev, filter, query.to_string(), pinned_first, sections, day());
        self.key = Some(key);
        self.terms = clip::parse_query(query).terms;
        let results = clip::search_clips_cached(entries, filter, query, &mut self.haystacks);
        let index: HashMap<Uuid, usize> = entries.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        let (rows, order) = clip::build_rows(&results, RowOptions { pinned_first, sections }, super::now_ms(), &chrono::Local);
        self.rows = rows
            .into_iter()
            .map(|r| match r {
                ClipRow::Header { key, label, count } => Row::Header { key, label, count },
                ClipRow::Entry { entry, index: i, dated, .. } => Row::Entry { entry: index[&entry.id], index: i, dated },
            })
            .collect();
        self.order = order.iter().map(|e| index[&e.id]).collect();
        self.pos = order.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
    }

    /// The `n`-th selectable entry's id.
    pub fn id_at(&self, entries: &[ClipEntry], n: usize) -> Option<Uuid> {
        self.order.get(n).and_then(|&i| entries.get(i)).map(|e| e.id)
    }

    /// Where an entry sits among the rows.
    pub fn row_of(&self, entries: &[ClipEntry], id: Uuid) -> Option<usize> {
        self.rows.iter().position(|r| matches!(r, Row::Entry { entry, .. } if entries.get(*entry).is_some_and(|e| e.id == id)))
    }
}

/// How the rows look right now (selection, focus), read from the window's state.
#[derive(Clone, Default)]
pub struct Look {
    pub active: Option<Uuid>,
    pub selection: Rc<HashSet<Uuid>>,
    pub focused: bool,
    pub show_numbers: bool,
    pub show_secrets: bool,
}

type RowsOf<S> = Rc<dyn for<'a> Fn(&'a S) -> Ref<'a, Rows>>;
type EntriesOf<S> = Rc<dyn for<'a> Fn(&'a S) -> &'a [ClipEntry]>;

/// A list's configuration.
pub struct ClipList<S> {
    pub key: &'static str,
    pub rows: RowsOf<S>,
    pub entries: EntriesOf<S>,
    pub look: Rc<dyn Fn(&S) -> Look>,
    pub density: Density,
    /// ⌘1–9 next to the first nine entries (picker).
    pub numbered: bool,
    pub label: String,
    /// `onPointer`: a primary press on a row, with its modifiers.
    pub on_pointer: OnPointer<S>,
    /// `onPick`: a double click.
    pub on_pick: OnPick<S>,
}

pub type OnPointer<S> = Rc<dyn Fn(&mut S, Uuid, Mods, &mut Tx)>;
pub type OnPick<S> = Rc<dyn Fn(&mut S, Uuid, &mut Tx)>;

/// The key a row's element has.
pub fn row_key(list: &str, entry: &ClipEntry) -> String {
    format!("{list}-{}", entry.id)
}

impl<S: 'static> ClipList<S> {
    /// The scrolling list, or `empty` when there are no rows.
    pub fn build(self, th: &'static Theme, s: &S, empty: El) -> El {
        let count = (self.rows)(s).rows.len();
        let (row_h, header_h, _) = self.density.sizes();
        let scroll = col().key(self.key).role("listbox").label(self.label.clone()).grow(1.0).min_h(0.0).w_full().scroll(ScrollSpec { stick_bottom: false });
        if count == 0 {
            return scroll.child(col().key(format!("{}-empty", self.key)).w_full().child(empty));
        }
        let rows_of = self.rows.clone();
        let entries_of = self.entries.clone();
        let key_list = self.key;
        let this = Rc::new(self);
        let builder = this.clone();
        let stack = col()
            .key(format!("{key_list}-rows"))
            .w_full()
            // `paddingStart: 2, paddingEnd: 8`.
            .pt(2.0)
            .pb(8.0)
            .growth(GrowthSpec::default())
            // Rows a filter, search or new copy moves jump to their place, as the web re-renders
            // them, instead of sliding (colours still ease).
            .instant_changes()
            .virtual_rows(
                count,
                row_h,
                move |s: &S, i| {
                    let rows = rows_of(s);
                    Cow::Owned(match rows.rows.get(i) {
                        Some(Row::Header { key, .. }) => format!("{key_list}-{key}"),
                        Some(Row::Entry { entry, .. }) => entries_of(s).get(*entry).map_or_else(|| format!("{key_list}-gone-{i}"), |e| row_key(key_list, e)),
                        None => format!("{key_list}-none-{i}"),
                    })
                },
                move |s: &S, i| builder.row(th, s, i, header_h),
            );
        scroll.child(stack)
    }

    fn row(&self, th: &'static Theme, s: &S, i: usize, header_h: f32) -> El {
        let rows = (self.rows)(s);
        let entries = (self.entries)(s);
        let t = &th.t;
        match rows.rows.get(i) {
            Some(Row::Header { key, label, count }) => {
                let k = format!("{}-{key}", self.key);
                memo(k.clone(), (label, count, th.dark), || {
                    // `flex h-full items-end gap-1.5 px-4 pb-1.5 text-[11px] font-medium text-faint`.
                    row()
                        .key(k.clone())
                        .w_full()
                        .h(header_h)
                        .items_end()
                        .gap(6.0)
                        .px(16.0)
                        .pb(6.0)
                        .a11y_hidden()
                        .child(text(label.clone()).font(TextStyle::new(11.0, weight::MEDIUM).lh(20.0)).color(t.faint))
                        .child(text(count.to_string()).font(TextStyle::new(11.0, weight::MEDIUM).lh(20.0).features("'tnum' 1")).color(t.faint).opacity(0.7))
                })
            }
            Some(Row::Entry { entry, index, dated }) => {
                let Some(e) = entries.get(*entry) else { return el() };
                let look = (self.look)(s);
                let active = look.active == Some(e.id);
                let selected = look.selection.len() > 1 && look.selection.contains(&e.id);
                let number = (self.numbered && *index < 9).then_some(index + 1);
                let concealed = clip::is_secret(e) && !look.show_secrets;
                let k = row_key(self.key, e);
                let version = (
                    (&e.hash, e.pinned, e.copy_count, e.copied_at, e.secret_override),
                    active,
                    selected,
                    look.focused,
                    number,
                    look.show_numbers,
                    *dated,
                    concealed,
                    &rows.terms,
                    th.dark,
                );
                memo(k.clone(), version, || self.entry_row(th, e, k.clone(), active, selected, look.focused, number, look.show_numbers, *dated, concealed, &rows.terms))
            }
            None => el(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn entry_row(&self, th: &Theme, e: &ClipEntry, key: String, active: bool, selected: bool, focused: bool, number: Option<usize>, show_numbers: bool, dated: bool, concealed: bool, terms: &[String]) -> El {
        let t = &th.t;
        let (row_h, _, tile_size) = self.density.sizes();
        let d = clip::describe(e);
        let mono = matches!(e.kind, ClipKind::Code | ClipKind::Color);
        let title_style = if mono { TextStyle::new(12.5, weight::MEDIUM).family(Family::Mono).lh(18.0) } else { TextStyle::new(13.0, weight::MEDIUM).lh(18.0) };
        let title_text: String = html_ws(&js_slice(&d.title, 200));
        let title = if title_text.is_empty() {
            text("Empty").font(TextStyle { italic: true, ..title_style }).color(t.muted_foreground).truncate()
        } else {
            marked(&title_text, terms, t).font(title_style).color(t.foreground).truncate()
        };
        let title = secret::row_secret(concealed, text(secret::dots(js_len(&d.title))).font(title_style).color(t.foreground).truncate(), title.w_full());
        let desc_style = if e.kind == ClipKind::Link { TextStyle::new(11.0, weight::NORMAL).family(Family::Mono).lh(15.71) } else { TextStyle::new(11.5, weight::NORMAL).lh(16.0) };
        let desc_color = Theme::tint(t.muted_foreground, 0.85);
        let subtitle = marked(&html_ws(&js_slice(&d.subtitle, 200)), terms, t).font(desc_style).color(desc_color).truncate().w_full();
        let stand = text(format!("{} · hover to show", clip::secret_reason(e))).font(desc_style).color(desc_color).truncate();
        let subtitle = secret::row_secret(concealed, stand, subtitle);

        let now = super::now_ms();
        let time = clip::clip_time(&e.copied_at, now, dated, &chrono::Local);
        let mut meta = row().h(14.0).gap(4.0).center_y();
        if e.copy_count > 1 {
            meta = meta.child(
                text(format!("×{}", e.copy_count)).key("count").font(TextStyle::new(10.5, weight::NORMAL).lh(10.5).features("'tnum' 1")).color(t.faint).help(format!("Copied {} times", e.copy_count)),
            );
        }
        if e.pinned {
            meta = meta.child(pin_glyph(12.0, Theme::tint(t.iris_foreground, 0.8)).key("pin"));
        }
        let actions = col()
            .items_end()
            .gap(4.0)
            .self_center()
            .child(text(time).font(TextStyle::new(11.0, weight::NORMAL).lh(11.0).features("'tnum' 1")).color(t.faint))
            .child(meta);

        let mut item = row()
            .key(format!("{key}-item"))
            .grow(1.0)
            .min_w(0.0)
            .ml(6.0)
            .mr(6.0)
            .h(row_h - 2.0)
            .gap(12.0)
            .pl(11.0)
            .pr(11.0)
            .center_y()
            .radius(10.0)
            .hover_group()
            .motion(rune::Spring::new(0.075, 0.0))
            .child(tile(th, e, active && focused, tile_size, true))
            .child(col().key("content").grow(1.0).min_w(0.0).gap(2.0).child(title.key("title")).child(subtitle.key("subtitle")))
            .child(actions.key("actions"));
        if let Some(n) = number {
            item = item.child(Shortcut::new(format!("mod+{n}")).tone(if show_numbers { Tone::Accent } else { Tone::Ghost }).build(th).key("shortcut"));
        }
        if active && focused {
            item = item.fill(t.iris_soft).box_shadow(Shadow { spread: 1.0, color: Theme::tint(t.iris, 0.3), inset: true, ..Shadow::default() });
        } else if active {
            item = item.fill(Theme::tint(t.foreground, 0.07)).elevate(&t.elev_selected);
        } else if selected {
            item = item.fill(Theme::tint(t.foreground, 0.06)).elevate(&t.elev_selected).hover(Theme::tint(t.foreground, 0.035));
        } else {
            item = item.hover(Theme::tint(t.foreground, 0.035));
        }
        let id = e.id;
        let (pointer, pick) = (self.on_pointer.clone(), self.on_pick.clone());
        let spoken = clip::spoken_label(e, concealed, now, &chrono::Local);
        let item = item
            .role("option")
            .label(spoken)
            .checked(active || selected)
            .on_drag(move |s: &mut S, drag, tx: &mut Tx| {
                if drag.phase == Phase::Down {
                    pointer(s, id, tx.mods(), tx);
                } else {
                    tx.live_only();
                }
            })
            .on_double_press_tx(move |s: &mut S, tx| pick(s, id, tx));
        // `h-full py-[1px]` around the item.
        row().key(key).w_full().h(row_h).pt(1.0).pb(1.0).child(item)
    }
}

/// The pinned glyph: lucide `pin`, filled, turned 45° (`-rotate-45 fill-current`).
pub fn pin_glyph(size: f32, color: rune::Color) -> El {
    icon(&PIN_FILLED, size).color(color).rotate(-std::f32::consts::FRAC_PI_4).a11y_hidden()
}

/// lucide `pin` with `fill-current`.
pub static PIN_FILLED: Icon = Icon {
    name: "kudzu-pin-filled",
    view: 24.0,
    body: r#"<g fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 17v5"/><path d="M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z"/></g>"#,
};

/// `text.slice(0, n)` in UTF-16 units, as the web cut it.
pub fn js_slice(s: &str, n: usize) -> String {
    kudzu_core::client::js::slice16(s, 0, n)
}

/// HTML's whitespace collapsing for one-line text (`white-space: normal`): runs of spaces,
/// tabs and newlines show as one space.
pub fn html_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}') {
            space = true;
        } else {
            if space && !out.is_empty() {
                out.push(' ');
            }
            space = false;
            out.push(c);
        }
    }
    if space && !out.is_empty() {
        out.push(' ');
    }
    out
}

/// `text.length`.
pub fn js_len(s: &str) -> usize {
    kudzu_core::client::js::len16(s)
}

/// UTF-16 offsets (from `matchRanges`) as byte offsets into `s`.
pub fn byte_ranges(s: &str, ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut map = Vec::with_capacity(s.len() + 1);
    for (b, ch) in s.char_indices() {
        for _ in 0..ch.len_utf16() {
            map.push(b);
        }
    }
    map.push(s.len());
    ranges.iter().map(|&(a, b)| (map[a.min(map.len() - 1)], map[b.min(map.len() - 1)])).filter(|(a, b)| a < b).collect()
}

/// `Marked`: text with search matches in the accent colour on a soft accent background.
pub fn marked(s: &str, terms: &[String], t: &crate::ui::theme::Tokens) -> El {
    let ranges = clip::match_ranges(s, terms);
    if ranges.is_empty() {
        return text(s.to_string());
    }
    let spans = byte_ranges(s, &ranges).into_iter().map(|(a, b)| Span::new(a, b).color(t.iris_foreground).bg(t.iris_soft, 0.0)).collect();
    rich(s.to_string(), spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_text_collapses_whitespace_as_html_does() {
        assert_eq!(html_ws("Image  2880 × 1800"), "Image 2880 × 1800");
        assert_eq!(html_ws("  a\n\tb  "), "a b ");
    }

    #[test]
    fn utf16_match_ranges_map_to_bytes() {
        // "é" is one UTF-16 unit and two bytes; "🚀" is two units and four bytes.
        let s = "é🚀ab";
        assert_eq!(byte_ranges(s, &[(3, 5)]), vec![(6, 8)]);
        assert_eq!(byte_ranges(s, &[(0, 1)]), vec![(0, 2)]);
    }

    #[test]
    fn rows_put_pinned_first_and_index_the_order() {
        use chrono::{Duration, Utc};
        let now = Utc::now();
        let mk = |n: u128, mins: i64, pinned: bool| ClipEntry {
            id: Uuid::from_u128(n),
            kind: ClipKind::Text,
            text: Some(format!("entry {n}")),
            text_len: 7,
            line_count: 1,
            files: vec![],
            image: None,
            color: None,
            rich: false,
            types: vec![],
            size: 7,
            source: None,
            hash: format!("{n}"),
            first_copied_at: now - Duration::minutes(mins),
            copied_at: now - Duration::minutes(mins),
            copy_count: 1,
            timings: vec![],
            last_used_at: None,
            use_count: 0,
            pinned,
            secret: None,
            secret_override: None,
        };
        let entries = vec![mk(1, 1, false), mk(2, 2, true), mk(3, 3, false)];
        let mut rows = Rows::default();
        rows.refresh(&entries, 1, ClipFilter::All, "", true, true);
        assert_eq!(rows.order, vec![1, 0, 2]);
        assert_eq!(rows.pos[&Uuid::from_u128(2)], 0);
        assert!(matches!(&rows.rows[0], Row::Header { label, count: 1, .. } if label == "Pinned"));
        rows.refresh(&entries, 1, ClipFilter::All, "entry 3", false, true);
        assert_eq!(rows.order, vec![2]);
        assert_eq!(rows.terms, vec!["entry".to_string(), "3".to_string()]);
    }
}
