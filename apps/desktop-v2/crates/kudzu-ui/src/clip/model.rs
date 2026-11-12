//! The clipboard list's state and transitions, with no Rune and no window state: rows built
//! from a [`ClipResults`] snapshot, the active row, the multi-selection and its anchor. The
//! main window and the picker both drive one of these (`PLAN.md` S58, S60).
//!
//! Rows are keyed by entry id and read their entry from the `Arc<History>` that came with the
//! results, never from a newer snapshot (the S28 fixes).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use kudzu_core::clipboard::sections::{build_rows, ClipRow, RowOptions};
use kudzu_core::clipboard::{ClipEntry, History};
use uuid::Uuid;

/// Selectable rows moved by Page Up and Page Down in the main window.
pub const PAGE: isize = 8;

/// One line of the list: a day header or a clip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The virtual list's key: the header key or the entry id.
    pub key: String,
    pub kind: RowKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind {
    Header {
        label: String,
        count: usize,
    },
    Entry {
        id: Uuid,
        /// Position in `History::entries` of the snapshot the rows were built from.
        index: usize,
        /// Show the date, not just the time (no sections above it).
        dated: bool,
        /// 0-based position among the selectable rows; the first nine get ⌘1–9.
        position: usize,
    },
}

/// The pointer modifiers a click carried.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Click {
    pub command: bool,
    pub shift: bool,
}

#[derive(Debug)]
pub struct ClipList {
    history: Arc<History>,
    rows: Vec<Row>,
    order: Vec<Uuid>,
    row_of: HashMap<Uuid, usize>,
    active: Option<Uuid>,
    multi: BTreeSet<Uuid>,
    anchor: Option<Uuid>,
    /// Where the active row sat last, so a vanished row hands the cursor to its neighbour.
    last_position: usize,
    version: u64,
}

impl Default for ClipList {
    fn default() -> Self {
        Self {
            history: Arc::new(History::new()),
            rows: Vec::new(),
            order: Vec::new(),
            row_of: HashMap::new(),
            active: None,
            multi: BTreeSet::new(),
            anchor: None,
            last_position: 0,
            version: 0,
        }
    }
}

impl ClipList {
    /// Rebuilds the rows from `indices` into `history`, in the order given. The active row
    /// stays when it is still listed; otherwise its neighbour takes over. Multi-selected ids
    /// that vanished drop out.
    pub fn set(
        &mut self,
        history: Arc<History>,
        indices: &[usize],
        options: RowOptions,
        now: DateTime<FixedOffset>,
    ) {
        let rows = build_rows(
            indices.iter().filter_map(|&i| history.entries.get(i)),
            options,
            now,
        );
        let at: HashMap<Uuid, usize> = indices
            .iter()
            .filter_map(|&i| history.entries.get(i).map(|e| (e.id, i)))
            .collect();
        self.rows.clear();
        self.row_of.clear();
        self.order.clear();
        for row in &rows.rows {
            match row {
                ClipRow::Header { key, label, count } => self.rows.push(Row {
                    key: key.clone(),
                    kind: RowKind::Header {
                        label: label.clone(),
                        count: *count,
                    },
                }),
                ClipRow::Entry {
                    entry,
                    index,
                    dated,
                    ..
                } => {
                    self.row_of.insert(entry.id, self.rows.len());
                    self.order.push(entry.id);
                    self.rows.push(Row {
                        key: entry.id.to_string(),
                        kind: RowKind::Entry {
                            id: entry.id,
                            index: at.get(&entry.id).copied().unwrap_or_default(),
                            dated: *dated,
                            position: *index,
                        },
                    });
                }
            }
        }
        self.history = history;
        self.version += 1;
        self.multi.retain(|id| self.row_of.contains_key(id));
        if self.anchor.is_some_and(|a| !self.row_of.contains_key(&a)) {
            self.anchor = None;
        }
        self.keep_active();
    }

    fn keep_active(&mut self) {
        if let Some(position) = self.position(self.active) {
            self.last_position = position;
        } else {
            self.active = self
                .order
                .get(self.last_position.min(self.order.len().saturating_sub(1)))
                .copied();
        }
    }

    fn position(&self, id: Option<Uuid>) -> Option<usize> {
        let id = id?;
        self.row_of.get(&id)?;
        self.order.iter().position(|&o| o == id)
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Bumps whenever rows are rebuilt: the virtual list's `virtual_rows_version`.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Selectable ids in display order.
    pub fn order(&self) -> &[Uuid] {
        &self.order
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn history(&self) -> &Arc<History> {
        &self.history
    }

    pub fn entry(&self, id: Uuid) -> Option<&ClipEntry> {
        self.history.get(id)
    }

    pub fn active(&self) -> Option<Uuid> {
        self.active
    }

    pub fn active_entry(&self) -> Option<&ClipEntry> {
        self.entry(self.active?)
    }

    /// Row index of `id`, for scrolling it into view with its header above it.
    pub fn row_index(&self, id: Uuid) -> Option<usize> {
        self.row_of.get(&id).copied()
    }

    /// The key to scroll to so the first row of a section keeps its header in view.
    pub fn scroll_key(&self, id: Uuid) -> Option<&str> {
        let at = self.row_index(id)?;
        let header = at
            .checked_sub(1)
            .filter(|&h| matches!(self.rows[h].kind, RowKind::Header { .. }));
        Some(self.rows[header.unwrap_or(at)].key.as_str())
    }

    pub fn is_multi(&self) -> bool {
        self.multi.len() > 1
    }

    pub fn in_multi(&self, id: Uuid) -> bool {
        self.is_multi() && self.multi.contains(&id)
    }

    pub fn multi_count(&self) -> usize {
        self.multi.len()
    }

    /// What an action applies to: the multi-selection in display order, else the active row.
    pub fn selection(&self) -> Vec<Uuid> {
        if self.is_multi() {
            self.order
                .iter()
                .copied()
                .filter(|id| self.multi.contains(id))
                .collect()
        } else {
            self.active.into_iter().collect()
        }
    }

    pub fn clear_multi(&mut self) -> bool {
        let had = !self.multi.is_empty();
        self.multi.clear();
        had
    }

    pub fn select_all(&mut self) {
        self.multi = self.order.iter().copied().collect();
    }

    /// Makes `id` the active row without touching the selection (a search that narrows).
    pub fn activate(&mut self, id: Uuid) {
        if self.row_of.contains_key(&id) {
            self.active = Some(id);
            self.last_position = self.position(Some(id)).unwrap_or(0);
        }
    }

    /// Moves the active row by `delta` selectable rows, clamped. With `extend`, selects the
    /// range from the anchor to the new row (Shift+arrows); without, drops the selection.
    pub fn move_by(&mut self, delta: isize, extend: bool) -> Option<Uuid> {
        if self.order.is_empty() {
            return None;
        }
        let last = self.order.len() as isize - 1;
        let from = match self.position(self.active) {
            Some(at) => at as isize,
            None if delta > 0 => -1,
            None => last + 1,
        };
        let to = (from + delta).clamp(0, last) as usize;
        let id = self.order[to];
        let previous = self.active;
        self.active = Some(id);
        self.last_position = to;
        if extend {
            let anchor = *self.anchor.get_or_insert(previous.unwrap_or(id));
            self.select_range(anchor, id);
        } else {
            self.anchor = Some(id);
            self.multi.clear();
        }
        Some(id)
    }

    /// A pointer press on `id`: ⌘ toggles it in the selection, Shift selects from the anchor,
    /// a plain press makes it the only active row.
    pub fn click(&mut self, id: Uuid, click: Click) {
        if !self.row_of.contains_key(&id) {
            return;
        }
        if click.command {
            if self.multi.is_empty() {
                self.multi.extend(self.active);
            }
            if !self.multi.remove(&id) {
                self.multi.insert(id);
            }
        } else if click.shift && self.anchor.is_some() {
            let anchor = self.anchor.unwrap_or(id);
            self.select_range(anchor, id);
        } else {
            self.anchor = Some(id);
            self.multi.clear();
        }
        self.active = Some(id);
        self.last_position = self.position(Some(id)).unwrap_or(0);
    }

    fn select_range(&mut self, a: Uuid, b: Uuid) {
        let (Some(a), Some(b)) = (self.position(Some(a)), self.position(Some(b))) else {
            return;
        };
        let (lo, hi) = (a.min(b), a.max(b));
        self.multi = self.order[lo..=hi].iter().copied().collect();
    }

    /// Entries for `ids`, skipping any that are gone from this snapshot.
    pub fn entries<'a>(&'a self, ids: &'a [Uuid]) -> impl Iterator<Item = &'a ClipEntry> {
        ids.iter().filter_map(|&id| self.entry(id))
    }

    /// The date the list was built for, in the viewer's zone.
    pub fn now_in(zone: FixedOffset, at: DateTime<Utc>) -> DateTime<FixedOffset> {
        at.with_timezone(&zone)
    }
}

#[cfg(test)]
pub(crate) mod tests;
