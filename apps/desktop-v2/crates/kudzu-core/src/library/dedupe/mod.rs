//! Finds duplicates created by syncing or repeated imports. Pure, deterministic and idempotent.
//!
//! Every device runs this independently after remote changes, so the survivor choice depends
//! only on the data (never on input order or the device) and all devices converge.
//! UUIDs tie-break by their natural order, which matches comparing the Swift app's uppercase
//! UUID strings.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::model::{GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};
use super::usage::latest;

/// Two or more surviving snippets share a trigger but expand to different content.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DedupeConflict {
    /// The shared trigger, lowercased.
    pub trigger: String,
    /// The snippets claiming it, sorted.
    pub snippet_ids: Vec<Uuid>,
}

/// Collapse every usage row for `device_id` whose snippet is in `source_snippet_ids` into one row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UsageMerge {
    pub snippet_id: Uuid,
    pub device_id: String,
    /// Snippet IDs whose rows (for this device) are folded in, sorted. May include `snippet_id`.
    pub source_snippet_ids: Vec<Uuid>,
    pub count: i64,
    pub chars_saved: i64,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl UsageMerge {
    /// The single usage row this merge produces.
    pub fn record(&self) -> UsageRecord {
        UsageRecord {
            snippet_id: self.snippet_id,
            device_id: self.device_id.clone(),
            count: self.count,
            chars_saved: self.chars_saved,
            last_used_at: self.last_used_at,
        }
    }

    fn covers(&self, row: &UsageRecord) -> bool {
        row.device_id == self.device_id && self.source_snippet_ids.contains(&row.snippet_id)
    }
}

/// The changes a dedupe pass wants to make. Pure data; apply it with [`DedupePlan::applied`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DedupePlan {
    /// Loser group ID → surviving group ID.
    pub group_merges: BTreeMap<Uuid, Uuid>,
    /// Loser snippet ID → surviving snippet ID.
    pub snippet_merges: BTreeMap<Uuid, Uuid>,
    /// Surviving groups whose fields changed (re-parented), sorted by ID.
    pub updated_groups: Vec<GroupRecord>,
    /// Surviving snippets whose fields changed (merged flags, re-grouped), sorted by ID.
    pub updated_snippets: Vec<SnippetRecord>,
    /// Usage rows to collapse, sorted by snippet then device.
    pub usage_merges: Vec<UsageMerge>,
    /// Same trigger, different content. Informational: not resolved by the plan.
    pub conflicts: Vec<DedupeConflict>,
}

/// Counts of what a dedupe pass changed, plus conflicts for the UI to surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DedupeReport {
    pub merged_groups: usize,
    pub merged_snippets: usize,
    /// Number of (snippet, device) usage rows rewritten.
    pub merged_usage_rows: usize,
    pub conflicts: Vec<DedupeConflict>,
}

impl DedupeReport {
    /// True when nothing was merged (conflicts may still exist).
    pub fn is_empty(&self) -> bool {
        self.merged_groups == 0 && self.merged_snippets == 0 && self.merged_usage_rows == 0
    }
}

impl DedupePlan {
    /// True when applying the plan would change nothing (conflicts don't count).
    pub fn is_empty(&self) -> bool {
        self.group_merges.is_empty()
            && self.snippet_merges.is_empty()
            && self.updated_groups.is_empty()
            && self.updated_snippets.is_empty()
            && self.usage_merges.is_empty()
    }

    /// The counts and conflicts this plan represents.
    pub fn report(&self) -> DedupeReport {
        DedupeReport {
            merged_groups: self.group_merges.len(),
            merged_snippets: self.snippet_merges.len(),
            merged_usage_rows: self.usage_merges.len(),
            conflicts: self.conflicts.clone(),
        }
    }

    /// The library after applying the plan.
    pub fn applied(&self, library: &KudzuLibrary) -> KudzuLibrary {
        if self.is_empty() {
            return library.clone();
        }
        let mut group_updates: HashMap<Uuid, &GroupRecord> = HashMap::new();
        for g in &self.updated_groups {
            group_updates.entry(g.id).or_insert(g);
        }
        let mut snippet_updates: HashMap<Uuid, &SnippetRecord> = HashMap::new();
        for s in &self.updated_snippets {
            snippet_updates.entry(s.id).or_insert(s);
        }
        let groups = library
            .groups
            .iter()
            .filter(|g| !self.group_merges.contains_key(&g.id))
            .map(|g| {
                group_updates
                    .get(&g.id)
                    .map_or_else(|| g.clone(), |u| (*u).clone())
            })
            .collect();
        let snippets = library
            .snippets
            .iter()
            .filter(|s| !self.snippet_merges.contains_key(&s.id))
            .map(|s| {
                snippet_updates
                    .get(&s.id)
                    .map_or_else(|| s.clone(), |u| (*u).clone())
            })
            .collect();
        let usage = library
            .usage
            .iter()
            .filter(|row| !self.usage_merges.iter().any(|m| m.covers(row)))
            .cloned()
            .chain(self.usage_merges.iter().map(UsageMerge::record))
            .collect();
        KudzuLibrary {
            format_version: library.format_version,
            groups,
            snippets,
            usage,
        }
    }
}

/// Triggers as a set: lowercased, de-duplicated and sorted.
pub fn normalized_triggers(triggers: &[String]) -> Vec<String> {
    triggers
        .iter()
        .map(|t| t.to_lowercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalized_group_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Computes the merge plan for `library`.
///
/// `aliases` maps a former snippet ID to the snippet it was merged into by an earlier pass
/// (possibly on another device); it re-points usage rows that arrived after their snippet was
/// already merged away.
pub fn plan(library: &KudzuLibrary, aliases: &HashMap<Uuid, Uuid>) -> DedupePlan {
    let mut plan = DedupePlan::default();

    // Groups. Merging two parents can make their children siblings with equal names, so repeat
    // until nothing changes.
    let mut groups_by_id: HashMap<Uuid, &GroupRecord> = HashMap::new();
    for g in &library.groups {
        groups_by_id.entry(g.id).or_insert(g);
    }
    let mut group_merges: HashMap<Uuid, Uuid> = HashMap::new();
    let resolve = |merges: &HashMap<Uuid, Uuid>, id: Uuid| {
        let mut current = id;
        while let Some(&next) = merges.get(&current) {
            current = next;
        }
        current
    };
    loop {
        let mut buckets: HashMap<(Option<Uuid>, String), Vec<Uuid>> = HashMap::new();
        for g in groups_by_id
            .values()
            .filter(|g| !group_merges.contains_key(&g.id))
        {
            let key = (
                g.parent_id.map(|p| resolve(&group_merges, p)),
                normalized_group_name(&g.name),
            );
            buckets.entry(key).or_default().push(g.id);
        }
        let mut changed = false;
        for mut ids in buckets.into_values().filter(|ids| ids.len() > 1) {
            ids.sort();
            for &loser in &ids[1..] {
                group_merges.insert(loser, ids[0]);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    plan.group_merges = group_merges
        .keys()
        .map(|&k| (k, resolve(&group_merges, k)))
        .collect();
    plan.updated_groups = groups_by_id
        .values()
        .filter(|g| !group_merges.contains_key(&g.id))
        .filter_map(|g| {
            let parent = g.parent_id.map(|p| resolve(&group_merges, p));
            (parent != g.parent_id).then(|| GroupRecord {
                parent_id: parent,
                ..(*g).clone()
            })
        })
        .collect();
    plan.updated_groups.sort_by_key(|g| g.id);

    // Snippets: identical normalized trigger set and content.
    let mut snippets_by_id: HashMap<Uuid, &SnippetRecord> = HashMap::new();
    for s in &library.snippets {
        snippets_by_id.entry(s.id).or_insert(s);
    }
    let mut snippet_buckets: HashMap<_, Vec<&SnippetRecord>> = HashMap::new();
    for s in snippets_by_id.values() {
        snippet_buckets
            .entry((
                normalized_triggers(&s.triggers),
                s.content.as_str(),
                s.formatting.as_ref(),
            ))
            .or_default()
            .push(s);
    }
    let mut survivors: Vec<SnippetRecord> = Vec::new();
    for mut bucket in snippet_buckets.into_values() {
        bucket.sort_by(|l, r| l.created_at.cmp(&r.created_at).then(l.id.cmp(&r.id)));
        let original = bucket[0];
        let mut survivor = original.clone();
        for loser in &bucket[1..] {
            plan.snippet_merges.insert(loser.id, survivor.id);
            survivor.is_favorite |= loser.is_favorite;
            survivor.is_enabled |= loser.is_enabled;
            survivor.updated_at = survivor.updated_at.max(loser.updated_at);
        }
        survivor.group_id = survivor.group_id.map(|g| resolve(&group_merges, g));
        if &survivor != original {
            plan.updated_snippets.push(survivor.clone());
        }
        survivors.push(survivor);
    }
    plan.updated_snippets.sort_by_key(|s| s.id);

    // Conflicts among survivors: a trigger claimed by different content.
    let mut claims: BTreeMap<String, BTreeSet<Uuid>> = BTreeMap::new();
    for s in &survivors {
        for t in normalized_triggers(&s.triggers) {
            claims.entry(t).or_default().insert(s.id);
        }
    }
    plan.conflicts = claims
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(trigger, ids)| DedupeConflict {
            trigger,
            snippet_ids: ids.into_iter().collect(),
        })
        .collect();

    // Usage: re-point to survivors and fold rows sharing (snippet, device).
    let live: HashSet<Uuid> = snippets_by_id.keys().copied().collect();
    let canonical = |id: Uuid| {
        let mut current = id;
        let mut seen = HashSet::new();
        while !live.contains(&current) {
            match aliases.get(&current) {
                Some(&next) if seen.insert(current) => current = next,
                _ => break,
            }
        }
        // An orphan row (snippet not synced yet, or alias to nowhere) stays as is.
        if !live.contains(&current) {
            return id;
        }
        plan.snippet_merges
            .get(&current)
            .copied()
            .unwrap_or(current)
    };
    let mut usage_buckets: HashMap<(Uuid, &str), Vec<&UsageRecord>> = HashMap::new();
    for row in &library.usage {
        usage_buckets
            .entry((canonical(row.snippet_id), row.device_id.as_str()))
            .or_default()
            .push(row);
    }
    let mut usage_merges: Vec<UsageMerge> = usage_buckets
        .into_iter()
        .filter(|((snippet, _), rows)| rows.len() > 1 || rows[0].snippet_id != *snippet)
        .map(|((snippet, device), rows)| UsageMerge {
            snippet_id: snippet,
            device_id: device.to_string(),
            source_snippet_ids: rows
                .iter()
                .map(|r| r.snippet_id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            count: rows.iter().map(|r| r.count).sum(),
            chars_saved: rows.iter().map(|r| r.chars_saved).sum(),
            last_used_at: rows.iter().fold(None, |acc, r| latest(acc, r.last_used_at)),
        })
        .collect();
    usage_merges.sort_by(|l, r| {
        l.snippet_id
            .cmp(&r.snippet_id)
            .then_with(|| l.device_id.cmp(&r.device_id))
    });
    plan.usage_merges = usage_merges;
    plan
}

/// Plans and applies a dedupe pass, returning the cleaned library and what changed.
pub fn dedupe(
    library: &KudzuLibrary,
    aliases: &HashMap<Uuid, Uuid>,
) -> (KudzuLibrary, DedupeReport) {
    let plan = plan(library, aliases);
    (plan.applied(library), plan.report())
}

#[cfg(test)]
mod tests;
