//! Adding an imported library to an existing one.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::library::dedupe::{self, DedupeReport};
use crate::library::model::{GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};
use crate::library::usage::latest;

/// Adds `incoming` to `existing`, then dedupes.
///
/// Records whose IDs already exist are kept unless the incoming snippet has a newer `updated_at`.
/// Usage rows for an existing (snippet, device) take the max of each counter: they are two
/// snapshots of the same counter (e.g. this device's own backup), so summing would double count.
/// Groups whose parent is missing, or whose parent link would form a cycle, become root groups.
pub fn merge_library(
    existing: &KudzuLibrary,
    incoming: &KudzuLibrary,
) -> (KudzuLibrary, DedupeReport) {
    let mut merged = existing.clone();

    // Groups: insert new ones without parents, then link parents that exist and don't loop.
    let mut parents: HashMap<Uuid, Option<Uuid>> =
        merged.groups.iter().map(|g| (g.id, g.parent_id)).collect();
    let mut inserted = Vec::new();
    for g in &incoming.groups {
        if parents.contains_key(&g.id) {
            continue;
        }
        parents.insert(g.id, None);
        inserted.push(merged.groups.len());
        merged.groups.push(GroupRecord {
            parent_id: None,
            ..g.clone()
        });
    }
    for index in inserted {
        let id = merged.groups[index].id;
        let wanted = incoming
            .groups
            .iter()
            .find(|g| g.id == id)
            .and_then(|g| g.parent_id);
        let Some(parent) = wanted.filter(|p| parents.contains_key(p)) else {
            continue;
        };
        if !is_descendant_or_self(parent, id, &parents) {
            parents.insert(id, Some(parent));
            merged.groups[index].parent_id = Some(parent);
        }
    }

    // Snippets: newer incoming edits win; group links must point at a known group.
    let known_group = |gid: Option<Uuid>| gid.filter(|g| parents.contains_key(g));
    let mut positions: HashMap<Uuid, usize> = HashMap::new();
    for (i, s) in merged.snippets.iter().enumerate() {
        positions.entry(s.id).or_insert(i);
    }
    for record in &incoming.snippets {
        let record = SnippetRecord {
            group_id: known_group(record.group_id),
            ..record.clone()
        };
        match positions.get(&record.id) {
            Some(&i) => {
                if record.updated_at > merged.snippets[i].updated_at {
                    merged.snippets[i] = record;
                }
            }
            None => {
                positions.insert(record.id, merged.snippets.len());
                merged.snippets.push(record);
            }
        }
    }

    // Usage: max of each counter for a known (snippet, device).
    let mut rows: HashMap<(Uuid, String), usize> = HashMap::new();
    for (i, u) in merged.usage.iter().enumerate() {
        rows.entry((u.snippet_id, u.device_id.clone())).or_insert(i);
    }
    for record in &incoming.usage {
        match rows.get(&(record.snippet_id, record.device_id.clone())) {
            Some(&i) => {
                let row = &mut merged.usage[i];
                *row = UsageRecord {
                    count: row.count.max(record.count),
                    chars_saved: row.chars_saved.max(record.chars_saved),
                    last_used_at: latest(row.last_used_at, record.last_used_at),
                    ..row.clone()
                };
            }
            None => {
                rows.insert(
                    (record.snippet_id, record.device_id.clone()),
                    merged.usage.len(),
                );
                merged.usage.push(record.clone());
            }
        }
    }

    dedupe::dedupe(&merged, &HashMap::new())
}

/// True if `candidate` is `group` or sits anywhere below it.
fn is_descendant_or_self(
    candidate: Uuid,
    group: Uuid,
    parents: &HashMap<Uuid, Option<Uuid>>,
) -> bool {
    let mut current = Some(candidate);
    let mut seen = HashSet::new();
    while let Some(node) = current.filter(|n| seen.insert(*n)) {
        if node == group {
            return true;
        }
        current = parents.get(&node).copied().flatten();
    }
    false
}
