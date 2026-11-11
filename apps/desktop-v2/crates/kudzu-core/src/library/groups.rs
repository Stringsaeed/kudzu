//! Inheritance of enablement, app rules and insertion through the group tree.
//!
//! Resolving an [`EffectiveSnippet`]'s insertion for a given app needs the matching
//! configuration, so that method lives in `matching::config`.
//!
//! The group tree for the sidebar and pickers ([`build_tree`], [`flatten`]), paths, descendant
//! sets, recursive counts and effective enablement are ported from the Tauri app's
//! `lib/groups.ts`. They tolerate cycles, dangling parents and duplicate ids the way the
//! TypeScript does; where a duplicate id appears the lookups differ on purpose from
//! [`GroupResolver`] (which keeps the first group): [`group_path`] and [`is_group_enabled`] keep
//! the *last* group with an id, [`build_tree`] the first in sibling order.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::model::{AppRule, AppRuleMode, GroupRecord, InsertionMethod, SnippetRecord};
use crate::search::locale_compare;

/// Human-readable title of an insertion method, for menus and descriptions.
pub fn insertion_title(method: InsertionMethod) -> &'static str {
    match method {
        InsertionMethod::Automatic => "Automatic",
        InsertionMethod::Type => "Type",
        InsertionMethod::Paste => "Paste",
    }
}

/// Summary of a rule such as "Block in Slack, Mail" or "Allow only in Terminal".
pub fn rule_summary(rule: &AppRule, app_name: &dyn Fn(&str) -> String) -> String {
    if rule.bundle_ids.is_empty() {
        return match rule.mode {
            AppRuleMode::Allow => "Disabled everywhere",
            AppRuleMode::Deny => "No restrictions",
        }
        .to_string();
    }
    let names = rule
        .bundle_ids
        .iter()
        .map(|id| app_name(id))
        .collect::<Vec<_>>()
        .join(", ");
    match rule.mode {
        AppRuleMode::Allow => format!("Allow only in {names}"),
        AppRuleMode::Deny => format!("Block in {names}"),
    }
}

/// Settings a snippet or subgroup inherits from its group ancestry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InheritedGroupSettings {
    pub is_enabled: bool,
    pub rule: Option<AppRule>,
    /// The nearest group in the ancestry that defined `rule`.
    pub rule_group: Option<GroupRecord>,
    pub insertion: Option<InsertionMethod>,
    /// The nearest group in the ancestry that defined `insertion`.
    pub insertion_group: Option<GroupRecord>,
}

static NO_INHERITANCE: InheritedGroupSettings = InheritedGroupSettings {
    is_enabled: true,
    rule: None,
    rule_group: None,
    insertion: None,
    insertion_group: None,
};

impl Default for InheritedGroupSettings {
    fn default() -> Self {
        NO_INHERITANCE.clone()
    }
}

impl InheritedGroupSettings {
    /// Describes the inherited app rule and its source group, or "No restrictions".
    pub fn rule_description(&self, app_name: &dyn Fn(&str) -> String) -> String {
        let Some(rule) = &self.rule else {
            return "No restrictions".into();
        };
        let summary = rule_summary(rule, app_name);
        match &self.rule_group {
            Some(group) => format!("Inherited from group '{}': {summary}", group.name),
            None => summary,
        }
    }

    /// Describes the inherited insertion method and its source group, or the global default.
    pub fn insertion_description(&self, default_insertion: InsertionMethod) -> String {
        match (self.insertion, &self.insertion_group) {
            (Some(insertion), Some(group)) => {
                format!(
                    "Inherited from group '{}': {}",
                    group.name,
                    insertion_title(insertion)
                )
            }
            (Some(insertion), None) => insertion_title(insertion).into(),
            (None, _) => format!(
                "Default ({}, or the app's setting in Settings → Apps)",
                insertion_title(default_insertion)
            ),
        }
    }
}

/// Resolves enablement, app rule and insertion through the group tree (`parent_id`),
/// tolerating cycles and dangling parent IDs.
#[derive(Debug, Clone, Default)]
pub struct GroupResolver {
    by_group: HashMap<Uuid, InheritedGroupSettings>,
    parent_settings_by_group: HashMap<Uuid, InheritedGroupSettings>,
}

impl GroupResolver {
    /// Builds the resolver; for duplicate IDs the first group wins.
    pub fn new(groups: &[GroupRecord]) -> Self {
        let mut records: HashMap<Uuid, &GroupRecord> = HashMap::with_capacity(groups.len());
        for group in groups {
            records.entry(group.id).or_insert(group);
        }
        let mut resolver = GroupResolver::default();
        for group in records.values() {
            resolver
                .by_group
                .insert(group.id, walk(Some(group), None, &records));
            let parent = group.parent_id.and_then(|id| records.get(&id).copied());
            resolver
                .parent_settings_by_group
                .insert(group.id, walk(parent, Some(group.id), &records));
        }
        resolver
    }

    /// Settings inherited by a snippet whose group is `group_id` (`None` or unknown → none).
    pub fn settings(&self, group_id: Option<Uuid>) -> &InheritedGroupSettings {
        group_id
            .and_then(|id| self.by_group.get(&id))
            .unwrap_or(&NO_INHERITANCE)
    }

    /// Settings inherited by `group_id` itself from its parent chain (excluding its own settings).
    pub fn inherited_settings_for_group_itself(&self, group_id: Uuid) -> &InheritedGroupSettings {
        self.parent_settings_by_group
            .get(&group_id)
            .unwrap_or(&NO_INHERITANCE)
    }

    /// The nearest rule set on `group_id` or an ancestor, with the group that sets it.
    pub fn inherited_rule(&self, group_id: Option<Uuid>) -> Option<(&AppRule, &GroupRecord)> {
        let settings = self.settings(group_id);
        Some((settings.rule.as_ref()?, settings.rule_group.as_ref()?))
    }

    /// The nearest insertion override on `group_id` or an ancestor, with the group that sets it.
    pub fn inherited_insertion(
        &self,
        group_id: Option<Uuid>,
    ) -> Option<(InsertionMethod, &GroupRecord)> {
        let settings = self.settings(group_id);
        Some((settings.insertion?, settings.insertion_group.as_ref()?))
    }
}

/// Walks child → root so the nearest ancestor wins for rule and insertion, while any disabled
/// ancestor disables the whole subtree.
fn walk(
    start: Option<&GroupRecord>,
    excluded: Option<Uuid>,
    records: &HashMap<Uuid, &GroupRecord>,
) -> InheritedGroupSettings {
    let mut result = InheritedGroupSettings::default();
    let mut visited: Vec<Uuid> = excluded.into_iter().collect();
    let mut current = start;
    while let Some(group) = current {
        if visited.contains(&group.id) {
            break;
        }
        visited.push(group.id);
        if !group.is_enabled {
            result.is_enabled = false;
        }
        if result.rule.is_none() {
            if let Some(rule) = &group.rule {
                result.rule = Some(rule.clone());
                result.rule_group = Some(group.clone());
            }
        }
        if result.insertion.is_none() {
            if let Some(insertion) = group.insertion {
                result.insertion = Some(insertion);
                result.insertion_group = Some(group.clone());
            }
        }
        current = group.parent_id.and_then(|id| records.get(&id).copied());
    }
    result
}

/// The nearest group at or above `group_id` that is turned off (cycle-safe).
pub(crate) fn nearest_disabled_group(
    group_id: Option<Uuid>,
    groups: &[GroupRecord],
) -> Option<Uuid> {
    let mut visited: Vec<Uuid> = Vec::new();
    let mut next = group_id;
    while let Some(id) = next {
        if visited.contains(&id) {
            break;
        }
        visited.push(id);
        let group = groups.iter().find(|g| g.id == id)?;
        if !group.is_enabled {
            return Some(id);
        }
        next = group.parent_id;
    }
    None
}

/// A snippet's effective, inheritance-resolved matching settings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EffectiveSnippet {
    pub is_enabled: bool,
    pub rule: Option<AppRule>,
    /// The group `rule` was inherited from; `None` if the snippet sets its own rule (or has none).
    pub rule_group: Option<GroupRecord>,
    /// Snippet insertion ▸ nearest ancestor group's; `None` falls back to global app/default.
    pub insertion: Option<InsertionMethod>,
    /// The group `insertion` was inherited from; `None` if the snippet sets its own (or has none).
    pub insertion_group: Option<GroupRecord>,
}

impl EffectiveSnippet {
    /// Resolves `snippet` against its group ancestry.
    pub fn new(snippet: &SnippetRecord, groups: &GroupResolver) -> Self {
        let inherited = groups.settings(snippet.group_id);
        let (rule, rule_group) = match &snippet.rule {
            Some(own) => (Some(own.clone()), None),
            None => (inherited.rule.clone(), inherited.rule_group.clone()),
        };
        let (insertion, insertion_group) = match snippet.insertion {
            Some(own) => (Some(own), None),
            None => (inherited.insertion, inherited.insertion_group.clone()),
        };
        EffectiveSnippet {
            is_enabled: snippet.is_enabled && inherited.is_enabled,
            rule,
            rule_group,
            insertion,
            insertion_group,
        }
    }
}

/// A group in the tree with its children, siblings ordered by sort index then name.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupNode<'a> {
    pub group: &'a GroupRecord,
    /// 0 for a top-level group.
    pub depth: usize,
    pub children: Vec<GroupNode<'a>>,
}

/// Arranges `groups` into a tree. A group whose parent is missing, or is itself, is top-level;
/// groups in a cycle are unreachable and left out; a repeated id appears once (the first in
/// sibling order).
pub fn build_tree(groups: &[GroupRecord]) -> Vec<GroupNode<'_>> {
    let ids: HashSet<Uuid> = groups.iter().map(|g| g.id).collect();
    let mut by_parent: HashMap<Option<Uuid>, Vec<&GroupRecord>> = HashMap::new();
    for g in groups {
        let parent = g.parent_id.filter(|p| ids.contains(p) && *p != g.id);
        by_parent.entry(parent).or_default().push(g);
    }
    for siblings in by_parent.values_mut() {
        siblings.sort_by(|a, b| {
            a.sort_index
                .cmp(&b.sort_index)
                .then_with(|| locale_compare(&a.name, &b.name))
        });
    }
    let mut seen = HashSet::new();
    walk_tree(&by_parent, None, 0, &mut seen)
}

fn walk_tree<'a>(
    by_parent: &HashMap<Option<Uuid>, Vec<&'a GroupRecord>>,
    parent: Option<Uuid>,
    depth: usize,
    seen: &mut HashSet<Uuid>,
) -> Vec<GroupNode<'a>> {
    let Some(siblings) = by_parent.get(&parent) else {
        return Vec::new();
    };
    // All siblings are claimed before any is descended into, as the TypeScript's filter-then-map
    // does.
    let claimed: Vec<&GroupRecord> = siblings
        .iter()
        .copied()
        .filter(|g| seen.insert(g.id))
        .collect();
    claimed
        .into_iter()
        .map(|group| GroupNode {
            group,
            depth,
            children: walk_tree(by_parent, Some(group.id), depth + 1, seen),
        })
        .collect()
}

/// The visible rows of the tree in display order: each node, then its children unless the node
/// is in `collapsed`.
pub fn flatten<'t, 'a>(
    nodes: &'t [GroupNode<'a>],
    collapsed: &HashSet<Uuid>,
) -> Vec<&'t GroupNode<'a>> {
    fn visit<'t, 'a>(
        nodes: &'t [GroupNode<'a>],
        collapsed: &HashSet<Uuid>,
        out: &mut Vec<&'t GroupNode<'a>>,
    ) {
        for node in nodes {
            out.push(node);
            if !collapsed.contains(&node.group.id) {
                visit(&node.children, collapsed, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(nodes, collapsed, &mut out);
    out
}

/// `id` and every group below it, however deep. Cycle-safe; an unknown `id` yields just itself.
pub fn descendant_ids(groups: &[GroupRecord], id: Uuid) -> HashSet<Uuid> {
    let mut out = HashSet::from([id]);
    let mut grew = true;
    while grew {
        grew = false;
        for g in groups {
            if g.parent_id.is_some_and(|p| out.contains(&p)) && out.insert(g.id) {
                grew = true;
            }
        }
    }
    out
}

/// Names from the top-level ancestor down to the group `id` itself; empty for `None` or an
/// unknown id. A cycle ends the walk at the first repeat.
pub fn group_path(groups: &[GroupRecord], id: Option<Uuid>) -> Vec<&str> {
    let by_id = last_by_id(groups);
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    let mut current = id.and_then(|id| by_id.get(&id));
    while let Some(group) = current.filter(|g| seen.insert(g.id)) {
        path.push(group.name.as_str());
        current = group.parent_id.and_then(|p| by_id.get(&p));
    }
    path.reverse();
    path
}

/// Whether the group and every ancestor are enabled; `None` and unknown ids count as enabled.
pub fn is_group_enabled(groups: &[GroupRecord], id: Option<Uuid>) -> bool {
    let by_id = last_by_id(groups);
    let mut seen = HashSet::new();
    let mut current = id.and_then(|id| by_id.get(&id));
    while let Some(group) = current.filter(|g| seen.insert(g.id)) {
        if !group.is_enabled {
            return false;
        }
        current = group.parent_id.and_then(|p| by_id.get(&p));
    }
    true
}

/// Snippets in each group, counting those in its subgroups too. Groups without snippets are
/// absent.
pub fn recursive_counts(
    groups: &[GroupRecord],
    snippets: &[SnippetRecord],
) -> HashMap<Uuid, usize> {
    let parent: HashMap<Uuid, Option<Uuid>> = groups.iter().map(|g| (g.id, g.parent_id)).collect();
    let mut counts = HashMap::new();
    for snippet in snippets {
        let mut seen = HashSet::new();
        let mut group = snippet.group_id;
        while let Some(id) = group.filter(|id| parent.contains_key(id) && !seen.contains(id)) {
            seen.insert(id);
            *counts.entry(id).or_insert(0) += 1;
            group = parent[&id];
        }
    }
    counts
}

fn last_by_id(groups: &[GroupRecord]) -> HashMap<Uuid, &GroupRecord> {
    groups.iter().map(|g| (g.id, g)).collect()
}
