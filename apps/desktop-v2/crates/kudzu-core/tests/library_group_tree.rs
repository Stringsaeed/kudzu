//! Group tree behaviour beyond the recorded fixtures: ordering and cycle safety.

use std::collections::HashSet;

use kudzu_core::library::groups::{
    build_tree, descendant_ids, flatten, group_path, recursive_counts,
};
use kudzu_core::{GroupRecord, SnippetRecord};

fn group(name: &str, sort_index: i64, parent: Option<&GroupRecord>) -> GroupRecord {
    let mut g = GroupRecord::new(name);
    g.sort_index = sort_index;
    g.parent_id = parent.map(|p| p.id);
    g
}

#[test]
fn siblings_order_by_sort_index_then_locale_name() {
    let groups = [
        group("beta", 1, None),
        group("Zed", 0, None),
        group("alpha", 1, None),
        group("Alpha", 1, None),
    ];
    let tree = build_tree(&groups);
    let names: Vec<&str> = tree.iter().map(|n| n.group.name.as_str()).collect();
    // sort index first, then ICU order: lowercase before uppercase, case ignored first.
    assert_eq!(names, ["Zed", "alpha", "Alpha", "beta"]);
}

#[test]
fn a_group_that_is_its_own_parent_is_top_level() {
    let mut lonely = group("Lonely", 0, None);
    lonely.parent_id = Some(lonely.id);
    let tree = build_tree(std::slice::from_ref(&lonely));
    assert_eq!(tree.len(), 1);
    assert_eq!(tree[0].depth, 0);
    assert_eq!(
        group_path(std::slice::from_ref(&lonely), Some(lonely.id)),
        ["Lonely"]
    );
}

#[test]
fn collapsing_hides_only_that_subtree() {
    let work = group("Work", 0, None);
    let email = group("Email", 0, Some(&work));
    let drafts = group("Drafts", 0, Some(&email));
    let home = group("Home", 1, None);
    let groups = [work.clone(), email.clone(), drafts, home.clone()];
    let tree = build_tree(&groups);
    let rows = |collapsed: &HashSet<_>| -> Vec<(&str, usize)> {
        flatten(&tree, collapsed)
            .iter()
            .map(|n| (n.group.name.as_str(), n.depth))
            .collect()
    };
    assert_eq!(
        rows(&HashSet::new()),
        [("Work", 0), ("Email", 1), ("Drafts", 2), ("Home", 0)]
    );
    assert_eq!(
        rows(&HashSet::from([email.id])),
        [("Work", 0), ("Email", 1), ("Home", 0)]
    );
    assert_eq!(rows(&HashSet::from([work.id])), [("Work", 0), ("Home", 0)]);
    assert_eq!(descendant_ids(&groups, work.id).len(), 3);
    assert_eq!(descendant_ids(&groups, home.id).len(), 1);
}

#[test]
fn counts_include_subgroups_and_survive_cycles() {
    let mut a = group("A", 0, None);
    let b = group("B", 0, Some(&a));
    a.parent_id = Some(b.id); // a cycle: A ↔ B
    let mut s = SnippetRecord::new("s", vec![";s".into()], "x");
    s.group_id = Some(b.id);
    let counts = recursive_counts(&[a.clone(), b.clone()], &[s]);
    assert_eq!(counts[&a.id], 1);
    assert_eq!(counts[&b.id], 1);
    assert!(build_tree(&[a, b]).is_empty());
}
