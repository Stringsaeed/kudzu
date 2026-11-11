//! Duplicate and shadowed triggers, for editor warnings.
//!
//! Triggers compare the way the matcher sees them, so the key folding comes from `matching`.

use std::collections::{HashMap, HashSet};

use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

use super::groups::{EffectiveSnippet, GroupResolver};
use super::model::{AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, SnippetRecord};
use crate::matching::case_propagation::is_cased;
use crate::matching::key_buffer::{folded_key, nfc};

/// Kind of trigger conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConflictKind {
    Duplicate,
    Shadowing,
}

/// A problem between two snippets' triggers, for editor warnings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriggerConflict {
    pub kind: ConflictKind,
    /// For shadowing: the shadowing (shorter, immediate) snippet.
    pub snippet_id: Uuid,
    pub trigger: String,
    pub other_snippet_id: Uuid,
    pub other_trigger: String,
}

/// Finds duplicate and shadowed triggers among effectively enabled snippets.
pub fn find_conflicts(snippets: &[SnippetRecord], groups: &[GroupRecord]) -> Vec<TriggerConflict> {
    find(snippets, groups)
}

struct Item<'a> {
    id: Uuid,
    trigger: &'a str,
    /// NFC key per cluster.
    exact: Vec<String>,
    /// Folded key per cluster.
    folded: Vec<String>,
    /// Whether each cluster has letter case.
    cased: Vec<bool>,
    propagates: bool,
    immediate: bool,
    rule: Option<AppRule>,
}

/// Snippets whose rules never apply to the same app don't conflict; two triggers on the same
/// snippet never conflict; an exact shadower only counts against an exact longer trigger, or
/// when the shared prefix has no letter case.
fn find(snippets: &[SnippetRecord], groups: &[GroupRecord]) -> Vec<TriggerConflict> {
    let resolver = GroupResolver::new(groups);
    let mut items = Vec::new();
    for snippet in snippets {
        let effective = EffectiveSnippet::new(snippet, &resolver);
        if !effective.is_enabled {
            continue;
        }
        for trigger in snippet.triggers.iter().filter(|t| !t.is_empty()) {
            let clusters: Vec<&str> = trigger.graphemes(true).collect();
            items.push(Item {
                id: snippet.id,
                trigger,
                exact: clusters.iter().map(|c| nfc(c).into_owned()).collect(),
                folded: clusters.iter().map(|c| folded_key(c)).collect(),
                cased: clusters.iter().map(|c| is_cased(c)).collect(),
                propagates: snippet.case_mode == CaseMode::Propagate,
                immediate: snippet.fire_mode == FireMode::Immediate,
                rule: effective.rule.clone(),
            });
        }
    }
    let mut conflicts = duplicates(&items);
    conflicts.extend(shadowing(&items));
    conflicts
}

fn duplicates(items: &[Item<'_>]) -> Vec<TriggerConflict> {
    let mut buckets: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        buckets.entry(item.folded.concat()).or_default().push(i);
    }
    let mut found = Vec::new();
    for indices in buckets.values().filter(|v| v.len() > 1) {
        for (n, &i) in indices.iter().enumerate() {
            for &j in &indices[n + 1..] {
                let (a, b) = (&items[i], &items[j]);
                if a.id == b.id || !(a.propagates || b.propagates || a.exact == b.exact) {
                    continue;
                }
                if rules_can_overlap(a.rule.as_ref(), b.rule.as_ref()) {
                    found.push((i.min(j), i.max(j)));
                }
            }
        }
    }
    found.sort_unstable();
    found
        .into_iter()
        .map(|(i, j)| conflict(ConflictKind::Duplicate, &items[i], &items[j]))
        .collect()
}

fn shadowing(items: &[Item<'_>]) -> Vec<TriggerConflict> {
    let mut folded_shadowers: HashMap<String, Vec<usize>> = HashMap::new();
    let mut exact_shadowers: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate().filter(|(_, item)| item.immediate) {
        if item.propagates {
            folded_shadowers
                .entry(item.folded.concat())
                .or_default()
                .push(i);
        } else {
            exact_shadowers
                .entry(item.exact.concat())
                .or_default()
                .push(i);
        }
    }
    let mut found: Vec<(usize, usize)> = Vec::new();
    let mut seen = HashSet::new();
    for (j, longer) in items.iter().enumerate() {
        let count = longer.exact.len();
        let mut folded_prefix = String::new();
        let mut exact_prefix = String::new();
        let mut prefix_cased = false;
        for length in 1..count {
            folded_prefix.push_str(&longer.folded[length - 1]);
            exact_prefix.push_str(&longer.exact[length - 1]);
            prefix_cased |= longer.cased[length - 1];
            // A propagating shadower fires whatever case the longer trigger is typed in.
            for &i in folded_shadowers.get(&folded_prefix).into_iter().flatten() {
                if seen.insert((i, j)) {
                    found.push((i, j));
                }
            }
            // An exact shadower only fires on its exact text: it blocks an exact longer trigger,
            // or a propagating one whose prefix cannot be typed in any other case.
            if longer.propagates && prefix_cased {
                continue;
            }
            for &i in exact_shadowers.get(&exact_prefix).into_iter().flatten() {
                if seen.insert((i, j)) {
                    found.push((i, j));
                }
            }
        }
    }
    found.retain(|&(i, j)| {
        items[i].id != items[j].id
            && rules_can_overlap(items[i].rule.as_ref(), items[j].rule.as_ref())
    });
    found.sort_unstable_by_key(|&(shadower, shadowed)| (shadowed, shadower));
    found
        .into_iter()
        .map(|(i, j)| conflict(ConflictKind::Shadowing, &items[i], &items[j]))
        .collect()
}

fn conflict(kind: ConflictKind, a: &Item<'_>, b: &Item<'_>) -> TriggerConflict {
    TriggerConflict {
        kind,
        snippet_id: a.id,
        trigger: a.trigger.to_string(),
        other_snippet_id: b.id,
        other_trigger: b.trigger.to_string(),
    }
}

/// Whether some app exists where both rules allow expansion. No rule means every app.
fn rules_can_overlap(a: Option<&AppRule>, b: Option<&AppRule>) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return true;
    };
    let a_ids: HashSet<&String> = a.bundle_ids.iter().collect();
    let b_ids: HashSet<&String> = b.bundle_ids.iter().collect();
    match (a.mode, b.mode) {
        (AppRuleMode::Allow, AppRuleMode::Allow) => !a_ids.is_disjoint(&b_ids),
        (AppRuleMode::Allow, AppRuleMode::Deny) => !a_ids.is_subset(&b_ids),
        (AppRuleMode::Deny, AppRuleMode::Allow) => !b_ids.is_subset(&a_ids),
        (AppRuleMode::Deny, AppRuleMode::Deny) => true,
    }
}
