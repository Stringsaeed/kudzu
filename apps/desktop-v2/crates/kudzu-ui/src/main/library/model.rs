//! The library list's model (`snippet-list.tsx`), with no `El`: scope, sort and the 10,000 cap
//! over core's search, the row data a row draws, the multi-selection with its anchor, and the
//! rule that keeps a selection inside the list (LIB-1 to LIB-4).
//!
//! [`Indexed`] folds one library snapshot once. [`list`] runs per query, scope or sort: it ranks
//! every snippet, keeps the scope's, caps at [`CAP`] and re-sorts, which orders exactly as the
//! Tauri app's `search(buildIndex(scoped), filter, 10_000)` because rank order is total and
//! library order breaks its ties in both.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use kudzu_core::library::groups::{descendant_ids, is_group_enabled};
use kudzu_core::library::usage::{usage_by_id, SnippetUsage};
use kudzu_core::search::{self, BaseKey, SearchIndex};
use kudzu_core::template::summary::{plain_summary, LIST_UNITS, SEARCH_UNITS};
use kudzu_core::{KudzuLibrary, SnippetRecord};
use uuid::Uuid;

use crate::main::state::LibraryFilter;

/// `search(index, filter, 10_000)`.
pub const CAP: usize = 10_000;
/// Page Up and Page Down move this many rows.
pub const PAGE: isize = 8;

/// The sort menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sort {
    #[default]
    BestMatch,
    Name,
    Edited,
    Used,
}

impl Sort {
    pub const ALL: [Sort; 4] = [Sort::BestMatch, Sort::Name, Sort::Edited, Sort::Used];

    pub const fn label(self) -> &'static str {
        match self {
            Sort::BestMatch => "Best match",
            Sort::Name => "Name",
            Sort::Edited => "Recently edited",
            Sort::Used => "Most used",
        }
    }
}

/// One library snapshot, folded for listing: the search index, usage summed across devices,
/// each snippet's base-sensitivity name key, which groups are off through themselves or an
/// ancestor, and the row summaries drawn so far.
pub struct Indexed {
    pub library: Arc<KudzuLibrary>,
    index: SearchIndex,
    usage: HashMap<Uuid, SnippetUsage>,
    names: Vec<BaseKey>,
    off_groups: HashSet<Uuid>,
    by_id: HashMap<Uuid, usize>,
    /// Row summaries by snippet id. They name nested snippets, so they live and die with the
    /// snapshot, as the Tauri cache is cleared when any name changes.
    summaries: RefCell<HashMap<Uuid, Rc<str>>>,
}

impl Indexed {
    pub fn new(library: Arc<KudzuLibrary>) -> Self {
        let usage = usage_by_id(&library.usage);
        let index = search::index(&library.snippets, &usage, &|content| {
            plain_summary(content, None, SEARCH_UNITS)
        });
        let names = library
            .snippets
            .iter()
            .map(|s| BaseKey::new(&s.name))
            .collect();
        let off_groups = library
            .groups
            .iter()
            .filter(|g| !is_group_enabled(&library.groups, Some(g.id)))
            .map(|g| g.id)
            .collect();
        let by_id = library
            .snippets
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();
        Self {
            library,
            index,
            usage,
            names,
            off_groups,
            by_id,
            summaries: RefCell::default(),
        }
    }

    pub fn snippet(&self, position: usize) -> &SnippetRecord {
        &self.library.snippets[position]
    }

    pub fn position(&self, id: Uuid) -> Option<usize> {
        self.by_id.get(&id).copied()
    }

    pub fn usage(&self, id: Uuid) -> SnippetUsage {
        self.usage.get(&id).copied().unwrap_or_default()
    }

    /// Off by its own flag or through a group above it (`groupOff`).
    pub fn is_off(&self, snippet: &SnippetRecord) -> bool {
        !snippet.is_enabled
            || snippet
                .group_id
                .is_some_and(|g| self.off_groups.contains(&g))
    }

    /// The 140-unit summary a row shows, with nested snippets by name.
    pub fn summary(&self, snippet: &SnippetRecord) -> Rc<str> {
        if let Some(cached) = self.summaries.borrow().get(&snippet.id) {
            return Rc::clone(cached);
        }
        let name_of = |id: &str| -> Option<&str> {
            let id = Uuid::parse_str(id).ok()?;
            self.position(id).map(|i| self.snippet(i).name.as_str())
        };
        let made: Rc<str> = plain_summary(&snippet.content, Some(&name_of), LIST_UNITS).into();
        self.summaries
            .borrow_mut()
            .insert(snippet.id, Rc::clone(&made));
        made
    }

    /// What the row for `hit` draws.
    pub fn row(&self, hit: &Hit) -> RowData {
        let s = self.snippet(hit.position);
        let untitled = s.name.is_empty();
        RowData {
            id: s.id,
            name: if untitled {
                "Untitled".to_owned()
            } else {
                s.name.clone()
            },
            untitled,
            // An unnamed snippet matches through its trigger; "Untitled" marks nothing.
            marks: if untitled {
                Vec::new()
            } else {
                search::highlight(&s.name, &hit.name_hits)
            },
            trigger: s.triggers.first().cloned(),
            more_triggers: s.triggers.len().saturating_sub(1),
            summary: self.summary(s),
            usage: self.usage(s.id).count,
            favorite: s.is_favorite,
            off: self.is_off(s),
        }
    }
}

/// One listed snippet: where it sits in the snapshot, and the UTF-16 units of its display
/// name that the query matched.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub position: usize,
    pub name_hits: Vec<usize>,
}

/// Everything one row shows. It is the row's memo revision too, so it holds every visible datum.
#[derive(Clone, Debug, PartialEq, Hash)]
pub struct RowData {
    pub id: Uuid,
    /// The name, or "Untitled".
    pub name: String,
    pub untitled: bool,
    /// Byte ranges of `name` to emphasise.
    pub marks: Vec<Range<usize>>,
    pub trigger: Option<String>,
    pub more_triggers: usize,
    pub summary: Rc<str>,
    pub usage: i64,
    pub favorite: bool,
    pub off: bool,
}

fn in_scope<'a>(
    ix: &'a Indexed,
    scope: &LibraryFilter,
) -> Box<dyn Fn(&SnippetRecord) -> bool + 'a> {
    match scope {
        LibraryFilter::All => Box::new(|_| true),
        LibraryFilter::Favorites => Box::new(|s| s.is_favorite),
        LibraryFilter::Recent => Box::new(|s| ix.usage(s.id).last_ms != 0),
        // The snippet's own flag: a group that is off shows on its rows, not in this view.
        LibraryFilter::Disabled => Box::new(|s| !s.is_enabled),
        LibraryFilter::Group(id) => {
            let ids = descendant_ids(&ix.library.groups, *id);
            Box::new(move |s| s.group_id.is_some_and(|g| ids.contains(&g)))
        }
    }
}

/// JavaScript's `String.prototype.trim()`.
fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// The rows of `scope` for `query`, in the order `sort` asks for (FEATURES LIB-2):
/// - Recent with no query, or with Best match: last use, newest first.
/// - Name, or Best match with no query: name at base sensitivity, unnamed last.
/// - Recently edited, Most used: newest edit, most uses first.
/// - Best match with a query: the ranking.
///
/// Every re-sort is stable over the ranking.
pub fn list(
    ix: &Indexed,
    scope: &LibraryFilter,
    query: &str,
    sort: Sort,
    now: DateTime<Utc>,
) -> Vec<Hit> {
    let keep = in_scope(ix, scope);
    let mut hits: Vec<Hit> = search::rank(&ix.index, query, ix.index.len(), now)
        .into_iter()
        .filter(|h| keep(ix.snippet(h.position)))
        .take(CAP)
        .map(|h| Hit {
            position: h.position,
            name_hits: h.name_hits,
        })
        .collect();
    let empty = js_trim(query).is_empty();
    let recent = matches!(scope, LibraryFilter::Recent);
    if recent && (empty || sort == Sort::BestMatch) {
        hits.sort_by_key(|h| Reverse(ix.usage(ix.snippet(h.position).id).last_ms));
    } else if sort == Sort::Name || (empty && sort == Sort::BestMatch) {
        hits.sort_by(|a, b| {
            let unnamed = |h: &Hit| ix.snippet(h.position).name.is_empty();
            unnamed(a)
                .cmp(&unnamed(b))
                .then_with(|| ix.names[a.position].cmp(&ix.names[b.position]))
        });
    } else if sort == Sort::Edited {
        hits.sort_by_key(|h| Reverse(ix.snippet(h.position).updated_at));
    } else if sort == Sort::Used {
        hits.sort_by_key(|h| Reverse(ix.usage(ix.snippet(h.position).id).count));
    }
    hits
}

/// Tauri's "keep a valid selection inside the visible list": the snippet to select given
/// the current one and the listed `ids`. A selection in the list stays. Otherwise, with a
/// non-empty list:
/// - a filter keeps a selected snippet the results leave out (the editor stays on it);
/// - All keeps a snippet that still exists;
/// - anything else selects the first row.
///
/// `filter` is the field's raw text, not trimmed, as `!filter` reads it.
pub fn keep_valid(
    cursor: Option<Uuid>,
    ids: &[Uuid],
    filter: &str,
    scope: &LibraryFilter,
    exists: impl Fn(Uuid) -> bool,
) -> Option<Uuid> {
    let Some(&first) = ids.first() else {
        return cursor;
    };
    match cursor {
        Some(id) if ids.contains(&id) => Some(id),
        Some(id) if !filter.is_empty() => Some(id),
        Some(id) if *scope == LibraryFilter::All && exists(id) => Some(id),
        _ => Some(first),
    }
}

/// How a row was clicked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClickMods {
    /// ⌘ (or ⌃): toggle the row in the multi-selection.
    pub toggle: bool,
    /// ⇧: the range from the anchor.
    pub range: bool,
}

/// The list's multi-selection and its anchor. The cursor (the open snippet) is the frame's
/// `selected_snippet`; transitions take it and return the new one. Ids keep insertion order,
/// as the Tauri `Set` does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Selection {
    order: Vec<Uuid>,
    set: HashSet<Uuid>,
    anchor: Option<Uuid>,
}

impl Selection {
    /// Every id in the multi-selection, one or more (a ⌘-click can leave one).
    pub fn multi(&self) -> &[Uuid] {
        &self.order
    }

    /// The multi-selection when it holds more than one row: what the bulk bar acts on.
    pub fn bulk(&self) -> &[Uuid] {
        if self.order.len() > 1 {
            &self.order
        } else {
            &[]
        }
    }

    /// `selection()`: the multi-selection when it holds more than one row, else the cursor.
    pub fn targets(&self, cursor: Option<Uuid>) -> Vec<Uuid> {
        if self.order.len() > 1 {
            self.order.clone()
        } else {
            cursor.into_iter().collect()
        }
    }

    /// `inSelection`: drawn as selected.
    pub fn includes(&self, id: Uuid, cursor: Option<Uuid>) -> bool {
        if self.order.len() > 1 {
            self.set.contains(&id)
        } else {
            cursor == Some(id)
        }
    }

    fn replace(&mut self, ids: impl IntoIterator<Item = Uuid>) {
        self.order.clear();
        self.set.clear();
        for id in ids {
            if self.set.insert(id) {
                self.order.push(id);
            }
        }
    }

    /// Escape: drops the multi-selection. True when there was one.
    pub fn clear(&mut self) -> bool {
        let had = !self.order.is_empty();
        self.replace([]);
        had
    }

    /// ⌘A.
    pub fn select_all(&mut self, ids: &[Uuid]) {
        self.replace(ids.iter().copied());
    }

    /// Keeps only the ids `keep` accepts (after a delete).
    pub fn retain(&mut self, keep: impl Fn(Uuid) -> bool) {
        let kept: Vec<Uuid> = self.order.iter().copied().filter(|id| keep(*id)).collect();
        self.replace(kept);
    }

    fn range(&mut self, ids: &[Uuid], from: Uuid, to: usize) {
        let from = ids.iter().position(|id| *id == from).unwrap_or(to);
        let (lo, hi) = if from < to { (from, to) } else { (to, from) };
        self.replace(ids[lo..=hi].iter().copied());
    }

    /// A press on row `id`; returns the new cursor, which is always `id`.
    pub fn click(&mut self, ids: &[Uuid], cursor: Option<Uuid>, id: Uuid, mods: ClickMods) -> Uuid {
        if mods.toggle {
            if self.order.is_empty() {
                self.replace(cursor);
            }
            if self.set.remove(&id) {
                self.order.retain(|x| *x != id);
            } else {
                self.set.insert(id);
                self.order.push(id);
            }
        } else if let (true, Some(anchor)) = (mods.range, self.anchor) {
            if let Some(to) = ids.iter().position(|x| *x == id) {
                self.range(ids, anchor, to);
            }
        } else {
            self.anchor = Some(id);
            self.replace([]);
        }
        id
    }

    /// Arrows, J/K, Home/End and Page keys: moves the cursor `delta` rows, clamped. With no
    /// cursor in the list a forward move starts at the top, a backward one at the bottom.
    /// `extend` (⇧) selects from the anchor to the new row; otherwise the row becomes the
    /// anchor and the multi-selection goes.
    pub fn step(
        &mut self,
        ids: &[Uuid],
        cursor: Option<Uuid>,
        delta: isize,
        extend: bool,
    ) -> Option<Uuid> {
        if ids.is_empty() {
            return None;
        }
        let last = ids.len() as isize - 1;
        let at = cursor
            .and_then(|c| ids.iter().position(|id| *id == c))
            .map_or(if delta > 0 { -1 } else { last + 1 }, |i| i as isize);
        let n = (at + delta).clamp(0, last) as usize;
        let id = ids[n];
        if extend {
            let anchor = self.anchor.or(cursor).unwrap_or(id);
            self.anchor = Some(anchor);
            self.range(ids, anchor, n);
        } else {
            self.anchor = Some(id);
            self.replace([]);
        }
        Some(id)
    }
}
