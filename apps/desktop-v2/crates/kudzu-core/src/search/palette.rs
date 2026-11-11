//! What the Quick Search palette lists.
//!
//! Scope and the `#prefix` filter choose the snippets. [`PaletteCache`] folds those into a
//! [`SearchIndex`] once per `(library revision, scope, groups)` and each keystroke only
//! [`rank`](super::rank)s. Kind and the form badge are computed in that same pass. The UI
//! keeps labels and icons.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{highlight, index, rank};
use crate::fmt;
use crate::library::groups::group_path;
use crate::library::usage::{usage_by_id, SnippetUsage};
use crate::template::{self, summary::plain_summary};
use crate::{KudzuLibrary, SnippetRecord};

/// Ranked rows the palette shows. The search index's own default is 200.
pub const RESULT_LIMIT: usize = 60;

/// Where [`create_seed`] puts the trimmed query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateAs {
    /// A single token that starts with the default trigger prefix.
    Trigger,
    /// Anything else, including an empty query from ⌘N.
    Name,
}

/// The unsaved editor a create action opens. `value` is the trimmed query.
///
/// `Debug` records the length, never the text.
#[derive(Clone, PartialEq, Eq)]
pub struct CreateSeed {
    pub as_: CreateAs,
    pub value: String,
}

impl std::fmt::Debug for CreateSeed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateSeed")
            .field("as_", &self.as_)
            .field("value_len", &self.value.len())
            .finish()
    }
}

/// `(text sent to rank, group prefix)`.
///
/// The prefix is the first `(start or whitespace) + # + non-whitespace` in `query`,
/// lowercased, and may be empty (`#` alone). That span, including the whitespace that
/// introduced it, is replaced with a space and the remainder is trimmed. No match leaves
/// the query untouched; ranking trims it.
pub fn split_query(query: &str) -> (String, Option<String>) {
    let chars: Vec<(usize, char)> = query.char_indices().collect();
    for (i, &(byte, ch)) in chars.iter().enumerate() {
        let boundary = i == 0 || is_js_space(chars[i - 1].1);
        if !boundary || ch != '#' {
            continue;
        }
        let start = if i == 0 { byte } else { chars[i - 1].0 };
        let mut end = byte + ch.len_utf8();
        let mut filter = String::new();
        for &(b, c) in chars.iter().skip(i + 1) {
            if is_js_space(c) {
                break;
            }
            filter.push(c);
            end = b + c.len_utf8();
        }
        let mut text = String::new();
        text.push_str(&query[..start]);
        text.push(' ');
        text.push_str(&query[end..]);
        return (text.trim().to_string(), Some(filter.to_lowercase()));
    }
    (query.to_string(), None)
}

/// A single token that starts with a non-empty prefix is a trigger.
/// `query` is the field's text, not the text left after the `#` filter.
pub fn create_seed(query: &str, prefix: &str) -> CreateSeed {
    let value = query.trim().to_string();
    let trigger = !value.is_empty()
        && !value.contains(' ')
        && !prefix.is_empty()
        && value.starts_with(prefix);
    CreateSeed {
        as_: if trigger {
            CreateAs::Trigger
        } else {
            CreateAs::Name
        },
        value,
    }
}

fn is_js_space(c: char) -> bool {
    c.is_whitespace()
}

/// All, Favorites, or Recent. Tab cycles them; the choice lasts one opening.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scope {
    #[default]
    All,
    Favorites,
    Recent,
}

impl Scope {
    pub const ALL: [Scope; 3] = [Scope::All, Scope::Favorites, Scope::Recent];

    pub fn label(self) -> &'static str {
        match self {
            Scope::All => "All",
            Scope::Favorites => "Favorites",
            Scope::Recent => "Recent",
        }
    }

    pub fn cycle(self, backward: bool) -> Scope {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        let n = Self::ALL.len();
        let next = if backward {
            (i + n - 1) % n
        } else {
            (i + 1) % n
        };
        Self::ALL[next]
    }
}

/// The icon beside a result, in the reference's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Form,
    Clipboard,
    Calendar,
    File,
}

pub fn kind_of(content: &str) -> Kind {
    let info = template::analyze(&template::parse(content));
    if !info.form_fields.is_empty() {
        return Kind::Form;
    }
    if info.uses_clipboard {
        return Kind::Clipboard;
    }
    if has_date_or_time(content) {
        return Kind::Calendar;
    }
    Kind::File
}

/// "Asks for City, Date" from the template's form fields. `None` when it asks for none.
pub fn form_badge(content: &str) -> Option<String> {
    let info = template::analyze(&template::parse(content));
    if info.form_fields.is_empty() {
        return None;
    }
    let names: Vec<&str> = info.form_fields.iter().map(|f| f.name.as_str()).collect();
    Some(format!("Asks for {}", names.join(", ")))
}

/// One result row. Names and triggers are drawn, never logged.
pub struct Row {
    pub id: Uuid,
    /// The snippet's name, or "Untitled" when that name is empty.
    pub name: String,
    /// Byte ranges of `name` to paint. Empty when the snippet itself has no name.
    pub highlights: Vec<std::ops::Range<usize>>,
    pub triggers: Vec<String>,
    /// Ancestors joined with ` › `, including the snippet's own group.
    pub path: String,
    pub favorite: bool,
    pub enabled: bool,
    pub count: i64,
    pub last_ms: i64,
    pub kind: Kind,
    pub form_badge: Option<String>,
    score: f64,
}

impl std::fmt::Debug for Row {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Row")
            .field("id", &self.id)
            .field("name_len", &self.name.len())
            .field("kind", &self.kind)
            .finish()
    }
}

/// What the list shows for one query.
pub struct Presentation {
    pub rows: Vec<Row>,
    pub create: bool,
    pub section: &'static str,
    /// Set when there is nothing to list, including no create row.
    pub empty: Option<&'static str>,
    /// `· #work`, or `· #…` when the prefix is empty. `None` when the query has no `#`.
    pub group_chip: Option<String>,
    /// The text ranking saw, after the `#` span was removed.
    pub text: String,
    /// The field text this list answers. A keystroke acts on the list only when it matches.
    pub for_query: String,
    pub for_scope: Scope,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            create: false,
            section: "Suggested",
            empty: Some("No snippets yet."),
            group_chip: None,
            text: String::new(),
            for_query: String::new(),
            for_scope: Scope::All,
        }
    }
}

impl std::fmt::Debug for Presentation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presentation")
            .field("rows", &self.rows.len())
            .field("create", &self.create)
            .field("section", &self.section)
            .field("for_query_len", &self.for_query.len())
            .field("for_scope", &self.for_scope)
            .finish()
    }
}

struct Prepared {
    id: Uuid,
    name: String,
    named: bool,
    triggers: Vec<String>,
    path: String,
    favorite: bool,
    enabled: bool,
    count: i64,
    last_ms: i64,
    kind: Kind,
    form_badge: Option<String>,
}

struct Slot {
    revision: u64,
    scope: Scope,
    /// `None` is "no `#` filter". `Some` is the sorted ids of groups whose names match.
    groups: Option<Vec<Uuid>>,
    index: super::SearchIndex,
    prepared: Vec<Prepared>,
}

/// The folded index for one `(revision, scope, groups)`. Shared across keystrokes.
#[derive(Default)]
pub struct PaletteCache {
    slot: Mutex<Option<Slot>>,
    builds: AtomicU64,
}

impl PaletteCache {
    /// How many times the index was built. A second query in the same scope does not add one.
    pub fn index_builds(&self) -> u64 {
        self.builds.load(Ordering::Relaxed)
    }

    /// Rank `query` against the cached index, rebuilding only when the library, scope or
    /// group filter changed.
    pub fn answer(
        &self,
        revision: u64,
        library: &KudzuLibrary,
        query: &str,
        scope: Scope,
        now: DateTime<Utc>,
    ) -> Presentation {
        let (text, group) = split_query(query);
        let groups = matching_groups(library, group.as_deref());
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = slot.as_ref().is_none_or(|cached| {
            cached.revision != revision || cached.scope != scope || cached.groups != groups
        });
        if fresh {
            *slot = Some(build(revision, library, scope, groups));
            self.builds.fetch_add(1, Ordering::Relaxed);
        }
        let cached = slot.as_ref().expect("the slot was just filled");
        let hits = rank(&cached.index, &text, RESULT_LIMIT, now);
        let mut rows: Vec<Row> = hits
            .iter()
            .map(|hit| {
                let prepared = &cached.prepared[hit.position];
                Row {
                    id: prepared.id,
                    name: prepared.name.clone(),
                    highlights: if prepared.named {
                        highlight(&prepared.name, hit.name_hits.as_slice())
                    } else {
                        Vec::new()
                    },
                    triggers: prepared.triggers.clone(),
                    path: prepared.path.clone(),
                    favorite: prepared.favorite,
                    enabled: prepared.enabled,
                    count: prepared.count,
                    last_ms: prepared.last_ms,
                    kind: prepared.kind,
                    form_badge: prepared.form_badge.clone(),
                    score: hit.score,
                }
            })
            .collect();
        if text.is_empty() {
            if scope == Scope::Recent {
                rows.sort_by_key(|row| std::cmp::Reverse(row.last_ms));
            } else {
                rows.sort_by(|a, b| {
                    b.favorite.cmp(&a.favorite).then_with(|| {
                        b.score
                            .partial_cmp(&a.score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                });
            }
        }
        let needle = query.trim().to_lowercase();
        let create = !needle.is_empty()
            && !rows
                .iter()
                .any(|row| row.triggers.iter().any(|t| t.to_lowercase() == needle));
        let empty = if rows.is_empty() && !create {
            Some(empty_copy(scope))
        } else {
            None
        };
        Presentation {
            rows,
            create,
            section: section_title(&text, scope),
            empty,
            group_chip: group.map(|prefix| {
                if prefix.is_empty() {
                    "· #…".to_string()
                } else {
                    format!("· #{prefix}")
                }
            }),
            text,
            for_query: query.to_owned(),
            for_scope: scope,
        }
    }
}

/// One search with a fresh cache. Scenes and tests use this; the palette keeps a cache.
pub fn present(
    library: &KudzuLibrary,
    query: &str,
    scope: Scope,
    now: DateTime<Utc>,
) -> Presentation {
    PaletteCache::default().answer(0, library, query, scope, now)
}

fn empty_copy(scope: Scope) -> &'static str {
    match scope {
        Scope::Favorites => "No favorites yet.",
        Scope::Recent => "Nothing used yet.",
        Scope::All => "No snippets yet.",
    }
}

fn section_title(text: &str, scope: Scope) -> &'static str {
    if !text.is_empty() {
        "Best matches"
    } else if scope == Scope::Recent {
        "Recently used"
    } else if scope == Scope::Favorites {
        "Favorites"
    } else {
        "Suggested"
    }
}

fn matching_groups(library: &KudzuLibrary, prefix: Option<&str>) -> Option<Vec<Uuid>> {
    let prefix = prefix?;
    let mut ids: Vec<Uuid> = library
        .groups
        .iter()
        .filter(|group| group.name.to_lowercase().starts_with(prefix))
        .map(|group| group.id)
        .collect();
    ids.sort();
    Some(ids)
}

fn build(revision: u64, library: &KudzuLibrary, scope: Scope, groups: Option<Vec<Uuid>>) -> Slot {
    let usage = usage_by_id(&library.usage);
    let snippets = filter_snippets(library, scope, groups.as_deref(), &usage);
    let indexed = index(&snippets, &usage, &|body| {
        plain_summary(body, None, crate::template::summary::SEARCH_UNITS)
    });
    let prepared = snippets
        .iter()
        .map(|snippet| prepare(snippet, library, &usage))
        .collect();
    Slot {
        revision,
        scope,
        groups,
        index: indexed,
        prepared,
    }
}

fn filter_snippets(
    library: &KudzuLibrary,
    scope: Scope,
    groups: Option<&[Uuid]>,
    usage: &std::collections::HashMap<Uuid, SnippetUsage>,
) -> Vec<SnippetRecord> {
    library
        .snippets
        .iter()
        .filter(|snippet| match scope {
            Scope::All => true,
            Scope::Favorites => snippet.is_favorite,
            Scope::Recent => usage.get(&snippet.id).is_some_and(|u| u.last_ms != 0),
        })
        .filter(|snippet| match groups {
            None => true,
            Some(ids) => snippet.group_id.is_some_and(|id| ids.contains(&id)),
        })
        .cloned()
        .collect()
}

fn prepare(
    snippet: &SnippetRecord,
    library: &KudzuLibrary,
    usage: &std::collections::HashMap<Uuid, SnippetUsage>,
) -> Prepared {
    let used = usage.get(&snippet.id).copied().unwrap_or_default();
    let named = !snippet.name.is_empty();
    Prepared {
        id: snippet.id,
        name: if named {
            snippet.name.clone()
        } else {
            "Untitled".into()
        },
        named,
        triggers: snippet.triggers.clone(),
        path: group_path(&library.groups, snippet.group_id).join(" › "),
        favorite: snippet.is_favorite,
        enabled: snippet.is_enabled,
        count: used.count,
        last_ms: used.last_ms,
        kind: kind_of(&snippet.content),
        form_badge: form_badge(&snippet.content),
    }
}

fn has_date_or_time(content: &str) -> bool {
    content.split("{{").skip(1).any(|rest| {
        let head = rest.trim_start();
        head.starts_with("date") || head.starts_with("time")
    })
}

/// The preview's Used line. A zero count is "Never" even when a timestamp is stored.
pub fn used_line(count: i64, last_ms: i64, now_ms: i64, offset_minutes: i32) -> String {
    if count == 0 {
        return "Never".into();
    }
    format!(
        "{}× · {}",
        fmt::format_count(count),
        fmt::relative_time(last_ms, now_ms, offset_minutes).to_lowercase()
    )
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
