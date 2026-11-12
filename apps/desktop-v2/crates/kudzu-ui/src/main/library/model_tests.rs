//! The list model against the S02 sample library (`lib/mock.ts`): scopes, sorts, the cap,
//! the selection anchors and the "keep a valid selection" rule (`snippet-list.tsx`).

use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use kudzu_app::fixtures::SampleState;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use uuid::Uuid;

use super::model::{keep_valid, list, ClickMods, Indexed, Selection, Sort, CAP};
use crate::main::state::LibraryFilter;

fn sample() -> KudzuLibrary {
    SampleState::load().unwrap().library
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 11, 12, 0, 0).unwrap()
}

fn names(ix: &Indexed, scope: &LibraryFilter, query: &str, sort: Sort) -> Vec<String> {
    list(ix, scope, query, sort, now())
        .iter()
        .map(|h| ix.snippet(h.position).name.clone())
        .collect()
}

fn group_id(lib: &KudzuLibrary, name: &str) -> Uuid {
    lib.groups.iter().find(|g| g.name == name).unwrap().id
}

fn indexed(lib: KudzuLibrary) -> Indexed {
    Indexed::new(Arc::new(lib))
}

#[test]
fn scopes_follow_the_view_and_a_group_includes_its_descendants() {
    let lib = sample();
    let (work, email) = (group_id(&lib, "Work"), group_id(&lib, "Email"));
    let ix = indexed(lib);
    let count = |scope: LibraryFilter| names(&ix, &scope, "", Sort::Name).len();
    assert_eq!(count(LibraryFilter::All), 18);
    assert_eq!(
        names(&ix, &LibraryFilter::Favorites, "", Sort::Name),
        ["Email signature", "Shrug", "Today"]
    );
    // Recent: snippets with a last use; Off: the snippet's own flag only.
    assert_eq!(count(LibraryFilter::Recent), 12);
    assert_eq!(
        names(&ix, &LibraryFilter::Disabled, "", Sort::Name),
        ["Lorem ipsum"]
    );
    assert_eq!(
        names(&ix, &LibraryFilter::Group(work), "", Sort::Name),
        [
            "Calendar link",
            "Email signature",
            "Meeting follow-up",
            "Out of office",
            "Sign-up form"
        ]
    );
    assert_eq!(count(LibraryFilter::Group(email)), 3);
}

#[test]
fn an_empty_best_match_sorts_by_name_with_unnamed_snippets_last() {
    let mut lib = sample();
    let mut unnamed = lib.snippets[0].clone();
    unnamed.id = Uuid::from_u128(1);
    unnamed.name.clear();
    lib.snippets.insert(0, unnamed);
    let mut lower = lib.snippets[5].clone();
    lower.id = Uuid::from_u128(2);
    lower.name = "bug report".into();
    lib.snippets.push(lower);
    let ix = indexed(lib);
    let listed = names(&ix, &LibraryFilter::All, "", Sort::BestMatch);
    assert_eq!(listed, names(&ix, &LibraryFilter::All, "", Sort::Name));
    assert_eq!(listed.first().map(String::as_str), Some("Bug report"));
    // Case ties keep rank order (stable); "bug report" ranks after "Bug report".
    assert_eq!(listed[1], "bug report");
    assert_eq!(listed[2], "Calendar link");
    assert_eq!(listed.last().map(String::as_str), Some(""));
    assert_eq!(listed[listed.len() - 2], "Today");
}

#[test]
fn best_match_with_a_query_keeps_the_ranking() {
    let ix = indexed(sample());
    let listed = names(&ix, &LibraryFilter::All, ";sig", Sort::BestMatch);
    assert_eq!(listed.first().map(String::as_str), Some("Email signature"));
    assert!(listed.contains(&"Sign-up form".to_owned()));
    // Name sort reorders the same hits.
    let by_name = names(&ix, &LibraryFilter::All, ";sig", Sort::Name);
    let mut sorted = listed.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert_eq!(by_name, sorted);
}

#[test]
fn recent_sorts_by_last_use_unless_a_query_asks_for_another_sort() {
    let ix = indexed(sample());
    let first = |sort, q: &str| names(&ix, &LibraryFilter::Recent, q, sort)[0].clone();
    // No query: last use, whatever the menu says.
    assert_eq!(first(Sort::Name, ""), "Email signature");
    assert_eq!(first(Sort::Used, ""), "Email signature");
    let recent = names(&ix, &LibraryFilter::Recent, "", Sort::Name);
    assert_eq!(
        &recent[..3],
        ["Email signature", "Meeting follow-up", "Out of office"]
    );
    // A query with Best match still sorts by last use; with Name, by name.
    assert_eq!(
        names(&ix, &LibraryFilter::Recent, "e", Sort::BestMatch)[0],
        "Email signature"
    );
    assert_eq!(first(Sort::Name, "e"), "Bug report");
}

#[test]
fn recently_edited_and_most_used_sort_descending() {
    let ix = indexed(sample());
    let edited = list(&ix, &LibraryFilter::All, "", Sort::Edited, now());
    let stamps: Vec<_> = edited
        .iter()
        .map(|h| ix.snippet(h.position).updated_at)
        .collect();
    assert!(stamps.windows(2).all(|w| w[0] >= w[1]));
    let used = names(&ix, &LibraryFilter::All, "", Sort::Used);
    assert_eq!(&used[..2], ["Email signature", "Meeting follow-up"]);
    // Unused snippets tie at 0 and keep the name order the empty query ranked them in.
    let tail: Vec<_> = used[12..].to_vec();
    let mut sorted = tail.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert_eq!(tail, sorted);
}

#[test]
fn the_list_stops_at_ten_thousand_results() {
    let mut lib = sample();
    let template = lib.snippets[0].clone();
    lib.snippets = (0..CAP as u128 + 50)
        .map(|n| SnippetRecord {
            id: Uuid::from_u128(n + 10),
            name: format!("Snippet {n:05}"),
            ..template.clone()
        })
        .collect();
    let ix = indexed(lib);
    assert_eq!(
        list(&ix, &LibraryFilter::All, "", Sort::Name, now()).len(),
        CAP
    );
}

#[test]
fn names_highlight_only_named_snippets() {
    let mut lib = sample();
    lib.snippets[0].name.clear();
    let ix = indexed(lib);
    let hits = list(&ix, &LibraryFilter::All, "sig", Sort::BestMatch, now());
    let signature = hits
        .iter()
        .find(|h| ix.snippet(h.position).name.is_empty())
        .expect("the unnamed signature still matches its trigger");
    let row = ix.row(signature);
    assert_eq!(row.name, "Untitled");
    assert!(row.untitled && row.marks.is_empty());
    let named = hits
        .iter()
        .find(|h| ix.snippet(h.position).name == "Sign-up form")
        .unwrap();
    assert!(!ix.row(named).marks.is_empty());
}

#[test]
fn rows_show_the_first_trigger_its_extra_count_usage_and_marks() {
    let mut lib = sample();
    let work = group_id(&lib, "Work");
    lib.groups
        .iter_mut()
        .find(|g| g.id == work)
        .unwrap()
        .is_enabled = false;
    let ix = indexed(lib);
    let hits = list(&ix, &LibraryFilter::All, "", Sort::Name, now());
    let row_of = |name: &str| {
        let hit = hits
            .iter()
            .find(|h| ix.snippet(h.position).name == name)
            .unwrap();
        ix.row(hit)
    };
    let follow = row_of("Meeting follow-up");
    assert_eq!(
        (follow.trigger.as_deref(), follow.more_triggers),
        (Some(";fu"), 1)
    );
    assert_eq!(follow.usage, 75);
    // The group is off, so its snippets show the off mark although their own flag is on.
    assert!(follow.off && !follow.favorite);
    let shrug = row_of("Shrug");
    assert!(shrug.favorite && !shrug.off);
    assert!(row_of("Lorem ipsum").off);
    assert_eq!(row_of("Sign-up form").usage, 0);
    assert!(!row_of("Email signature").summary.is_empty());
}

fn ids(n: u128) -> Vec<Uuid> {
    (1..=n).map(Uuid::from_u128).collect()
}

#[test]
fn shift_moves_extend_from_a_fixed_anchor() {
    let list = ids(6);
    let mut sel = Selection::default();
    let mut cursor = Some(list[2]);
    cursor = sel.step(&list, cursor, 1, true);
    cursor = sel.step(&list, cursor, 1, true);
    assert_eq!(cursor, Some(list[4]));
    assert_eq!(sel.multi(), &list[2..=4]);
    // Back past the anchor: the range flips around it.
    for _ in 0..3 {
        cursor = sel.step(&list, cursor, -1, true);
    }
    assert_eq!(cursor, Some(list[1]));
    assert_eq!(sel.multi(), &list[1..=2]);
    // A plain move resets the anchor and drops the multi-selection.
    cursor = sel.step(&list, cursor, 1, false);
    assert_eq!(cursor, Some(list[2]));
    assert!(sel.multi().is_empty());
    assert_eq!(sel.targets(cursor), vec![list[2]]);
}

#[test]
fn steps_clamp_and_start_from_the_ends_without_a_cursor() {
    let list = ids(20);
    let mut sel = Selection::default();
    assert_eq!(sel.step(&list, None, 1, false), Some(list[0]));
    assert_eq!(sel.step(&list, None, -1, false), Some(list[19]));
    assert_eq!(sel.step(&list, Some(list[3]), 8, false), Some(list[11]));
    assert_eq!(sel.step(&list, Some(list[3]), -8, false), Some(list[0]));
    assert_eq!(sel.step(&list, Some(list[3]), -20, false), Some(list[0]));
    assert_eq!(sel.step(&list, Some(list[3]), 20, false), Some(list[19]));
    assert_eq!(sel.step(&[], Some(list[3]), 1, false), None);
}

#[test]
fn command_click_toggles_from_the_cursor_and_shift_click_takes_a_range() {
    let list = ids(6);
    let mut sel = Selection::default();
    let toggle = ClickMods {
        toggle: true,
        range: false,
    };
    let range = ClickMods {
        toggle: false,
        range: true,
    };
    let mut cursor = Some(sel.click(&list, None, list[0], ClickMods::default()));
    cursor = Some(sel.click(&list, cursor, list[1], toggle));
    assert_eq!(cursor, Some(list[1]));
    assert_eq!(sel.multi(), &list[0..=1]);
    assert_eq!(sel.bulk(), &list[0..=1]);
    assert!(sel.includes(list[0], cursor) && sel.includes(list[1], cursor));
    // ⌘-click again removes it; one left is not a bulk selection.
    cursor = Some(sel.click(&list, cursor, list[1], toggle));
    assert_eq!(sel.multi(), &list[0..1]);
    assert!(sel.bulk().is_empty());
    assert_eq!(sel.targets(cursor), vec![list[1]]);
    // Shift-click ranges from the anchor (the last plain click), in list order.
    cursor = Some(sel.click(&list, cursor, list[4], range));
    assert_eq!(sel.multi(), &list[0..=4]);
    assert_eq!(sel.targets(cursor), list[0..=4].to_vec());
    // ⌘A takes every row; Escape clears the multi-selection only.
    sel.select_all(&list);
    assert_eq!(sel.multi().len(), 6);
    assert!(sel.clear());
    assert!(!sel.clear());
    assert_eq!(sel.targets(cursor), vec![list[4]]);
}

#[test]
fn a_selection_stays_valid_inside_the_list() {
    let list = ids(4);
    let outside = Uuid::from_u128(99);
    let exists = |id: Uuid| id == outside || list.contains(&id);
    let all = LibraryFilter::All;
    // In the list: kept.
    assert_eq!(
        keep_valid(Some(list[2]), &list, "", &all, exists),
        Some(list[2])
    );
    // Nothing selected: the first row, also while filtering.
    assert_eq!(keep_valid(None, &list, "", &all, exists), Some(list[0]));
    assert_eq!(keep_valid(None, &list, "x", &all, exists), Some(list[0]));
    // A filter keeps the editor on a snippet the results left out.
    assert_eq!(
        keep_valid(Some(outside), &list, "x", &all, exists),
        Some(outside)
    );
    assert_eq!(
        keep_valid(Some(outside), &list, "x", &LibraryFilter::Favorites, exists),
        Some(outside)
    );
    // Without one: All keeps a snippet that exists; another view selects its first row.
    assert_eq!(
        keep_valid(Some(outside), &list, "", &all, exists),
        Some(outside)
    );
    assert_eq!(
        keep_valid(Some(outside), &list, "", &LibraryFilter::Favorites, exists),
        Some(list[0])
    );
    let gone = Uuid::from_u128(100);
    assert_eq!(
        keep_valid(Some(gone), &list, "", &all, exists),
        Some(list[0])
    );
    // An empty list leaves the selection alone.
    assert_eq!(keep_valid(Some(gone), &[], "", &all, exists), Some(gone));
    assert_eq!(keep_valid(None, &[], "", &all, exists), None);
}

#[test]
fn a_deleted_id_leaves_the_multi_selection() {
    let list = ids(4);
    let mut sel = Selection::default();
    sel.select_all(&list);
    sel.retain(|id| id != list[1]);
    assert_eq!(sel.multi(), [list[0], list[2], list[3]]);
}
