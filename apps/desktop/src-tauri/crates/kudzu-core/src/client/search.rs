//! Fuzzy, usage-boosted snippet search shared by the palette and the library list
//! (`lib/search.ts`), plus how those two views scoped and ordered results
//! (`components/app/palette/palette.tsx`, `components/app/main/snippet-list.tsx`).
//!
//! Each whitespace-separated term must match the name, a trigger, or the content. Name and
//! trigger matches are subsequences scored per character with bonuses for word starts and
//! consecutive runs (the fzf model, greedy variant), weighted name > trigger > content;
//! content only matches contiguous text. A query equal to a trigger always wins. Usage
//! nudges ties without burying a clearly better match.
//!
//! Matching works on lowercased UTF-16 code units, exactly like the JS strings it mirrors, so
//! scores, lengths and highlight offsets agree with the web UI. The index folds each snippet's
//! fields once per edit stamp ([`SearchCache`]), so usage-only updates (every expansion)
//! don't re-tokenize the library.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::{KudzuLibrary, SnippetRecord};

use super::groups::descendant_ids;
use super::js;
use super::stats::SnippetUsage;
use super::template::plain_summary16;

/// A snippet's fields, folded for matching.
#[derive(Debug, Clone, PartialEq)]
pub struct Folded {
    key: FoldKey,
    /// Display name: the name, else the first trigger, else "Untitled".
    pub name: String,
    /// Lowercased display name (UTF-16).
    pub name_lower: Vec<u16>,
    pub triggers_lower: Vec<Vec<u16>>,
    /// Lowercased plain summary of the content, at most 2,000 units (UTF-16).
    pub content_lower: Vec<u16>,
}

/// The edit stamp: `${updatedAt}|${name}|${triggers}|${content.length}`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FoldKey {
    updated_at: i64,
    name: String,
    triggers: Vec<String>,
    content_len: usize,
}

impl FoldKey {
    fn of(s: &SnippetRecord) -> Self {
        Self { updated_at: s.updated_at.timestamp(), name: s.name.clone(), triggers: s.triggers.clone(), content_len: js::len16(&s.content) }
    }
}

/// Characters of content kept for matching.
pub const CONTENT_FOLD_LIMIT: usize = 2000;

fn fold(s: &SnippetRecord, key: FoldKey) -> Folded {
    let name = [s.name.as_str(), s.triggers.first().map_or("", String::as_str)].into_iter().find(|n| !n.is_empty()).unwrap_or("Untitled").to_string();
    Folded {
        key,
        name_lower: js::utf16(&js::lower(&name)),
        name,
        triggers_lower: s.triggers.iter().map(|t| js::utf16(&js::lower(t))).collect(),
        content_lower: js::lower16(&plain_summary16(&s.content, None, CONTENT_FOLD_LIMIT)),
    }
}

/// One searchable snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchEntry<'a> {
    pub snippet: &'a SnippetRecord,
    pub folded: Arc<Folded>,
    pub usage: i64,
    /// Last use (ms since the epoch, 0 = never).
    pub last_used: i64,
}

/// Folded fields keyed by snippet id and edit stamp. Keep one per window/view and reuse it.
#[derive(Debug, Clone, Default)]
pub struct SearchCache {
    folded: HashMap<Uuid, Arc<Folded>>,
}

impl SearchCache {
    /// Builds the index, refolding only snippets whose edit stamp changed.
    pub fn build_index<'a>(&mut self, snippets: impl IntoIterator<Item = &'a SnippetRecord>, usage: &HashMap<Uuid, SnippetUsage>) -> Vec<SearchEntry<'a>> {
        snippets
            .into_iter()
            .map(|s| {
                let key = FoldKey::of(s);
                let folded = match self.folded.get(&s.id) {
                    Some(f) if f.key == key => f.clone(),
                    _ => {
                        let f = Arc::new(fold(s, key));
                        self.folded.insert(s.id, f.clone());
                        f
                    }
                };
                let u = usage.get(&s.id).copied().unwrap_or_default();
                SearchEntry { snippet: s, folded, usage: u.count, last_used: u.last }
            })
            .collect()
    }
}

/// [`SearchCache::build_index`] without a cache.
pub fn build_index<'a>(snippets: impl IntoIterator<Item = &'a SnippetRecord>, usage: &HashMap<Uuid, SnippetUsage>) -> Vec<SearchEntry<'a>> {
    SearchCache::default().build_index(snippets, usage)
}

/// Which field decided a hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MatchField {
    Name,
    Trigger,
    Content,
}

/// A ranked result.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit<'a> {
    pub snippet: &'a SnippetRecord,
    pub score: f64,
    /// `None` for an empty query.
    pub field: Option<MatchField>,
    /// UTF-16 offsets into the lowercased display name that matched, sorted, unique.
    pub name_hits: Vec<usize>,
}

fn is_boundary_char(u: u16) -> bool {
    js::is_space_unit(u) || (u < 128 && b"-_./:;,()[]{}'\"!?".contains(&(u as u8)))
}

fn is_boundary(text: &[u16], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = text[i - 1];
    if is_boundary_char(prev) {
        return true;
    }
    // camelHump: lower → upper in the original casing is approximated by letter → digit here.
    let c = text.get(i).copied().unwrap_or(0);
    (u16::from(b'a')..=u16::from(b'z')).contains(&prev) && (u16::from(b'0')..=u16::from(b'9')).contains(&c)
}

struct Fuzzy {
    score: f64,
    positions: Vec<usize>,
}

/// Greedy subsequence match with a backwards tightening pass.
fn fuzzy(term: &[u16], text: &[u16], want_positions: bool) -> Option<Fuzzy> {
    let (n, m) = (text.len(), term.len());
    if m == 0 {
        return Some(Fuzzy { score: 0.0, positions: vec![] });
    }
    if m > n {
        return None;
    }
    // Fast path: contiguous substring.
    if let Some(idx) = js::index_of16(text, term, 0) {
        let mut score = (m * 16 + 8 * (m - 1)) as f64;
        if is_boundary(text, idx) {
            score += 20.0;
        }
        if idx == 0 {
            score += 20.0;
        }
        if m == n {
            score += 20.0;
        }
        score -= idx.min(20) as f64 * 0.5;
        return Some(Fuzzy { score, positions: if want_positions { (idx..idx + m).collect() } else { vec![] } });
    }
    // Forward scan to find the end of the earliest match.
    let mut ti = 0;
    let mut end = None;
    for (i, &c) in text.iter().enumerate() {
        if c == term[ti] {
            ti += 1;
            if ti == m {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;
    // Backward scan from `end` to find the tightest start.
    let mut ti = m as isize - 1;
    let mut start = end;
    for i in (0..=end).rev() {
        if text[i] == term[ti as usize] {
            ti -= 1;
            if ti < 0 {
                start = i;
                break;
            }
        }
    }
    // Score the window forward.
    let mut ti = 0;
    let mut score = 0.0;
    let mut prev_match: isize = -2;
    let mut positions = Vec::new();
    let mut i = start;
    while i <= end && ti < m {
        if text[i] == term[ti] {
            let mut s = 16.0;
            if is_boundary(text, i) {
                s += if ti == 0 { 20.0 } else { 10.0 };
            }
            if prev_match == i as isize - 1 {
                s += 4.0;
            }
            score += s;
            prev_match = i as isize;
            if want_positions {
                positions.push(i);
            }
            ti += 1;
        } else {
            score -= if prev_match == i as isize - 1 { 3.0 } else { 1.0 };
        }
        i += 1;
    }
    Some(Fuzzy { score, positions })
}

fn usage_boost(e: &SearchEntry<'_>, now_ms: i64) -> f64 {
    let freq = (1.0 + e.usage as f64).log2() * 4.0;
    let age_days = if e.last_used != 0 { (now_ms - e.last_used) as f64 / 86_400_000.0 } else { f64::INFINITY };
    let recency = if age_days < 1.0 {
        6.0
    } else if age_days < 7.0 {
        3.0
    } else if age_days < 30.0 {
        1.0
    } else {
        0.0
    };
    freq + recency
}

/// Default result cap.
pub const DEFAULT_LIMIT: usize = 200;
/// Terms are cut to this many UTF-16 units.
pub const TERM_LIMIT: usize = 64;

fn by_score_then_name(a: &SearchHit<'_>, b: &SearchHit<'_>) -> std::cmp::Ordering {
    js::by_score_desc(a.score, b.score, || js::locale_compare(&a.snippet.name, &b.snippet.name))
}

/// Ranks `entries` for `query`, best first, at most `limit`. An empty query sorts by usage,
/// then recency, then name. `now_ms` is the current time in ms since the epoch.
pub fn search<'a>(entries: &[SearchEntry<'a>], query: &str, limit: usize, now_ms: i64) -> Vec<SearchHit<'a>> {
    let q = js::lower(js::trim(query));
    if q.is_empty() {
        let mut hits: Vec<SearchHit<'a>> = entries.iter().map(|e| SearchHit { snippet: e.snippet, score: usage_boost(e, now_ms), field: None, name_hits: vec![] }).collect();
        hits.sort_by(by_score_then_name);
        hits.truncate(limit);
        return hits;
    }
    let terms: Vec<Vec<u16>> = js::split_spaces(&q).into_iter().map(|t| js::utf16(t).into_iter().take(TERM_LIMIT).collect()).collect();
    let mut hits = Vec::new();
    'entries: for e in entries {
        let f = &e.folded;
        let mut total = 0.0;
        let mut field = None;
        let mut best = f64::NEG_INFINITY;
        let mut name_hits: Vec<usize> = Vec::new();
        for term in &terms {
            let mut term_best = f64::NEG_INFINITY;
            let mut term_field = None;
            if let Some(n) = fuzzy(term, &f.name_lower, true) {
                term_best = n.score;
                term_field = Some(MatchField::Name);
                name_hits.extend(n.positions);
            }
            for t in &f.triggers_lower {
                if t == term {
                    term_best = term_best.max(10_000.0);
                    term_field = Some(MatchField::Trigger);
                    continue;
                }
                if t.starts_with(term) {
                    if 5_000.0 > term_best {
                        term_best = 5_000.0 - t.len() as f64;
                        term_field = Some(MatchField::Trigger);
                    }
                    continue;
                }
                if let Some(r) = fuzzy(term, t, false) {
                    if r.score * 0.85 > term_best {
                        term_best = r.score * 0.85;
                        term_field = Some(MatchField::Trigger);
                    }
                }
            }
            // Content only matches contiguous text: fuzzy subsequences over long bodies are
            // noise (and the slowest part of a keystroke).
            if term_best < 40.0 && term.len() > 1 {
                if let Some(at) = js::index_of16(&f.content_lower, term, 0) {
                    let c = (term.len() * 16) as f64 + if is_boundary(&f.content_lower, at) { 10.0 } else { 0.0 };
                    if c * 0.6 > term_best {
                        term_best = c * 0.6;
                        term_field = Some(MatchField::Content);
                    }
                }
            }
            if term_best == f64::NEG_INFINITY {
                continue 'entries;
            }
            total += term_best;
            if term_best > best {
                best = term_best;
                field = term_field;
            }
        }
        total += usage_boost(e, now_ms);
        if !e.snippet.is_enabled {
            total -= 15.0;
        }
        name_hits.sort_unstable();
        name_hits.dedup();
        hits.push(SearchHit { snippet: e.snippet, score: total, field, name_hits });
    }
    hits.sort_by(by_score_then_name);
    hits.truncate(limit);
    hits
}

fn last_of(usage: &HashMap<Uuid, SnippetUsage>, id: Uuid) -> i64 {
    usage.get(&id).map_or(0, |u| u.last)
}

// MARK: Palette

/// The palette's scope tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteScope {
    All,
    Favorites,
    Recent,
}

/// Scope tabs in order (Tab / ⇧Tab cycle through them).
pub const PALETTE_SCOPES: [(PaletteScope, &str); 3] = [(PaletteScope::All, "All"), (PaletteScope::Favorites, "Favorites"), (PaletteScope::Recent, "Recent")];

/// Results the palette shows.
pub const PALETTE_LIMIT: usize = 60;

impl PaletteScope {
    /// The next (or, with `back`, previous) tab, wrapping around.
    pub fn cycle(self, back: bool) -> Self {
        let i = PALETTE_SCOPES.iter().position(|(s, _)| *s == self).unwrap_or(0);
        let n = PALETTE_SCOPES.len();
        PALETTE_SCOPES[(i + if back { n - 1 } else { 1 }) % n].0
    }
}

/// A palette query split into search text and a `#group` filter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PaletteQuery {
    pub text: String,
    /// Lowercased prefix after the first `#` that starts the query or follows whitespace;
    /// `Some("")` for a bare `#` (any grouped snippet).
    pub group_filter: Option<String>,
}

/// `"#work invoice"` → group filter `work` + text `invoice`. Without a filter the text is the
/// query as typed (untrimmed); with one, it's the rest, trimmed.
pub fn parse_palette_query(query: &str) -> PaletteQuery {
    // `/(?:^|\s)#(\S*)/`
    let mut found = None;
    let mut chars = query.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let next_is_hash = chars.peek().is_some_and(|(_, n)| *n == '#');
        if i == 0 && c == '#' {
            found = Some((0, 0));
        } else if js::is_space(c) && next_is_hash {
            found = Some((i, i + c.len_utf8()));
        }
        if found.is_some() {
            break;
        }
    }
    let Some((start, hash)) = found else {
        return PaletteQuery { text: query.to_string(), group_filter: None };
    };
    let tail = &query[hash + 1..];
    let word_len = tail.find(js::is_space).unwrap_or(tail.len());
    let whole = &query[start..hash + 1 + word_len];
    PaletteQuery { text: js::trim(&query.replacen(whole, " ", 1)).to_string(), group_filter: Some(js::lower(&tail[..word_len])) }
}

/// Snippets in the palette's scope, narrowed to groups whose lowercased name starts with
/// `group_filter`.
pub fn palette_scope<'a>(library: &'a KudzuLibrary, scope: PaletteScope, usage: &HashMap<Uuid, SnippetUsage>, group_filter: Option<&str>) -> Vec<&'a SnippetRecord> {
    let groups: Option<Vec<Uuid>> = group_filter.map(|f| library.groups.iter().filter(|g| js::lower(&g.name).starts_with(f)).map(|g| g.id).collect());
    library
        .snippets
        .iter()
        .filter(|s| match scope {
            PaletteScope::All => true,
            PaletteScope::Favorites => s.is_favorite,
            PaletteScope::Recent => last_of(usage, s.id) != 0,
        })
        .filter(|s| groups.as_ref().is_none_or(|ids| s.group_id.is_some_and(|g| ids.contains(&g))))
        .collect()
}

/// Reorders palette hits when there's no search text: Recent by last use, others favourites
/// first then by score. With text the search ranking stands.
pub fn palette_order(hits: &mut [SearchHit<'_>], text: &str, scope: PaletteScope, usage: &HashMap<Uuid, SnippetUsage>) {
    if !text.is_empty() {
        return;
    }
    if scope == PaletteScope::Recent {
        hits.sort_by_key(|h| Reverse(last_of(usage, h.snippet.id)));
    } else {
        hits.sort_by(|a, b| b.snippet.is_favorite.cmp(&a.snippet.is_favorite).then_with(|| js::by_score_desc(a.score, b.score, || std::cmp::Ordering::Equal)));
    }
}

/// The palette's ranked hits for a raw query: parse, scope, index, search, order.
pub fn palette_hits<'a>(library: &'a KudzuLibrary, query: &str, scope: PaletteScope, usage: &HashMap<Uuid, SnippetUsage>, cache: &mut SearchCache, now_ms: i64) -> Vec<SearchHit<'a>> {
    let q = parse_palette_query(query);
    let scoped = palette_scope(library, scope, usage, q.group_filter.as_deref());
    let index = cache.build_index(scoped, usage);
    let mut hits = search(&index, &q.text, PALETTE_LIMIT, now_ms);
    palette_order(&mut hits, &q.text, scope, usage);
    hits
}

/// Whether the palette offers "Create snippet": the query isn't blank and no hit has it as a
/// trigger (case-insensitively).
pub fn palette_offers_create(query: &str, hits: &[SearchHit<'_>]) -> bool {
    let q = js::trim(query);
    if q.is_empty() {
        return false;
    }
    let q = js::lower(q);
    !hits.iter().any(|h| h.snippet.triggers.iter().any(|t| js::lower(t) == q))
}

// MARK: Library list

/// The library list's sidebar view (other views don't list snippets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListView {
    All,
    Favorites,
    Recent,
    Disabled,
    Group(Uuid),
}

/// The list's sort menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ListSort {
    #[default]
    Relevance,
    Name,
    Edited,
    Used,
}

/// Results the list searches over.
pub const LIST_LIMIT: usize = 10_000;

/// Snippets in a view; a group view includes nested groups.
pub fn list_scope<'a>(library: &'a KudzuLibrary, view: ListView, usage: &HashMap<Uuid, SnippetUsage>) -> Vec<&'a SnippetRecord> {
    let s = library.snippets.iter();
    match view {
        ListView::All => s.collect(),
        ListView::Favorites => s.filter(|x| x.is_favorite).collect(),
        ListView::Recent => s.filter(|x| last_of(usage, x.id) != 0).collect(),
        ListView::Disabled => s.filter(|x| !x.is_enabled).collect(),
        ListView::Group(id) => {
            let ids = descendant_ids(&library.groups, id);
            s.filter(|x| x.group_id.is_some_and(|g| ids.contains(&g))).collect()
        }
    }
}

/// Applies the list's sort to search hits. Recent (by relevance or without a filter) sorts by
/// last use; Name, or Relevance without a filter, by name (unnamed last); Edited newest first;
/// Used by count. Relevance with a filter keeps the search ranking.
pub fn list_order(hits: &mut [SearchHit<'_>], filter: &str, sort: ListSort, view: ListView, usage: &HashMap<Uuid, SnippetUsage>) {
    let q = js::trim(filter);
    if view == ListView::Recent && (q.is_empty() || sort == ListSort::Relevance) {
        hits.sort_by_key(|h| Reverse(last_of(usage, h.snippet.id)));
    } else if sort == ListSort::Name || (q.is_empty() && sort == ListSort::Relevance) {
        let key = |s: &SnippetRecord| if s.name.is_empty() { "\u{FFFF}".to_string() } else { s.name.clone() };
        hits.sort_by(|a, b| js::locale_compare_base(&key(a.snippet), &key(b.snippet)));
    } else if sort == ListSort::Edited {
        hits.sort_by_key(|h| Reverse(h.snippet.updated_at.timestamp()));
    } else if sort == ListSort::Used {
        let count = |s: &SnippetRecord| usage.get(&s.id).map_or(0, |u| u.count);
        hits.sort_by_key(|h| Reverse(count(h.snippet)));
    }
}

/// The list's hits for a view, filter and sort.
pub fn list_hits<'a>(library: &'a KudzuLibrary, view: ListView, filter: &str, sort: ListSort, usage: &HashMap<Uuid, SnippetUsage>, cache: &mut SearchCache, now_ms: i64) -> Vec<SearchHit<'a>> {
    let index = cache.build_index(list_scope(library, view, usage), usage);
    let mut hits = search(&index, filter, LIST_LIMIT, now_ms);
    list_order(&mut hits, filter, sort, view, usage);
    hits
}

/// What "New snippet" pre-fills from an unmatched filter: a `;trigger`, or a name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewSnippetSeed {
    Trigger(String),
    Name(String),
}

pub fn new_snippet_seed(filter: &str) -> NewSnippetSeed {
    if filter.starts_with(';') {
        NewSnippetSeed::Trigger(js::trim(filter).to_string())
    } else {
        NewSnippetSeed::Name(js::trim(filter).to_string())
    }
}
