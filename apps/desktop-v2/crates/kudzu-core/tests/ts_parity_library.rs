//! Group tree and usage statistics parity with the Tauri app's `lib/groups.ts` and
//! `lib/stats.ts`, from fixtures recorded by running that TypeScript (`groups.json`,
//! `group-edge-cases.json`, `stats.json` under `tests/fixtures/ts-parity/`, over the sample
//! library of `search-inputs.json`).
//!
//! The edge-case fixtures use ids such as `"a"` and `"missing"` that are not UUIDs; the tests
//! give each distinct string a stable UUID and translate back when comparing.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use kudzu_core::library::groups::{
    build_tree, descendant_ids, flatten, group_path, is_group_enabled, recursive_counts, GroupNode,
};
use kudzu_core::library::usage::{totals, usage_by_id};
use kudzu_core::{GroupRecord, KudzuLibrary};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

fn read(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ts-parity")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn sample_library() -> KudzuLibrary {
    let inputs: Value = serde_json::from_str(&read("search-inputs.json")).unwrap();
    serde_json::from_value(inputs["library"].clone()).unwrap()
}

/// Gives every distinct fixture id string a stable UUID, so non-UUID ids can be tested.
#[derive(Default)]
struct Ids {
    forward: HashMap<String, Uuid>,
    back: HashMap<Uuid, String>,
}

impl Ids {
    fn uuid(&mut self, raw: &str) -> Uuid {
        if let Some(id) = self.forward.get(raw) {
            return *id;
        }
        let id = Uuid::from_u128(self.forward.len() as u128 + 1);
        self.forward.insert(raw.to_string(), id);
        self.back.insert(id, raw.to_string());
        id
    }

    fn group(&mut self, raw: &Value) -> GroupRecord {
        let mut group = GroupRecord::new(raw["name"].as_str().unwrap());
        group.id = self.uuid(raw["id"].as_str().unwrap());
        group.parent_id = raw["parentID"].as_str().map(|p| self.uuid(p));
        group.sort_index = raw["sortIndex"].as_i64().unwrap();
        group.is_enabled = raw["isEnabled"].as_bool().unwrap();
        group
    }

    fn raw(&self, id: Uuid) -> &str {
        &self.back[&id]
    }
}

/// `{ id, depth, children: [...] }`, the shape the fixtures record.
fn tree_shape(ids: &Ids, nodes: &[GroupNode<'_>]) -> Value {
    Value::Array(
        nodes
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": ids.raw(n.group.id),
                    "depth": n.depth,
                    "children": tree_shape(ids, &n.children),
                })
            })
            .collect(),
    )
}

/// The same projection of a recorded tree.
fn expected_shape(nodes: &Value) -> Value {
    Value::Array(
        nodes
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                serde_json::json!({
                    "id": n["group"]["id"],
                    "depth": n["depth"],
                    "children": expected_shape(&n["children"]),
                })
            })
            .collect(),
    )
}

fn flat_ids(ids: &Ids, flat: &[&GroupNode<'_>]) -> Vec<String> {
    flat.iter()
        .map(|n| ids.raw(n.group.id).to_string())
        .collect()
}

fn expected_flat_ids(nodes: &Value) -> Vec<String> {
    nodes
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["group"]["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn sample_group_tree_flatten_paths_and_counts_match() {
    let fixture: Value = serde_json::from_str(&read("groups.json")).unwrap();
    let library = sample_library();
    let groups = &library.groups;
    let mut ids = Ids::default();
    for g in groups {
        ids.forward.insert(g.id.to_string(), g.id);
        ids.back.insert(g.id, g.id.to_string());
    }

    let tree = build_tree(groups);
    assert_eq!(tree_shape(&ids, &tree), expected_shape(&fixture["tree"]));
    assert_eq!(
        flat_ids(&ids, &flatten(&tree, &HashSet::new())),
        expected_flat_ids(&fixture["flattened"])
    );
    let collapsed = HashSet::from([groups[0].id]);
    assert_eq!(
        flat_ids(&ids, &flatten(&tree, &collapsed)),
        expected_flat_ids(&fixture["collapsed"])
    );

    for case in fixture["cases"].as_array().unwrap() {
        let id: Uuid = case["id"].as_str().unwrap().parse().unwrap();
        let want_path: Vec<&str> = case["path"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(group_path(groups, Some(id)), want_path, "path of {id}");
        let want_desc: BTreeSet<Uuid> = case["descendants"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().parse().unwrap())
            .collect();
        let got_desc: BTreeSet<Uuid> = descendant_ids(groups, id).into_iter().collect();
        assert_eq!(got_desc, want_desc, "descendants of {id}");
        assert_eq!(
            is_group_enabled(groups, Some(id)),
            case["enabled"].as_bool().unwrap(),
            "enabled {id}"
        );
    }

    let want_counts: HashMap<Uuid, usize> = fixture["counts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            (
                pair[0].as_str().unwrap().parse().unwrap(),
                pair[1].as_u64().unwrap() as usize,
            )
        })
        .collect();
    assert!(!want_counts.is_empty());
    assert_eq!(recursive_counts(groups, &library.snippets), want_counts);
}

#[test]
fn edge_case_groups_cycles_orphans_and_duplicate_ids_match() {
    let cases: Vec<Value> = serde_json::from_str(&read("group-edge-cases.json")).unwrap();
    assert_eq!(cases.len(), 3);
    let library = sample_library();
    for (n, case) in cases.iter().enumerate() {
        let mut ids = Ids::default();
        let groups: Vec<GroupRecord> = case["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| ids.group(g))
            .collect();
        let tree = build_tree(&groups);
        assert_eq!(
            tree_shape(&ids, &tree),
            expected_shape(&case["tree"]),
            "case {n}: tree"
        );
        for path in case["paths"].as_array().unwrap() {
            let id = ids.uuid(path["id"].as_str().unwrap());
            let want: Vec<&str> = path["path"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            assert_eq!(group_path(&groups, Some(id)), want, "case {n}: path");
            let want_desc: BTreeSet<Uuid> = path["descendants"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| ids.uuid(v.as_str().unwrap()))
                .collect();
            let got: BTreeSet<Uuid> = descendant_ids(&groups, id).into_iter().collect();
            assert_eq!(got, want_desc, "case {n}: descendants");
            assert_eq!(
                is_group_enabled(&groups, Some(id)),
                path["enabled"].as_bool().unwrap(),
                "case {n}: enabled"
            );
        }
        // The sample snippets belong to groups these fixtures do not define.
        assert_eq!(
            recursive_counts(&groups, &library.snippets).len(),
            case["counts"].as_array().unwrap().len(),
            "case {n}: counts"
        );
    }
}

#[derive(Deserialize)]
struct Stat {
    count: i64,
    chars: i64,
    last: i64,
}

#[test]
fn usage_aggregation_and_totals_match() {
    let fixture: Value = serde_json::from_str(&read("stats.json")).unwrap();
    let library = sample_library();
    let usage = usage_by_id(&library.usage);

    let want: HashMap<Uuid, Stat> = fixture["usage"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            (
                pair[0].as_str().unwrap().parse().unwrap(),
                serde_json::from_value(pair[1].clone()).unwrap(),
            )
        })
        .collect();
    assert_eq!(usage.len(), want.len());
    for (id, stat) in &want {
        let got = &usage[id];
        assert_eq!(
            (got.count, got.chars, got.last_ms),
            (stat.count, stat.chars, stat.last),
            "usage of {id}"
        );
    }

    let rows = fixture["totals"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    for row in rows {
        let wpm = row["wpm"].as_u64().unwrap() as u32;
        let result = &row["result"];
        let got = totals(&library.snippets, &usage, wpm);
        assert_eq!(got.expansions, result["expansions"].as_i64().unwrap());
        assert_eq!(got.chars, result["chars"].as_i64().unwrap());
        assert!(
            (got.minutes_saved - result["minutesSaved"].as_f64().unwrap()).abs() < 1e-9,
            "minutes saved at {wpm} wpm"
        );
        assert_eq!(
            got.active_snippets as u64,
            result["activeSnippets"].as_u64().unwrap()
        );
        assert_eq!(
            got.used_snippets as u64,
            result["usedSnippets"].as_u64().unwrap()
        );
    }
}
