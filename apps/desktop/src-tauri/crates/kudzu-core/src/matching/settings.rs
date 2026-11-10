//! Inheritance of enablement, app rules and insertion through the group tree.

use std::collections::HashMap;

use uuid::Uuid;

use super::MatchingConfig;
use crate::types::{AppRule, AppRuleMode, GroupRecord, InsertionMethod, SnippetRecord};

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
pub(crate) fn nearest_disabled_group(group_id: Option<Uuid>, groups: &[GroupRecord]) -> Option<Uuid> {
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

    /// Insertion for `bundle_id`: listed rule override ▸ snippet ▸ group ▸ global per-app ▸ default.
    pub fn insertion_for(
        &self,
        bundle_id: Option<&str>,
        config: &MatchingConfig,
    ) -> InsertionMethod {
        let rule_override = self.rule.as_ref().and_then(|rule| {
            let id = bundle_id?;
            rule.bundle_ids
                .iter()
                .any(|b| b == id)
                .then_some(rule.insertion)
                .flatten()
        });
        resolve_insertion(rule_override, self.insertion, bundle_id, config)
    }
}

/// The shared tail of the insertion precedence chain.
pub(crate) fn resolve_insertion(
    rule_override: Option<InsertionMethod>,
    own: Option<InsertionMethod>,
    bundle_id: Option<&str>,
    config: &MatchingConfig,
) -> InsertionMethod {
    rule_override
        .or(own)
        .or_else(|| bundle_id.and_then(|id| config.app_insertions.get(id).copied()))
        .unwrap_or(config.default_insertion)
}
