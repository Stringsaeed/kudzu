//! The quick-search palette (`components/app/palette/palette.tsx`, `app/palette/page.tsx`): a
//! floating panel summoned by a global shortcut over any app. Type to fuzzy-search (name,
//! trigger, content; usage-boosted, `#group` filters, Tab for the scope), see the expanded
//! result live, and insert it into the app you came from without touching the mouse.
//!
//! [`PaletteUi`] holds what the component's `useState`s did; [`view`] draws it. The search runs
//! on each edit over `kudzu_core::client::search` with one [`SearchCache`], so a keystroke only
//! re-scores (5,000 snippets: well under a frame in release, see the test below) and the view
//! builds at most 60 rows.

mod view;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use kudzu_core::client::groups::group_path;
use kudzu_core::client::js;
use kudzu_core::client::search::{palette_offers_create, palette_order, palette_scope, parse_palette_query, search, PaletteQuery, PaletteScope, SearchCache, SearchEntry, SearchHit, PALETTE_LIMIT};
use kudzu_core::client::stats::{usage_by_id, SnippetUsage};
use kudzu_core::template::RenderResult;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use kudzu_engine::AppInfo;
use rune::{El, Key, KeyEvent, Modifier, TextEdit, Tx};
use uuid::Uuid;

use crate::backend::commands;
use crate::backend::core::{core, Event};
use crate::backend::windows;

pub use view::view;

/// One ranked result: the snippet's index in the library and the matched name characters.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Hit {
    pub idx: usize,
    /// UTF-16 offsets into the lowercased display name (as `SearchHit::name_hits`).
    pub name_hits: Vec<usize>,
}

/// How a pick acts on a row (`pick(i, mode)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Insert,
    Copy,
    Edit,
}

pub struct PaletteUi {
    pub(crate) library: Arc<KudzuLibrary>,
    pub(crate) usage: HashMap<Uuid, SnippetUsage>,
    /// `groupPath(...).join(" › ")` per group.
    pub(crate) paths: HashMap<Uuid, String>,
    ids: HashMap<Uuid, usize>,
    cache: SearchCache,
    /// The library in collator order by name, worked out on a thread (see [`Ranked`]).
    order: Arc<parking_lot::Mutex<Option<NameOrder>>>,
    /// What the snippets' names hash to now ([`names_stamp`]).
    names_stamp: u64,
    /// Name-order requests made so far.
    order_seq: u64,
    /// Each snippet with its name and content dropped, for [`Ranked`].
    stand_ins: Vec<SnippetRecord>,
    /// The app the palette inserts into.
    pub(crate) target: Option<AppInfo>,
    pub(crate) query: TextEdit,
    pub(crate) scope: PaletteScope,
    active: usize,
    /// The modifier is held: the ⌘1–9 hints light up.
    pub(crate) show_numbers: bool,
    pub(crate) hits: Vec<Hit>,
    pub(crate) create_row: bool,
    /// The search text and `#group` filter of the query the hits are for.
    pub(crate) text: String,
    pub(crate) group_filter: Option<String>,
    /// The active snippet's rendered preview.
    pub(crate) rendered: Option<(Uuid, RenderResult)>,
    /// Replaces the footer's "Insert into …" ("Copied").
    pub(crate) flash: Option<String>,
    /// The row last scrolled into view (the web's effect on `clamped`).
    scrolled: Option<usize>,
}

impl PaletteUi {
    pub fn init() -> PaletteUi {
        let library = Arc::new(core().store.lock().library.clone());
        PaletteUi::with_library(library)
    }

    pub(crate) fn with_library(library: Arc<KudzuLibrary>) -> PaletteUi {
        let mut query = TextEdit::new("");
        // `autoFocus`.
        query.focused = true;
        let mut s = PaletteUi {
            library: Arc::new(KudzuLibrary::default()),
            usage: HashMap::new(),
            paths: HashMap::new(),
            ids: HashMap::new(),
            cache: SearchCache::default(),
            order: Arc::new(parking_lot::Mutex::new(None)),
            names_stamp: 0,
            order_seq: 0,
            stand_ins: Vec::new(),
            target: None,
            query,
            scope: PaletteScope::All,
            active: 0,
            show_numbers: false,
            hits: Vec::new(),
            create_row: false,
            text: String::new(),
            group_filter: None,
            rendered: None,
            flash: None,
            scrolled: None,
        };
        s.set_library(library);
        s
    }

    fn set_library(&mut self, library: Arc<KudzuLibrary>) {
        // Name order only changes with names; usage-only updates (every expansion) keep it.
        let names = names_stamp(&library);
        if names != self.names_stamp {
            self.names_stamp = names;
            self.order_seq += 1;
            let (order, lib, seq) = (self.order.clone(), library.clone(), self.order_seq);
            let work = move || {
                let rank = name_rank(&lib);
                let mut slot = order.lock();
                // An older library's order finishing late doesn't replace a newer one.
                if slot.as_ref().is_none_or(|o| o.seq < seq) {
                    *slot = Some(NameOrder { stamp: names, seq, rank });
                }
            };
            // Unit tests rank in place, so they see the fast path at once.
            let spawned = !cfg!(test) && std::thread::Builder::new().name("kudzu-palette-order".into()).spawn(work.clone()).is_ok();
            if !spawned {
                work();
            }
        }
        self.stand_ins = library.snippets.iter().map(|s| SnippetRecord { name: String::new(), content: String::new(), formatting: None, ..s.clone() }).collect();
        self.usage = usage_by_id(&library);
        self.paths = library.groups.iter().map(|g| (g.id, group_path(&library.groups, Some(g.id)).join(" › "))).collect();
        self.ids = library.snippets.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
        self.library = library;
        // The library changed under the preview: render it again.
        self.rendered = None;
        self.refresh();
    }

    /// Runs the search for the current query and scope, then renders the active preview.
    fn refresh(&mut self) {
        let query = self.query.text.clone();
        let q = parse_palette_query(&query);
        let now = chrono::Utc::now().timestamp_millis();
        let library = self.library.clone();
        let (hits, create_row) = self.palette_hits(&library, &query, &q, now);
        self.hits = hits;
        self.create_row = create_row;
        self.text = q.text;
        self.group_filter = q.group_filter;
        self.render_current();
    }

    /// `palette_hits`, ranked identically but without the collator in the sort. `search` breaks
    /// score ties with `localeCompare` on the names, which allocates on every comparison: with
    /// 5,000 snippets a one-letter query ties thousands of hits and the sort alone takes ~100 ms.
    /// So once the library's name order is known (a thread works it out per name change), the
    /// index goes in in that order, over stand-ins whose names are empty: ties then compare in
    /// a few nanoseconds, and the stable sort leaves them in name order, as the collator would.
    /// Until then it is `palette_hits` itself.
    /// The hits, and whether to offer the create row.
    fn palette_hits(&mut self, library: &KudzuLibrary, query: &str, q: &PaletteQuery, now: i64) -> (Vec<Hit>, bool) {
        let ids = &self.ids;
        let finish = |hits: Vec<SearchHit<'_>>| {
            let create = palette_offers_create(query, &hits);
            (hits.into_iter().filter_map(|h| Some(Hit { idx: *ids.get(&h.snippet.id)?, name_hits: h.name_hits })).collect(), create)
        };
        let order = self.order.lock().as_ref().filter(|o| o.stamp == self.names_stamp).map(|o| o.rank.clone());
        let mut scoped = palette_scope(library, self.scope, &self.usage, q.group_filter.as_deref());
        let Some(rank) = order else {
            let index = self.cache.build_index(scoped, &self.usage);
            let mut hits = search(&index, &q.text, PALETTE_LIMIT, now);
            palette_order(&mut hits, &q.text, self.scope, &self.usage);
            return finish(hits);
        };
        scoped.sort_by_key(|s| rank.get(&s.id).copied().unwrap_or(usize::MAX));
        let index = self.cache.build_index(scoped, &self.usage);
        let stand_ins = &self.stand_ins;
        let index: Vec<SearchEntry<'_>> = index.into_iter().map(|e| SearchEntry { snippet: &stand_ins[ids[&e.snippet.id]], ..e }).collect();
        let mut hits = search(&index, &q.text, PALETTE_LIMIT, now);
        palette_order(&mut hits, &q.text, self.scope, &self.usage);
        finish(hits)
    }

    pub(crate) fn total(&self) -> usize {
        self.hits.len() + usize::from(self.create_row)
    }

    /// `clamped`: the active row, kept inside the results.
    pub(crate) fn clamped(&self) -> usize {
        self.active.min(self.total().saturating_sub(1))
    }

    pub(crate) fn snippet(&self, hit: &Hit) -> &SnippetRecord {
        &self.library.snippets[hit.idx]
    }

    /// The active snippet (none on the create row).
    pub(crate) fn current(&self) -> Option<&SnippetRecord> {
        self.hits.get(self.clamped()).map(|h| self.snippet(h))
    }

    /// The live preview of the active snippet.
    fn render_current(&mut self) {
        let Some(s) = self.current() else { return };
        // Unit tests run without the backend.
        if crate::backend::core::try_core().is_none() {
            return;
        }
        if self.rendered.as_ref().is_some_and(|(id, _)| *id == s.id) {
            return;
        }
        let (id, content, formatting) = (s.id, s.content.clone(), s.formatting.clone());
        let result = commands::render_preview(content, Some(id), HashMap::new(), formatting);
        self.rendered = Some((id, result));
    }

    /// The preview for the active snippet, once rendered.
    pub(crate) fn preview(&self) -> Option<&RenderResult> {
        let cur = self.current()?;
        self.rendered.as_ref().filter(|(id, _)| *id == cur.id).map(|(_, r)| r)
    }

    pub(crate) fn section_label(&self) -> &'static str {
        if !self.text.is_empty() {
            "Best matches"
        } else {
            match self.scope {
                PaletteScope::Recent => "Recently used",
                PaletteScope::Favorites => "Favorites",
                PaletteScope::All => "Suggested",
            }
        }
    }

    pub(crate) fn set_active(&mut self, i: usize, tx: &mut Tx) {
        self.active = i;
        self.after(tx);
    }

    /// After a change: render the new active preview, keep the active row in view.
    fn after(&mut self, tx: &mut Tx) {
        self.render_current();
        let c = self.clamped();
        if self.scrolled != Some(c) {
            self.scrolled = Some(c);
            if self.total() > 0 {
                tx.scroll_into_view(row_key(c));
            }
        }
    }

    pub(crate) fn set_scope(&mut self, scope: PaletteScope, tx: &mut Tx) {
        self.scope = scope;
        self.active = 0;
        self.refresh();
        self.after(tx);
    }

    /// `onChange` of the search field.
    pub(crate) fn query_changed(&mut self, tx: &mut Tx) {
        self.active = 0;
        self.refresh();
        self.after(tx);
    }

    fn trimmed_query(&self) -> String {
        js::trim(&self.query.text).to_string()
    }

    /// `pick(i, mode)`: the create row opens Kudzu with a new snippet.
    pub(crate) fn pick(&mut self, i: usize, mode: Mode) {
        if i >= self.hits.len() {
            commands::palette_create(self.trimmed_query());
            return;
        }
        let id = self.snippet(&self.hits[i]).id;
        match mode {
            Mode::Insert => commands::palette_insert(id),
            Mode::Copy => {
                self.flash = Some("Copied".into());
                if let Err(e) = commands::palette_copy(id) {
                    log::warn!("palette copy: {e}");
                }
            }
            Mode::Edit => commands::show_main(Some(id)),
        }
    }

    /// `onKeyDown` on the search field. True takes the key.
    pub(crate) fn on_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        let m = ev.mods.cmd;
        if let Key::Modifier { modifier, pressed } = ev.key {
            // `if (mod) setShowNumbers(true)` on the key going down; `onKeyUp` of Meta or
            // Control clears it.
            let is_cmd = matches!(modifier, Modifier::SuperLeft | Modifier::SuperRight);
            let is_ctrl = matches!(modifier, Modifier::ControlLeft | Modifier::ControlRight);
            if pressed && m {
                self.show_numbers = true;
            } else if !pressed && (is_cmd || is_ctrl) {
                self.show_numbers = false;
            }
            return true;
        }
        if m {
            self.show_numbers = true;
        }
        let total = self.total();
        let ctrl = |c: char| ev.mods.ctrl && !m && (tx.key_base() == Some(c) || matches!(&ev.key, Key::Text(t) if t.eq_ignore_ascii_case(&c.to_string())));
        match &ev.key {
            Key::Down => self.step(1, total, tx),
            _ if ctrl('n') => self.step(1, total, tx),
            Key::Up => self.step(-1, total, tx),
            _ if ctrl('p') => self.step(-1, total, tx),
            Key::PageDown | Key::PageUp => {
                let d: isize = if ev.key == Key::PageDown { 7 } else { -7 };
                let i = (self.active as isize + d).min(total as isize - 1).max(0);
                self.set_active(i as usize, tx);
            }
            Key::Enter => {
                if total > 0 {
                    self.pick(self.clamped(), if m { Mode::Copy } else { Mode::Insert });
                }
            }
            Key::Command('e') if m => {
                if self.current().is_some() {
                    self.pick(self.clamped(), Mode::Edit);
                }
            }
            Key::Command('n') if m => commands::palette_create(self.trimmed_query()),
            Key::Command(c @ '1'..='9') if m => {
                let i = (*c as u8 - b'1') as usize;
                if i < self.hits.len() {
                    self.pick(i, Mode::Insert);
                }
            }
            Key::Tab => {
                let scope = self.scope.cycle(ev.mods.shift);
                self.set_scope(scope, tx);
            }
            Key::Escape => {
                if self.query.text.is_empty() {
                    commands::hide_palette();
                } else {
                    self.query.clear();
                    self.query_changed(tx);
                }
            }
            // A ⌘ chord the palette doesn't use still lit the numbers.
            _ => return false,
        }
        true
    }

    /// ↓ / ↑ (and ⌃N / ⌃P), wrapping.
    fn step(&mut self, d: isize, total: usize, tx: &mut Tx) {
        let n = total.max(1) as isize;
        let cur = self.active.min(total.saturating_sub(1)) as isize;
        let next = (cur + d).rem_euclid(n);
        self.set_active(next as usize, tx);
    }

    /// `Events.paletteShow`: a fresh search, focused.
    fn show(&mut self, target: Option<AppInfo>, tx: &mut Tx) {
        tx.instant();
        self.target = target;
        self.query.clear();
        self.query.focused = true;
        self.scope = PaletteScope::All;
        self.active = 0;
        self.flash = None;
        self.refresh();
        self.after(tx);
    }
}

/// A hash of every snippet's id and name, in library order.
fn names_stamp(library: &KudzuLibrary) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in &library.snippets {
        s.id.hash(&mut h);
        s.name.hash(&mut h);
    }
    h.finish() | 1
}

/// The library's name order, for the names it was worked out from.
struct NameOrder {
    stamp: u64,
    /// When it was asked for, so the newest wins.
    seq: u64,
    rank: Arc<HashMap<Uuid, usize>>,
}

/// Snippets in `search`'s tie order (`localeCompare` on the name, stable in library order).
fn name_rank(library: &KudzuLibrary) -> Arc<HashMap<Uuid, usize>> {
    let mut order: Vec<&SnippetRecord> = library.snippets.iter().collect();
    order.sort_by(|a, b| js::locale_compare(&a.name, &b.name));
    Arc::new(order.into_iter().enumerate().map(|(i, s)| (s.id, i)).collect())
}

/// The key of result row `i` (the create row is `hits.len()`).
pub(crate) fn row_key(i: usize) -> String {
    format!("pal-row-{i}")
}

pub fn apply(s: &mut PaletteUi, ev: &Event, tx: &mut Tx) -> bool {
    match ev {
        Event::Library(lib) => {
            s.set_library(lib.clone());
            true
        }
        Event::PaletteShow { target } => {
            s.show(target.clone(), tx);
            true
        }
        // The appearance is read per view (`ui::theme_for`); the shell refreshes on change.
        _ => false,
    }
}

/// `window.blur`: hide, without handing focus back (it went where the user clicked).
pub fn on_focus(s: &mut PaletteUi, focused: bool, _tx: &mut Tx) {
    if !focused {
        s.show_numbers = false;
        windows::hide_palette(false);
    }
}

/// Characters of `name` (by code point, as the web's `[...text]`) that `hits` marks.
pub(crate) fn highlight_ranges(name: &str, hits: &[usize]) -> Vec<std::ops::Range<usize>> {
    if hits.is_empty() {
        return Vec::new();
    }
    let set: HashSet<usize> = hits.iter().copied().collect();
    name.char_indices().enumerate().filter(|(i, _)| set.contains(i)).map(|(_, (b, c))| b..b + c.len_utf8()).collect()
}

/// CSS `box-shadow` on a translucent box. Rune draws an outer shadow under the whole box, so
/// through a see-through fill (`bg-foreground/[0.08]`) its interior would darken the box; CSS
/// never paints a shadow under its box. Inset layers draw as usual; each outer layer becomes a
/// ring outside the box: exact for unblurred layers (`0 0 0 1px`, `0 1px 0`), and a band
/// blurred by the layer's σ for soft ones.
pub(crate) fn css_shadows(mut e: El, shadows: &[rune::Shadow], radius: f32) -> El {
    for (i, s) in shadows.iter().enumerate() {
        if s.inset {
            e = e.box_shadow(*s);
            continue;
        }
        if s.color.a <= 0.0 {
            continue;
        }
        let sp = s.spread;
        let ring = if s.blur <= 0.0 {
            // The shifted, spread box minus the box: one border per side.
            let side = |d: f32| (d.max(0.0) + sp).max(0.0);
            crate::ui::kit::button::layer(-sp).border_sides([side(-s.y), side(s.x), side(s.y), side(-s.x)], s.color)
        } else {
            let b = s.blur;
            crate::ui::kit::button::layer(-(sp + b)).border(b, s.color).blur(b * 0.5)
        };
        e = e.child(ring.key(format!("css-shadow-{i}")).offset(s.x, s.y).radius(radius + sp.max(0.0)));
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;
    use kudzu_core::GroupRecord;

    fn snippet(name: &str, trigger: &str, content: &str) -> SnippetRecord {
        let mut s: SnippetRecord = serde_json::from_value(serde_json::json!({ "id": Uuid::new_v4(), "name": name, "triggers": [trigger], "content": content })).unwrap();
        s.is_enabled = true;
        s
    }

    pub(super) fn library(n: usize) -> KudzuLibrary {
        let mut lib = KudzuLibrary::default();
        let g: GroupRecord = serde_json::from_value(serde_json::json!({ "id": Uuid::new_v4(), "name": "Work" })).unwrap();
        for i in 0..n {
            let mut s = snippet(&format!("Snippet number {i} about invoices"), &format!(";s{i}"), &format!("Hello {i}, here is the invoice text for the customer. Thanks!"));
            if i % 3 == 0 {
                s.group_id = Some(g.id);
            }
            lib.snippets.push(s);
        }
        lib.groups.push(g);
        lib
    }

    #[test]
    fn group_filter_and_create_row_follow_the_query() {
        let mut s = PaletteUi::with_library(Arc::new(library(30)));
        assert_eq!(s.hits.len(), 30);
        assert!(!s.create_row);
        s.query.replace_text("#wor invoice");
        s.refresh();
        assert_eq!(s.group_filter.as_deref(), Some("wor"));
        assert_eq!(s.text, "invoice");
        assert_eq!(s.hits.len(), 10);
        assert!(s.create_row);
        s.query.replace_text(";s4");
        s.refresh();
        assert!(!s.create_row, "an exact trigger doesn't offer to create it");
        assert_eq!(s.snippet(&s.hits[0]).triggers[0], ";s4");
    }

    #[test]
    fn highlights_map_code_points_to_bytes() {
        assert_eq!(highlight_ranges("é ab", &[0, 2]), vec![0..2, 3..4]);
        assert!(highlight_ranges("abc", &[]).is_empty());
    }

    /// 5,000 snippets with varied names and content, a quarter of them used.
    pub(super) fn realistic(n: usize) -> KudzuLibrary {
        const WORDS: [&str; 24] = ["invoice", "meeting", "follow", "address", "signature", "thanks", "reply", "draft", "send", "review", "update", "status", "bug", "report", "schedule", "call", "intro", "welcome", "refund", "order", "shipping", "notes", "agenda", "summary"];
        let mut seed = 42_u64;
        let mut next = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize
        };
        let mut lib = library(0);
        for i in 0..n {
            let name = (0..3).map(|_| WORDS[next() % WORDS.len()]).collect::<Vec<_>>().join(" ");
            let content = (0..24).map(|_| WORDS[next() % WORDS.len()]).collect::<Vec<_>>().join(" ");
            let mut s = snippet(&name, &format!(";{}{i}", &WORDS[next() % WORDS.len()][..3]), &content);
            s.is_favorite = i % 17 == 0;
            if i % 4 == 0 {
                lib.usage.push(serde_json::from_value(serde_json::json!({ "snippetID": s.id, "deviceID": "t", "count": next() % 50 + 1, "charsSaved": 10, "lastUsedAt": "2026-10-01T00:00:00Z" })).unwrap());
            }
            lib.snippets.push(s);
        }
        lib
    }

    #[test]
    fn ranking_by_name_order_matches_palette_hits() {
        let lib = Arc::new(realistic(800));
        let mut s = PaletteUi::with_library(lib.clone());
        for scope in [PaletteScope::All, PaletteScope::Favorites, PaletteScope::Recent] {
            for query in ["", "#", "  ", "s", "re", "send r", "#wo", "zz", ";inv"] {
                s.scope = scope;
                s.query.replace_text(query);
                s.refresh();
                let mut cache = SearchCache::default();
                let want: Vec<Uuid> = kudzu_core::client::search::palette_hits(&lib, query, scope, &s.usage, &mut cache, chrono::Utc::now().timestamp_millis()).iter().map(|h| h.snippet.id).collect();
                let got: Vec<Uuid> = s.hits.iter().map(|h| s.snippet(h).id).collect();
                assert_eq!(got, want, "{scope:?} {query:?}");
            }
        }
    }

    #[test]
    fn a_keystroke_over_five_thousand_snippets_stays_small() {
        let mut s = PaletteUi::with_library(Arc::new(realistic(5000)));
        let mut worst = std::time::Duration::ZERO;
        for q in ["s", "se", "sen", "send", "send r", "send re", "", "#", "re"] {
            let t = std::time::Instant::now();
            s.query.replace_text(q);
            s.refresh();
            let d = t.elapsed();
            eprintln!("{q:>8}: {d:?} ({} hits)", s.hits.len());
            worst = worst.max(d);
        }
        // Debug builds are far slower than release; this guards against work that grows with
        // the library per keystroke beyond scoring it.
        assert!(worst < std::time::Duration::from_millis(if cfg!(debug_assertions) { 400 } else { 12 }), "{worst:?}");
    }
}
