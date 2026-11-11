//! Which rows the palette lists.
//!
//! Scope and the `#prefix` filter run first. The prefix matches a group's own name
//! (case-insensitive) and keeps snippets assigned to those groups directly: a child
//! named "Email" under "Work" matches `#em`, not `#work`. Ranking then uses the
//! search index (60 results, terms cut at 64 units, content summary cut at 2,000).
//! An empty query reorders: Recent by last use, otherwise favorites first.

use std::ops::Range;

use chrono::{DateTime, Utc};
use kudzu_core::fmt::{self, format_count};
use kudzu_core::library::groups::group_path;
use kudzu_core::library::usage::{usage_by_id, SnippetUsage};
use kudzu_core::search::{self, highlight};
use kudzu_core::template::{self, summary::plain_summary};
use kudzu_core::{KudzuLibrary, SnippetRecord};
use uuid::Uuid;

use super::query::{split_query, RESULT_LIMIT};

/// All, Favorites, or Recent. Tab cycles them; the choice lasts one opening.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
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

/// One result row. Names and triggers are drawn, never logged.
pub struct Row {
    pub id: Uuid,
    /// The snippet's name, or "Untitled" when that name is empty.
    pub name: String,
    /// Byte ranges of `name` to paint. Empty when the snippet itself has no name,
    /// because the search hits then index the trigger, not this label.
    pub highlights: Vec<Range<usize>>,
    pub triggers: Vec<String>,
    /// Ancestors joined with ` › `, including the snippet's own group.
    pub path: String,
    pub favorite: bool,
    pub enabled: bool,
    pub count: i64,
    pub last_ms: i64,
    score: f64,
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
        }
    }
}

impl std::fmt::Debug for Presentation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Presentation")
            .field("rows", &self.rows.len())
            .field("create", &self.create)
            .field("section", &self.section)
            .finish()
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

/// The preview's Used line. A zero count is "Never" even when a timestamp is stored.
pub fn used_line(count: i64, last_ms: i64, now_ms: i64, offset_minutes: i32) -> String {
    if count == 0 {
        return "Never".into();
    }
    format!(
        "{}× · {}",
        format_count(count),
        fmt::relative_time(last_ms, now_ms, offset_minutes).to_lowercase()
    )
}

pub fn present(
    library: &KudzuLibrary,
    query: &str,
    scope: Scope,
    now: DateTime<Utc>,
) -> Presentation {
    let (text, group) = split_query(query);
    let usage = usage_by_id(&library.usage);
    let filtered = filter_snippets(library, scope, group.as_deref(), &usage);
    let index = search::index(&filtered, &usage, &|body| {
        plain_summary(body, None, kudzu_core::template::summary::SEARCH_UNITS)
    });
    let hits = search::rank(&index, &text, RESULT_LIMIT, now);
    let mut rows: Vec<Row> = hits
        .iter()
        .map(|hit| {
            let snippet = &filtered[hit.position];
            let used = usage.get(&snippet.id).copied().unwrap_or_default();
            row_from(snippet, library, hit.name_hits.as_slice(), hit.score, used)
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
        Some(match scope {
            Scope::Favorites => "No favorites yet.",
            Scope::Recent => "Nothing used yet.",
            Scope::All => "No snippets yet.",
        })
    } else {
        None
    };
    let section = if !text.is_empty() {
        "Best matches"
    } else if scope == Scope::Recent {
        "Recently used"
    } else if scope == Scope::Favorites {
        "Favorites"
    } else {
        "Suggested"
    };
    let group_chip = group.map(|prefix| {
        if prefix.is_empty() {
            "· #…".to_string()
        } else {
            format!("· #{prefix}")
        }
    });
    Presentation {
        rows,
        create,
        section,
        empty,
        group_chip,
        text,
    }
}

fn filter_snippets(
    library: &KudzuLibrary,
    scope: Scope,
    group: Option<&str>,
    usage: &std::collections::HashMap<Uuid, SnippetUsage>,
) -> Vec<SnippetRecord> {
    let group_ids: Option<Vec<Uuid>> = group.map(|prefix| {
        library
            .groups
            .iter()
            .filter(|g| g.name.to_lowercase().starts_with(prefix))
            .map(|g| g.id)
            .collect()
    });
    library
        .snippets
        .iter()
        .filter(|snippet| match scope {
            Scope::All => true,
            Scope::Favorites => snippet.is_favorite,
            Scope::Recent => usage.get(&snippet.id).is_some_and(|u| u.last_ms != 0),
        })
        .filter(|snippet| match &group_ids {
            None => true,
            Some(ids) => snippet.group_id.is_some_and(|id| ids.contains(&id)),
        })
        .cloned()
        .collect()
}

fn row_from(
    snippet: &SnippetRecord,
    library: &KudzuLibrary,
    name_hits: &[usize],
    score: f64,
    used: SnippetUsage,
) -> Row {
    let named = !snippet.name.is_empty();
    Row {
        id: snippet.id,
        name: if named {
            snippet.name.clone()
        } else {
            "Untitled".into()
        },
        highlights: if named {
            highlight(&snippet.name, name_hits)
        } else {
            Vec::new()
        },
        triggers: snippet.triggers.clone(),
        path: group_path(&library.groups, snippet.group_id).join(" › "),
        favorite: snippet.is_favorite,
        enabled: snippet.is_enabled,
        count: used.count,
        last_ms: used.last_ms,
        score,
    }
}

fn has_date_or_time(content: &str) -> bool {
    content.split("{{").skip(1).any(|rest| {
        let head = rest.trim_start();
        head.starts_with("date") || head.starts_with("time")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use kudzu_core::{GroupRecord, UsageRecord};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap()
    }

    fn at(ms_before: i64) -> Option<DateTime<Utc>> {
        Some(now() - chrono::Duration::milliseconds(ms_before))
    }

    fn use_of(id: Uuid, count: i64, last: Option<DateTime<Utc>>) -> UsageRecord {
        UsageRecord {
            snippet_id: id,
            device_id: "test".into(),
            count,
            chars_saved: 0,
            last_used_at: last,
        }
    }

    struct Lib {
        alpha: Uuid,
        beta: Uuid,
        gamma: Uuid,
        delta: Uuid,
        library: KudzuLibrary,
    }

    fn sample() -> Lib {
        let work = GroupRecord::new("Work");
        let mut email = GroupRecord::new("Email");
        email.parent_id = Some(work.id);
        let personal = GroupRecord::new("Personal");
        let mut alpha = SnippetRecord::new("Alpha", vec![";a".into()], "alpha body");
        alpha.group_id = Some(work.id);
        alpha.is_favorite = true;
        let mut beta = SnippetRecord::new("Beta", vec![";b".into()], "beta body");
        beta.group_id = Some(email.id);
        let mut gamma = SnippetRecord::new("Gamma", vec![";g".into()], "gamma body");
        gamma.group_id = Some(personal.id);
        gamma.is_favorite = true;
        let delta = SnippetRecord::new("Delta", vec![";d".into()], "delta body");
        let library = KudzuLibrary {
            groups: vec![work.clone(), email.clone(), personal.clone()],
            snippets: vec![alpha.clone(), beta.clone(), gamma.clone(), delta.clone()],
            usage: vec![
                use_of(alpha.id, 10, at(1_000)),
                use_of(beta.id, 4, at(60_000)),
                use_of(delta.id, 2, at(3_600_000)),
            ],
            ..KudzuLibrary::default()
        };
        Lib {
            alpha: alpha.id,
            beta: beta.id,
            gamma: gamma.id,
            delta: delta.id,
            library,
        }
    }

    fn ids(p: &Presentation) -> Vec<Uuid> {
        p.rows.iter().map(|r| r.id).collect()
    }

    #[test]
    fn an_empty_query_lists_favorites_first_then_usage() {
        let lib = sample();
        let p = present(&lib.library, "", Scope::All, now());
        assert_eq!(ids(&p), vec![lib.alpha, lib.gamma, lib.beta, lib.delta]);
        assert_eq!(p.section, "Suggested");
        assert!(!p.create);
        assert!(p.empty.is_none());
    }

    #[test]
    fn favorites_and_recent_scopes_filter_and_recent_orders_by_last_use() {
        let lib = sample();
        let fav = present(&lib.library, "", Scope::Favorites, now());
        assert_eq!(ids(&fav), vec![lib.alpha, lib.gamma]);
        assert_eq!(fav.section, "Favorites");
        let recent = present(&lib.library, "", Scope::Recent, now());
        assert_eq!(ids(&recent), vec![lib.alpha, lib.beta, lib.delta]);
        assert!(!ids(&recent).contains(&lib.gamma));
        assert_eq!(recent.section, "Recently used");
    }

    #[test]
    fn a_hash_prefix_matches_the_groups_own_name_not_its_descendants() {
        let lib = sample();
        let work = present(&lib.library, "#work", Scope::All, now());
        assert_eq!(ids(&work), vec![lib.alpha]);
        assert_eq!(work.group_chip.as_deref(), Some("· #work"));
        assert_eq!(work.rows[0].path, "Work");
        let email = present(&lib.library, "#em", Scope::All, now());
        assert_eq!(ids(&email), vec![lib.beta]);
        assert_eq!(email.rows[0].path, "Work › Email");
        let crossed = present(&lib.library, "#work beta", Scope::All, now());
        assert!(crossed.rows.is_empty());
        assert!(crossed.create);
        let personal = present(&lib.library, "#p", Scope::Favorites, now());
        assert_eq!(ids(&personal), vec![lib.gamma]);
    }

    #[test]
    fn a_bare_hash_keeps_grouped_snippets_and_drops_the_rest() {
        let lib = sample();
        let p = present(&lib.library, "#", Scope::All, now());
        let got = ids(&p);
        assert!(got.contains(&lib.alpha) && got.contains(&lib.beta) && got.contains(&lib.gamma));
        assert!(!got.contains(&lib.delta));
        assert_eq!(p.group_chip.as_deref(), Some("· #…"));
    }

    #[test]
    fn recent_scope_drops_a_never_used_name_match() {
        let lib = sample();
        let p = present(&lib.library, "gamma", Scope::Recent, now());
        assert!(p.rows.is_empty());
        assert!(p.create);
    }

    #[test]
    fn the_create_row_hides_only_when_a_hit_trigger_equals_the_whole_query() {
        let lib = sample();
        let exact = present(&lib.library, ";a", Scope::All, now());
        assert!(!exact.create);
        assert_eq!(ids(&exact), vec![lib.alpha]);
        let fresh = present(&lib.library, "brand new", Scope::All, now());
        assert!(fresh.create);
        assert_eq!(fresh.section, "Best matches");
    }

    #[test]
    fn an_empty_scope_names_itself() {
        let lib = sample();
        let none = KudzuLibrary::default();
        assert_eq!(
            present(&none, "", Scope::All, now()).empty,
            Some("No snippets yet.")
        );
        assert_eq!(
            present(&lib.library, "", Scope::Favorites, now()).empty,
            None
        );
        let no_fav = present(&KudzuLibrary::default(), "", Scope::Favorites, now());
        assert_eq!(no_fav.empty, Some("No favorites yet."));
        assert_eq!(
            present(&KudzuLibrary::default(), "", Scope::Recent, now()).empty,
            Some("Nothing used yet.")
        );
    }

    #[test]
    fn a_disabled_snippet_stays_in_the_list() {
        let mut lib = sample();
        lib.library.snippets[0].is_enabled = false;
        let p = present(&lib.library, "alpha", Scope::All, now());
        assert_eq!(p.rows[0].id, lib.alpha);
        assert!(!p.rows[0].enabled);
    }

    #[test]
    fn an_empty_name_is_untitled_and_is_not_highlighted() {
        let mut snippet = SnippetRecord::new("", vec![";noname".into()], "body");
        snippet.id = Uuid::from_u128(9);
        let library = KudzuLibrary {
            snippets: vec![snippet],
            ..KudzuLibrary::default()
        };
        let p = present(&library, ";noname", Scope::All, now());
        assert_eq!(p.rows[0].name, "Untitled");
        assert!(p.rows[0].highlights.is_empty());
    }

    #[test]
    fn the_term_is_cut_at_64_units_and_the_list_at_60_rows() {
        let name = "a".repeat(64);
        let snippet = SnippetRecord::new(&name, vec![], "body");
        let id = snippet.id;
        let library = KudzuLibrary {
            snippets: vec![snippet],
            ..KudzuLibrary::default()
        };
        let query = format!("{name}zzz");
        let p = present(&library, &query, Scope::All, now());
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].id, id);

        let mut many = KudzuLibrary::default();
        for i in 0..70 {
            many.snippets
                .push(SnippetRecord::new(format!("Snippet {i:02}"), vec![], "x"));
        }
        assert_eq!(present(&many, "", Scope::All, now()).rows.len(), 60);
    }

    #[test]
    fn content_past_2000_units_is_not_searched() {
        let inside = format!("hello {}", "a".repeat(1994));
        let body = format!("{inside} zephyrunique");
        assert!(inside.chars().count() >= 2000 || inside.encode_utf16().count() >= 2000);
        let visible = SnippetRecord::new("Visible", vec![], body);
        let other = SnippetRecord::new("Other", vec![], "zephyrunique lives here");
        let visible_id = visible.id;
        let other_id = other.id;
        let library = KudzuLibrary {
            snippets: vec![visible, other],
            ..KudzuLibrary::default()
        };
        let missed = present(&library, "zephyrunique", Scope::All, now());
        assert_eq!(ids(&missed), vec![other_id]);
        let hit = present(&library, "hello", Scope::All, now());
        assert_eq!(ids(&hit), vec![visible_id]);
    }

    #[test]
    fn the_used_line_and_the_form_badge_follow_the_reference() {
        let now_ms = now().timestamp_millis();
        assert_eq!(used_line(0, now_ms, now_ms, 0), "Never");
        let line = used_line(1200, now_ms - 5_000, now_ms, 0);
        assert!(line.starts_with("1.2k× · "), "{line}");
        let badge = form_badge("Hi {{field name=City}} and {{field name=Date}}").unwrap();
        assert_eq!(badge, "Asks for City, Date");
        assert!(form_badge("plain").is_none());
        assert_eq!(kind_of("{{field name=City}}"), Kind::Form);
        assert_eq!(kind_of("{{clipboard}}"), Kind::Clipboard);
        assert_eq!(kind_of("on {{date}}"), Kind::Calendar);
        assert_eq!(kind_of("plain"), Kind::File);
    }

    #[test]
    fn a_name_match_highlights_the_shown_name() {
        let lib = sample();
        let p = present(&lib.library, "alp", Scope::All, now());
        assert_eq!(&p.rows[0].name[p.rows[0].highlights[0].clone()], "Alp");
    }
}
