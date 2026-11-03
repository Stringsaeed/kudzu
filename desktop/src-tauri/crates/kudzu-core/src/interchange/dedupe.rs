//! Finds duplicates created by syncing or repeated imports. Pure, deterministic and idempotent.
//!
//! Every device runs this independently after remote changes, so the survivor choice depends
//! only on the data (never on input order or the device) and all devices converge.
//! UUIDs tie-break by their natural order, which matches comparing the Swift app's uppercase
//! UUID strings.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::stats::latest;
use crate::types::{iso8601_opt, GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};

/// Two or more surviving snippets share a trigger but expand to different content.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DedupeConflict {
    /// The shared trigger, lowercased.
    pub trigger: String,
    /// The snippets claiming it, sorted.
    pub snippet_ids: Vec<Uuid>,
}

/// Collapse every usage row for `device_id` whose snippet is in `source_snippet_ids` into one row.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageMerge {
    pub snippet_id: Uuid,
    pub device_id: String,
    /// Snippet IDs whose rows (for this device) are folded in, sorted. May include `snippet_id`.
    pub source_snippet_ids: Vec<Uuid>,
    pub count: i64,
    pub chars_saved: i64,
    #[serde(with = "iso8601_opt", default)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    let mut snippet_buckets: HashMap<(Vec<String>, &str), Vec<&SnippetRecord>> = HashMap::new();
    for s in snippets_by_id.values() {
        snippet_buckets
            .entry((normalized_triggers(&s.triggers), s.content.as_str()))
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
mod tests {
    use super::*;
    use crate::types::{FireMode, KudzuLibrary};
    use chrono::{Duration, TimeZone};

    fn uuid(n: u64) -> Uuid {
        Uuid::parse_str(&format!("00000000-0000-0000-0000-{n:012}")).unwrap()
    }

    fn day0() -> DateTime<Utc> {
        Utc.timestamp_opt(1_778_307_200, 0).unwrap()
    }

    fn snippet(id: u64, triggers: &[&str], content: &str, created: DateTime<Utc>) -> SnippetRecord {
        let mut s = SnippetRecord::new(
            id.to_string(),
            triggers.iter().map(|t| t.to_string()).collect(),
            content,
        );
        s.id = uuid(id);
        s.created_at = created;
        s.updated_at = created;
        s
    }

    fn group(id: u64, name: &str, parent: Option<u64>) -> GroupRecord {
        GroupRecord {
            id: uuid(id),
            parent_id: parent.map(uuid),
            ..GroupRecord::new(name)
        }
    }

    fn usage(
        id: Uuid,
        device: &str,
        count: i64,
        chars: i64,
        last: Option<DateTime<Utc>>,
    ) -> UsageRecord {
        UsageRecord {
            snippet_id: id,
            device_id: device.into(),
            count,
            chars_saved: chars,
            last_used_at: last,
        }
    }

    fn lib(
        groups: Vec<GroupRecord>,
        snippets: Vec<SnippetRecord>,
        usage: Vec<UsageRecord>,
    ) -> KudzuLibrary {
        KudzuLibrary {
            groups,
            snippets,
            usage,
            ..KudzuLibrary::default()
        }
    }

    fn no_aliases() -> HashMap<Uuid, Uuid> {
        HashMap::new()
    }

    #[test]
    fn empty_library_has_empty_plan() {
        let p = plan(&KudzuLibrary::default(), &no_aliases());
        assert!(p.is_empty());
        assert!(p.conflicts.is_empty());
    }

    #[test]
    fn merges_snippets_with_same_normalized_triggers_and_content() {
        let mut older = snippet(9, &[";sig", ";s"], "Best", day0());
        older.name = "Sig".into();
        older.is_enabled = false;
        let mut newer = snippet(1, &[";S", ";SIG"], "Best", day0() + Duration::seconds(10));
        newer.is_favorite = true;
        newer.updated_at = day0() + Duration::seconds(20);
        let library = lib(vec![], vec![newer.clone(), older.clone()], vec![]);

        let p = plan(&library, &no_aliases());
        assert_eq!(p.snippet_merges, BTreeMap::from([(newer.id, older.id)]));
        let survivor = &p.updated_snippets[0];
        assert_eq!(survivor.id, older.id);
        assert_eq!(survivor.name, "Sig");
        assert_eq!(survivor.triggers, vec![";sig", ";s"]);
        assert!(survivor.is_favorite && survivor.is_enabled);
        assert_eq!(survivor.created_at, day0());
        assert_eq!(survivor.updated_at, day0() + Duration::seconds(20));
        assert_eq!(
            p.applied(&library)
                .snippets
                .iter()
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            vec![older.id]
        );
    }

    #[test]
    fn created_at_tie_breaks_on_smallest_uuid() {
        let a = snippet(2, &[";x"], "X", day0());
        let b = snippet(1, &[";x"], "X", day0());
        let p = plan(
            &lib(vec![], vec![a.clone(), b.clone()], vec![]),
            &no_aliases(),
        );
        assert_eq!(p.snippet_merges, BTreeMap::from([(a.id, b.id)]));
    }

    #[test]
    fn duplicate_triggers_within_a_snippet_are_a_set() {
        let a = snippet(1, &[";x", ";x"], "X", day0());
        let b = snippet(2, &[";X"], "X", day0());
        let p = plan(
            &lib(vec![], vec![a.clone(), b.clone()], vec![]),
            &no_aliases(),
        );
        assert_eq!(p.snippet_merges, BTreeMap::from([(b.id, a.id)]));
    }

    #[test]
    fn different_content_is_reported_as_conflict_not_merged() {
        let a = snippet(1, &[";addr", ";home"], "1 Main St", day0());
        let b = snippet(2, &[";ADDR"], "2 Side St", day0());
        let c = snippet(3, &[";other"], "x", day0());
        let p = plan(
            &lib(vec![], vec![b.clone(), c, a.clone()], vec![]),
            &no_aliases(),
        );
        assert!(p.is_empty());
        assert_eq!(
            p.conflicts,
            vec![DedupeConflict {
                trigger: ";addr".into(),
                snippet_ids: vec![a.id, b.id]
            }]
        );
    }

    #[test]
    fn content_comparison_is_exact() {
        let a = snippet(1, &[";x"], "Hello", day0());
        let b = snippet(2, &[";x"], "hello", day0());
        let p = plan(&lib(vec![], vec![a, b], vec![]), &no_aliases());
        assert!(p.snippet_merges.is_empty());
        assert_eq!(p.conflicts.len(), 1);
    }

    #[test]
    fn usage_of_merged_snippets_is_repointed_and_summed_per_device() {
        let keep = snippet(1, &[";k"], "K", day0());
        let drop = snippet(2, &[";k"], "K", day0() + Duration::seconds(1));
        let rows = vec![
            usage(keep.id, "macA", 3, 30, Some(day0())),
            usage(
                drop.id,
                "macA",
                2,
                20,
                Some(day0() + Duration::seconds(100)),
            ),
            usage(drop.id, "macB", 5, 50, None),
        ];
        let library = lib(vec![], vec![keep.clone(), drop.clone()], rows);
        let p = plan(&library, &no_aliases());
        assert_eq!(
            p.usage_merges,
            vec![
                UsageMerge {
                    snippet_id: keep.id,
                    device_id: "macA".into(),
                    source_snippet_ids: vec![keep.id, drop.id],
                    count: 5,
                    chars_saved: 50,
                    last_used_at: Some(day0() + Duration::seconds(100)),
                },
                UsageMerge {
                    snippet_id: keep.id,
                    device_id: "macB".into(),
                    source_snippet_ids: vec![drop.id],
                    count: 5,
                    chars_saved: 50,
                    last_used_at: None,
                },
            ]
        );
        let result: HashSet<UsageRecord> = p.applied(&library).usage.into_iter().collect();
        assert_eq!(
            result,
            HashSet::from([
                usage(
                    keep.id,
                    "macA",
                    5,
                    50,
                    Some(day0() + Duration::seconds(100))
                ),
                usage(keep.id, "macB", 5, 50, None),
            ])
        );
    }

    #[test]
    fn duplicate_usage_rows_for_same_snippet_and_device_are_summed() {
        let s = snippet(1, &[";s"], "S", day0());
        let library = lib(
            vec![],
            vec![s.clone()],
            vec![usage(s.id, "m", 1, 1, None), usage(s.id, "m", 2, 2, None)],
        );
        let (result, report) = dedupe(&library, &no_aliases());
        assert_eq!(result.usage, vec![usage(s.id, "m", 3, 3, None)]);
        assert_eq!(report.merged_usage_rows, 1);
    }

    #[test]
    fn orphan_usage_is_left_alone_unless_an_alias_points_somewhere() {
        let s = snippet(1, &[";s"], "S", day0());
        let (ghost, aliased) = (uuid(50), uuid(51));
        let library = lib(
            vec![],
            vec![s.clone()],
            vec![
                usage(ghost, "m", 1, 0, None),
                usage(aliased, "m", 4, 0, None),
            ],
        );
        let p = plan(&library, &HashMap::from([(aliased, s.id)]));
        assert_eq!(
            p.usage_merges,
            vec![UsageMerge {
                snippet_id: s.id,
                device_id: "m".into(),
                source_snippet_ids: vec![aliased],
                count: 4,
                chars_saved: 0,
                last_used_at: None
            }]
        );
        assert!(p
            .applied(&library)
            .usage
            .iter()
            .any(|u| u.snippet_id == ghost));
    }

    #[test]
    fn merges_groups_by_case_insensitive_name_under_same_parent() {
        let work1 = group(1, "Work", None);
        let work2 = group(2, " work ", None);
        let personal = group(3, "Personal", None);
        let nested_work = group(4, "Work", Some(3));
        let mut s = snippet(10, &[";s"], "S", day0());
        s.group_id = Some(work2.id);
        let library = lib(
            vec![
                work2.clone(),
                personal.clone(),
                work1.clone(),
                nested_work.clone(),
            ],
            vec![s],
            vec![],
        );

        let p = plan(&library, &no_aliases());
        assert_eq!(p.group_merges, BTreeMap::from([(work2.id, work1.id)]));
        assert_eq!(
            p.updated_snippets
                .iter()
                .map(|s| s.group_id)
                .collect::<Vec<_>>(),
            vec![Some(work1.id)]
        );
        let result = p.applied(&library);
        assert_eq!(
            result.groups.iter().map(|g| g.id).collect::<HashSet<_>>(),
            HashSet::from([work1.id, personal.id, nested_work.id])
        );
        assert_eq!(result.snippets[0].group_id, Some(work1.id));
    }

    #[test]
    fn group_merges_cascade_into_children() {
        let library = lib(
            vec![
                group(1, "Work", None),
                group(2, "Work", None),
                group(4, "Email", Some(1)),
                group(3, "Email", Some(2)),
                group(5, "Leaf", Some(3)),
            ],
            vec![],
            vec![],
        );
        let p = plan(&library, &no_aliases());
        assert_eq!(
            p.group_merges,
            BTreeMap::from([(uuid(2), uuid(1)), (uuid(4), uuid(3))])
        );
        let result = p.applied(&library);
        assert_eq!(result.groups.len(), 3);
        assert_eq!(
            result
                .groups
                .iter()
                .find(|g| g.id == uuid(3))
                .unwrap()
                .parent_id,
            Some(uuid(1))
        );
        assert_eq!(
            result
                .groups
                .iter()
                .find(|g| g.id == uuid(5))
                .unwrap()
                .parent_id,
            Some(uuid(3))
        );
    }

    fn messy_library() -> KudzuLibrary {
        let grouped = |mut s: SnippetRecord, g: Option<u64>| {
            s.group_id = g.map(uuid);
            s
        };
        let mut favorite = grouped(
            snippet(11, &[";X"], "X", day0() - Duration::seconds(5)),
            Some(4),
        );
        favorite.is_favorite = true;
        let mut disabled = grouped(snippet(12, &[";x"], "X", day0()), Some(2));
        disabled.is_enabled = false;
        lib(
            vec![
                group(1, "A", None),
                group(2, "a", None),
                group(3, "B", Some(2)),
                group(4, "b", Some(1)),
            ],
            vec![
                grouped(snippet(10, &[";x"], "X", day0()), Some(3)),
                favorite,
                disabled,
                snippet(13, &[";x"], "Y", day0()),
                grouped(snippet(14, &[";y"], "Y", day0()), Some(2)),
            ],
            vec![
                usage(uuid(10), "d1", 1, 1, Some(day0())),
                usage(uuid(11), "d1", 2, 2, Some(day0() + Duration::seconds(9))),
                usage(uuid(12), "d2", 3, 3, None),
                usage(uuid(14), "d2", 1, 1, None),
            ],
        )
    }

    #[test]
    fn is_idempotent_and_order_independent() {
        let library = messy_library();
        let p = plan(&library, &no_aliases());
        let once = p.applied(&library);
        let second = plan(&once, &no_aliases());
        assert!(second.is_empty());
        assert_eq!(second.applied(&once), once);

        let mut shuffled = library.clone();
        shuffled.groups.reverse();
        shuffled.snippets.reverse();
        shuffled.usage.reverse();
        assert_eq!(plan(&shuffled, &no_aliases()), p);
    }

    #[test]
    fn same_espanso_config_imported_on_two_macs_converges() {
        let import = |base: u64, at: DateTime<Utc>| {
            let g = group(base, "base", None);
            let mk = |n: u64, triggers: &[&str], content: &str| {
                let mut s = snippet(base + n, triggers, content, at);
                s.group_id = Some(g.id);
                s
            };
            let mut date = mk(2, &[":date"], "{{date}}");
            date.fire_mode = FireMode::WholeWord;
            (
                g.clone(),
                vec![
                    mk(1, &[":sig"], "Regards"),
                    date,
                    mk(3, &[":addr", ":address"], "1 Main St"),
                ],
            )
        };
        let (ga, sa) = import(100, day0());
        let (gb, sb) = import(200, day0() + Duration::hours(1));
        let rows = vec![
            usage(uuid(101), "A", 4, 28, Some(day0() + Duration::seconds(10))),
            usage(
                uuid(201),
                "B",
                6,
                42,
                Some(day0() + Duration::seconds(7200)),
            ),
            usage(uuid(203), "B", 1, 9, Some(day0() + Duration::seconds(7300))),
        ];
        let synced = lib(
            vec![ga.clone(), gb.clone()],
            [sa.clone(), sb.clone()].concat(),
            rows.clone(),
        );
        let p = plan(&synced, &no_aliases());
        assert_eq!(p.group_merges, BTreeMap::from([(uuid(200), uuid(100))]));
        assert_eq!(
            p.snippet_merges,
            BTreeMap::from([
                (uuid(201), uuid(101)),
                (uuid(202), uuid(102)),
                (uuid(203), uuid(103))
            ])
        );
        assert!(p.conflicts.is_empty());

        let result = p.applied(&synced);
        assert_eq!(
            result.groups.iter().map(|g| g.id).collect::<Vec<_>>(),
            vec![uuid(100)]
        );
        assert!(result
            .snippets
            .iter()
            .all(|s| s.group_id == Some(uuid(100))));
        let before = crate::interchange::stats::summary(&synced.usage, &synced.snippets, 40);
        let after = crate::interchange::stats::summary(&result.usage, &result.snippets, 40);
        assert_eq!(after.total_expansions, before.total_expansions);
        assert_eq!(after.total_chars_saved, before.total_chars_saved);

        let mut reversed_rows = rows;
        reversed_rows.reverse();
        let other = lib(vec![gb, ga], [sb, sa].concat(), reversed_rows);
        assert_eq!(plan(&other, &no_aliases()), p);
        assert!(plan(&result, &no_aliases()).is_empty());
    }
}
