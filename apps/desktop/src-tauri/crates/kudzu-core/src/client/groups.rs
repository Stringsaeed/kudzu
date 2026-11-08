//! Group tree helpers (`lib/groups.ts`): nesting, ordering, paths, effective enablement and
//! counts. Dangling or self parents count as roots; cycles and repeated ids never loop.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{GroupRecord, SnippetRecord};

use super::js;

/// A group in the sidebar tree.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupNode<'a> {
    pub group: &'a GroupRecord,
    pub depth: usize,
    pub children: Vec<GroupNode<'a>>,
}

/// Groups nested under their parents, siblings by `sort_index` then name. A group whose
/// parent is missing (or itself) is a root; groups only reachable through a cycle are left
/// out, and a repeated id appears once (its first sibling in order).
pub fn build_tree(groups: &[GroupRecord]) -> Vec<GroupNode<'_>> {
    let ids: HashSet<Uuid> = groups.iter().map(|g| g.id).collect();
    let mut by_parent: HashMap<Option<Uuid>, Vec<&GroupRecord>> = HashMap::new();
    for g in groups {
        let parent = g.parent_id.filter(|p| ids.contains(p) && *p != g.id);
        by_parent.entry(parent).or_default().push(g);
    }
    fn walk<'a>(by_parent: &HashMap<Option<Uuid>, Vec<&'a GroupRecord>>, seen: &mut HashSet<Uuid>, parent: Option<Uuid>, depth: usize) -> Vec<GroupNode<'a>> {
        let mut list = by_parent.get(&parent).cloned().unwrap_or_default();
        list.sort_by(|a, b| a.sort_index.cmp(&b.sort_index).then_with(|| js::locale_compare(&a.name, &b.name)));
        // Siblings are all claimed before descending, as `.filter(...).map(...)` does.
        let level: Vec<&GroupRecord> = list.into_iter().filter(|g| seen.insert(g.id)).collect();
        level.into_iter().map(|g| GroupNode { group: g, depth, children: walk(by_parent, seen, Some(g.id), depth + 1) }).collect()
    }
    walk(&by_parent, &mut HashSet::new(), None, 0)
}

/// Depth-first rows, skipping the children of collapsed groups.
pub fn flatten<'n, 'a>(nodes: &'n [GroupNode<'a>], collapsed: Option<&HashSet<Uuid>>) -> Vec<&'n GroupNode<'a>> {
    fn visit<'n, 'a>(list: &'n [GroupNode<'a>], collapsed: Option<&HashSet<Uuid>>, out: &mut Vec<&'n GroupNode<'a>>) {
        for n in list {
            out.push(n);
            if !collapsed.is_some_and(|c| c.contains(&n.group.id)) {
                visit(&n.children, collapsed, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(nodes, collapsed, &mut out);
    out
}

/// `id` and every group nested under it.
pub fn descendant_ids(groups: &[GroupRecord], id: Uuid) -> HashSet<Uuid> {
    let mut out = HashSet::from([id]);
    let mut grew = true;
    while grew {
        grew = false;
        for g in groups {
            if let Some(parent) = g.parent_id {
                if out.contains(&parent) && !out.contains(&g.id) {
                    out.insert(g.id);
                    grew = true;
                }
            }
        }
    }
    out
}

// `new Map(groups.map((g) => [g.id, g]))`: the last group with an id wins.
fn by_id(groups: &[GroupRecord]) -> HashMap<Uuid, &GroupRecord> {
    groups.iter().map(|g| (g.id, g)).collect()
}

/// Names from the root down to `id` (empty when unknown).
pub fn group_path(groups: &[GroupRecord], id: Option<Uuid>) -> Vec<String> {
    let by_id = by_id(groups);
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    let mut cur = id.and_then(|id| by_id.get(&id).copied());
    while let Some(g) = cur {
        if !seen.insert(g.id) {
            break;
        }
        path.insert(0, g.name.clone());
        cur = g.parent_id.and_then(|p| by_id.get(&p).copied());
    }
    path
}

/// Whether `id` and all its ancestors are enabled (an unknown or absent group is).
pub fn is_group_enabled(groups: &[GroupRecord], id: Option<Uuid>) -> bool {
    let by_id = by_id(groups);
    let mut seen = HashSet::new();
    let mut cur = id.and_then(|id| by_id.get(&id).copied());
    while let Some(g) = cur {
        if seen.contains(&g.id) {
            break;
        }
        if !g.is_enabled {
            return false;
        }
        seen.insert(g.id);
        cur = g.parent_id.and_then(|p| by_id.get(&p).copied());
    }
    true
}

/// Snippets per group, counting each snippet in its group and every ancestor.
pub fn recursive_counts(groups: &[GroupRecord], snippets: &[SnippetRecord]) -> HashMap<Uuid, usize> {
    let parent: HashMap<Uuid, Option<Uuid>> = groups.iter().map(|g| (g.id, g.parent_id)).collect();
    let mut counts = HashMap::new();
    for s in snippets {
        let mut seen = HashSet::new();
        let mut g = s.group_id;
        while let Some(id) = g {
            if !parent.contains_key(&id) || !seen.insert(id) {
                break;
            }
            *counts.entry(id).or_insert(0) += 1;
            g = parent[&id];
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn group(n: u128, name: &str, parent: Option<u128>, sort: i64) -> GroupRecord {
        GroupRecord { id: id(n), name: name.into(), parent_id: parent.map(id), sort_index: sort, ..GroupRecord::new(name) }
    }

    fn names(nodes: &[&GroupNode<'_>]) -> Vec<(String, usize)> {
        nodes.iter().map(|n| (n.group.name.clone(), n.depth)).collect()
    }

    #[test]
    fn builds_an_ordered_tree() {
        let groups = vec![
            group(1, "Work", None, 1),
            group(2, "personal", None, 0),
            group(3, "Email", Some(1), 0),
            group(4, "apple", Some(1), 0),
            group(5, "Deep", Some(3), 0),
            group(6, "Orphan", Some(99), 0),
            group(7, "Self", Some(7), 2),
            group(8, "CycleA", Some(9), 0),
            group(9, "CycleB", Some(8), 0),
        ];
        let tree = build_tree(&groups);
        let rows = flatten(&tree, None);
        assert_eq!(names(&rows), [
            ("Orphan".to_string(), 0),
            ("personal".into(), 0),
            ("Work".into(), 0),
            ("apple".into(), 1),
            ("Email".into(), 1),
            ("Deep".into(), 2),
            ("Self".into(), 0),
        ]);
        let collapsed = HashSet::from([id(1)]);
        assert_eq!(flatten(&tree, Some(&collapsed)).len(), 4);
    }

    #[test]
    fn repeated_ids_appear_once() {
        let groups = vec![group(1, "B", None, 0), group(1, "A", None, 0), group(2, "Child", Some(1), 0)];
        let tree = build_tree(&groups);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].group.name, "A");
        assert_eq!(tree[0].children.len(), 1);
    }

    #[test]
    fn descendants_paths_and_enablement() {
        let mut groups = vec![group(1, "Work", None, 0), group(2, "Email", Some(1), 0), group(3, "Deep", Some(2), 0), group(4, "Other", None, 0)];
        assert_eq!(descendant_ids(&groups, id(1)), HashSet::from([id(1), id(2), id(3)]));
        assert_eq!(descendant_ids(&groups, id(42)), HashSet::from([id(42)]));
        assert_eq!(group_path(&groups, Some(id(3))), ["Work", "Email", "Deep"]);
        assert!(group_path(&groups, None).is_empty());
        assert!(group_path(&groups, Some(id(42))).is_empty());
        assert!(is_group_enabled(&groups, Some(id(3))));
        groups[0].is_enabled = false;
        assert!(!is_group_enabled(&groups, Some(id(3))));
        assert!(is_group_enabled(&groups, Some(id(4))));
        assert!(is_group_enabled(&groups, None));
        // Cycles terminate.
        let cyclic = vec![group(8, "A", Some(9), 0), group(9, "B", Some(8), 0)];
        assert_eq!(group_path(&cyclic, Some(id(8))), ["B", "A"]);
        assert!(is_group_enabled(&cyclic, Some(id(8))));
    }

    #[test]
    fn counts_include_ancestors() {
        let groups = vec![group(1, "Work", None, 0), group(2, "Email", Some(1), 0), group(8, "A", Some(9), 0), group(9, "B", Some(8), 0)];
        let snippet = |g: Option<u128>| SnippetRecord { group_id: g.map(id), ..SnippetRecord::new("s", vec![], "") };
        let snippets = vec![snippet(Some(2)), snippet(Some(1)), snippet(None), snippet(Some(77)), snippet(Some(8))];
        let counts = recursive_counts(&groups, &snippets);
        assert_eq!(counts.get(&id(1)), Some(&2));
        assert_eq!(counts.get(&id(2)), Some(&1));
        assert_eq!(counts.get(&id(8)), Some(&1));
        assert_eq!(counts.get(&id(9)), Some(&1));
        assert_eq!(counts.get(&id(77)), None);
    }
}
