use super::*;
use chrono::TimeZone;
use uuid::Uuid;

use crate::{GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap()
}

fn at(ms_before: i64) -> Option<chrono::DateTime<Utc>> {
    Some(now() - chrono::Duration::milliseconds(ms_before))
}

fn use_of(id: Uuid, count: i64, last: Option<chrono::DateTime<Utc>>) -> UsageRecord {
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
fn a_hash_prefix_is_the_group_filter_and_leaves_the_text() {
    let (text, group) = split_query("#Work invoice");
    assert_eq!(text, "invoice");
    assert_eq!(group.as_deref(), Some("work"));
}

#[test]
fn only_the_first_hash_matches_and_a_mid_word_hash_does_not_start_one() {
    let (text, group) = split_query("cost#tag #Email later #nope");
    assert_eq!(group.as_deref(), Some("email"));
    assert_eq!(text, "cost#tag  later #nope");
}

#[test]
fn a_bare_hash_matches_every_group_name() {
    let (text, group) = split_query("see #");
    assert_eq!(text, "see");
    assert_eq!(group.as_deref(), Some(""));
}

#[test]
fn no_hash_keeps_the_query_including_surrounding_space() {
    let (text, group) = split_query("  hello  ");
    assert_eq!(text, "  hello  ");
    assert!(group.is_none());
}

#[test]
fn a_prefixed_token_is_a_trigger_and_anything_else_is_a_name() {
    assert_eq!(create_seed(";zz", ";").as_, CreateAs::Trigger);
    assert_eq!(create_seed(";zz", ";").value, ";zz");
    assert_eq!(create_seed("brand new", ";").as_, CreateAs::Name);
    assert_eq!(create_seed("alpha", ";").as_, CreateAs::Name);
    assert_eq!(create_seed(";zz", "").as_, CreateAs::Name);
    assert_eq!(create_seed("  ", ";").value, "");
    assert_eq!(create_seed("  ", ";").as_, CreateAs::Name);
}

#[test]
fn an_empty_query_lists_favorites_first_then_usage() {
    let lib = sample();
    let p = present(&lib.library, "", Scope::All, now());
    assert_eq!(ids(&p), vec![lib.alpha, lib.gamma, lib.beta, lib.delta]);
    assert_eq!(p.section, "Suggested");
    assert!(!p.create);
    assert!(p.empty.is_none());
    assert_eq!(p.for_query, "");
    assert_eq!(p.for_scope, Scope::All);
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

    let snippet = SnippetRecord::new("City", vec![], "Hi {{field name=City}}");
    let library = KudzuLibrary {
        snippets: vec![snippet],
        ..KudzuLibrary::default()
    };
    let listed = present(&library, "city", Scope::All, now());
    assert_eq!(listed.rows[0].kind, Kind::Form);
    assert_eq!(listed.rows[0].form_badge.as_deref(), Some("Asks for City"));
}

#[test]
fn a_name_match_highlights_the_shown_name() {
    let lib = sample();
    let p = present(&lib.library, "alp", Scope::All, now());
    assert_eq!(&p.rows[0].name[p.rows[0].highlights[0].clone()], "Alp");
}

#[test]
fn a_second_query_reuses_the_index() {
    let lib = sample();
    let cache = PaletteCache::default();
    let now = now();
    let _ = cache.answer(1, &lib.library, "alp", Scope::All, now);
    let _ = cache.answer(1, &lib.library, "beta", Scope::All, now);
    assert_eq!(cache.index_builds(), 1);
    let _ = cache.answer(1, &lib.library, "#work", Scope::All, now);
    assert_eq!(cache.index_builds(), 2);
    let _ = cache.answer(2, &lib.library, "#work", Scope::All, now);
    assert_eq!(cache.index_builds(), 3);
    let _ = cache.answer(2, &lib.library, "", Scope::Favorites, now);
    assert_eq!(cache.index_builds(), 4);
}
