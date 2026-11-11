//! iCloud sync model: how the library maps to CloudKit records and how changes merge.
//!
//! Pure logic with no CloudKit types, so every rule here is unit-tested. The desktop app's
//! CloudKit layer moves [`RemoteRecord`]s in and out and calls these functions.
//!
//! # Records
//!
//! | Record type     | Name                | `payload`                                   |
//! |-----------------|---------------------|---------------------------------------------|
//! | `KudzuSnippet`  | `snippet-<uuid>`    | the snippet's JSON (same shape as exports)  |
//! | `KudzuGroup`    | `group-<uuid>`      | the group's JSON                            |
//! | `KudzuUsage`    | `usage-<device id>` | that Mac's usage rows (only it writes them) |
//! | `KudzuMeta`     | `meta`              | `{ "syncVersion": n }`                      |
//!
//! Types are prefixed because the container also holds the Swift app's SwiftData (`CD_…`) types.
//!
//! Every record has a single string field, `payload`, holding JSON. The CloudKit schema never
//! changes when Kudzu adds a field: new fields are new JSON keys.
//!
//! # Rules that keep mixed versions safe
//!
//! * **Unknown keys survive.** Encoding merges known fields over the last payload received from
//!   the server, so an older Kudzu editing a snippet never erases fields a newer one added.
//! * **Sync version gate.** `meta.syncVersion` names the record format. A Mac that finds a
//!   version above [`SYNC_VERSION`] stops sending (it keeps receiving nothing it can't read) and
//!   asks the user to update.
//! * **Usage never conflicts.** Each Mac owns exactly one `Usage` record.
//! * **Conflicts:** last writer wins per record, by `updatedAt` for snippets; deletions win.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::types::{GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};

/// The record format this build writes and fully understands. Bump only for changes older
/// builds would misread (not for new JSON keys, which are always safe).
pub const SYNC_VERSION: i64 = 1;

/// The CloudKit zone holding the library.
pub const ZONE_NAME: &str = "Kudzu";
/// The single field every record carries.
pub const PAYLOAD_FIELD: &str = "payload";
/// Name of the format record.
pub const META_RECORD: &str = "meta";

/// CloudKit record types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum RecordKind {
    Snippet,
    Group,
    Usage,
    Meta,
}

impl RecordKind {
    /// The CloudKit record type name.
    pub fn record_type(self) -> &'static str {
        match self {
            Self::Snippet => "KudzuSnippet",
            Self::Group => "KudzuGroup",
            Self::Usage => "KudzuUsage",
            Self::Meta => "KudzuMeta",
        }
    }

    /// Parses a CloudKit record type name.
    pub fn from_record_type(s: &str) -> Option<Self> {
        [Self::Snippet, Self::Group, Self::Usage, Self::Meta]
            .into_iter()
            .find(|k| k.record_type() == s)
    }
}

/// Record name for a snippet.
pub fn snippet_name(id: Uuid) -> String {
    format!("snippet-{id}")
}
/// Record name for a group.
pub fn group_name(id: Uuid) -> String {
    format!("group-{id}")
}
/// Record name for one Mac's usage rows.
pub fn usage_name(device_id: &str) -> String {
    format!("usage-{device_id}")
}

/// A record as it travels to or from CloudKit, minus CloudKit's system fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteRecord {
    pub kind: RecordKind,
    pub name: String,
    pub payload: String,
}

/// What this Mac last knew the server held, per record. Persisted between launches.
///
/// It's how local edits are found (current state vs. what the server has) and how unknown
/// JSON keys are preserved (they live in these payloads).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncIndex {
    /// Record name → last server payload.
    pub records: BTreeMap<String, IndexedRecord>,
    /// Highest `syncVersion` seen on the server.
    #[serde(rename = "serverSyncVersion", default)]
    pub server_sync_version: i64,
    /// True once the first full fetch finished and duplicates were merged.
    #[serde(rename = "initialMergeDone", default)]
    pub initial_merge_done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedRecord {
    pub kind: RecordKind,
    pub payload: String,
}

impl SyncIndex {
    /// Whether the server holds a format this build can't safely write.
    pub fn blocked_by_newer_format(&self) -> bool {
        self.server_sync_version > SYNC_VERSION
    }
}

/// Local edits to send: records to save (built at send time by [`encode`]) and to delete.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalChanges {
    pub saves: Vec<String>,
    pub deletes: Vec<String>,
}

impl LocalChanges {
    pub fn is_empty(&self) -> bool {
        self.saves.is_empty() && self.deletes.is_empty()
    }
}

/// Serializes `value` over `base` (the last server payload), keeping keys `value` doesn't
/// know about. Known keys `value` omits (skipped `None`s) are removed, so clearing a field syncs.
fn merge_json<T: Serialize>(value: &T, base: Option<&str>, known: &[&str]) -> String {
    let mut out: Map<String, Value> = base
        .and_then(|b| serde_json::from_str::<Value>(b).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default();
    for k in known {
        out.remove(*k);
    }
    if let Value::Object(fields) = serde_json::to_value(value).expect("serializable") {
        out.extend(fields);
    }
    serde_json::to_string(&Value::Object(out)).expect("json")
}

const SNIPPET_KEYS: &[&str] = &[
    "id",
    "name",
    "triggers",
    "content",
    "formatting",
    "groupID",
    "fireMode",
    "caseMode",
    "insertion",
    "rule",
    "isFavorite",
    "isEnabled",
    "createdAt",
    "updatedAt",
];
const GROUP_KEYS: &[&str] = &[
    "id",
    "name",
    "parentID",
    "sortIndex",
    "isEnabled",
    "insertion",
    "rule",
];

#[derive(Serialize, Deserialize)]
struct UsagePayload {
    #[serde(rename = "deviceID")]
    device_id: String,
    rows: Vec<UsageRecord>,
}

#[derive(Serialize, Deserialize)]
struct MetaPayload {
    #[serde(rename = "syncVersion")]
    sync_version: i64,
}

/// The payload for record `name` from the current library, or `None` if it no longer exists
/// locally (so there's nothing to send).
pub fn encode(
    name: &str,
    library: &KudzuLibrary,
    index: &SyncIndex,
    device_id: &str,
) -> Option<RemoteRecord> {
    let base = index.records.get(name).map(|r| r.payload.as_str());
    if name == META_RECORD {
        let version = index.server_sync_version.max(SYNC_VERSION);
        let payload = merge_json(
            &MetaPayload {
                sync_version: version,
            },
            base,
            &["syncVersion"],
        );
        return Some(RemoteRecord {
            kind: RecordKind::Meta,
            name: name.into(),
            payload,
        });
    }
    if name == usage_name(device_id) {
        let mut rows: Vec<UsageRecord> = library
            .usage
            .iter()
            .filter(|u| u.device_id == device_id)
            .cloned()
            .collect();
        rows.sort_by_key(|u| u.snippet_id);
        let payload = merge_json(
            &UsagePayload {
                device_id: device_id.into(),
                rows,
            },
            base,
            &["deviceID", "rows"],
        );
        return Some(RemoteRecord {
            kind: RecordKind::Usage,
            name: name.into(),
            payload,
        });
    }
    if let Some(id) = name
        .strip_prefix("snippet-")
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        let s = library.snippets.iter().find(|s| s.id == id)?;
        return Some(RemoteRecord {
            kind: RecordKind::Snippet,
            name: name.into(),
            payload: merge_json(s, base, SNIPPET_KEYS),
        });
    }
    if let Some(id) = name
        .strip_prefix("group-")
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        let g = library.groups.iter().find(|g| g.id == id)?;
        return Some(RemoteRecord {
            kind: RecordKind::Group,
            name: name.into(),
            payload: merge_json(g, base, GROUP_KEYS),
        });
    }
    None
}

fn same_json(a: &str, b: &str) -> bool {
    match (
        serde_json::from_str::<Value>(a),
        serde_json::from_str::<Value>(b),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// Finds local edits: everything whose current payload differs from what the server has,
/// plus records the server has that no longer exist here. Never deletes other Macs' usage or
/// the meta record. Empty while the server's format is newer than this build.
pub fn local_changes(library: &KudzuLibrary, index: &SyncIndex, device_id: &str) -> LocalChanges {
    let mut out = LocalChanges::default();
    if index.blocked_by_newer_format() {
        return out;
    }
    let mut names: Vec<String> =
        Vec::with_capacity(library.snippets.len() + library.groups.len() + 2);
    names.extend(library.groups.iter().map(|g| group_name(g.id)));
    names.extend(library.snippets.iter().map(|s| snippet_name(s.id)));
    if library.usage.iter().any(|u| u.device_id == device_id)
        || index.records.contains_key(&usage_name(device_id))
    {
        names.push(usage_name(device_id));
    }
    if index.server_sync_version < SYNC_VERSION {
        names.push(META_RECORD.into());
    }
    for name in names {
        let Some(rec) = encode(&name, library, index, device_id) else {
            continue;
        };
        match index.records.get(&name) {
            Some(known) if same_json(&known.payload, &rec.payload) => {}
            _ => out.saves.push(name),
        }
    }
    let present: std::collections::HashSet<String> = library
        .snippets
        .iter()
        .map(|s| snippet_name(s.id))
        .chain(library.groups.iter().map(|g| group_name(g.id)))
        .collect();
    for (name, rec) in &index.records {
        if matches!(rec.kind, RecordKind::Snippet | RecordKind::Group) && !present.contains(name) {
            out.deletes.push(name.clone());
        }
    }
    out
}

/// Records that the server accepted `record` (a save finished).
pub fn record_saved(index: &mut SyncIndex, record: &RemoteRecord) {
    index.records.insert(
        record.name.clone(),
        IndexedRecord {
            kind: record.kind,
            payload: record.payload.clone(),
        },
    );
    if record.kind == RecordKind::Meta {
        if let Ok(m) = serde_json::from_str::<MetaPayload>(&record.payload) {
            index.server_sync_version = index.server_sync_version.max(m.sync_version);
        }
    }
}

/// Records that the server confirmed a deletion.
pub fn record_deleted(index: &mut SyncIndex, name: &str) {
    index.records.remove(name);
}

/// The result of applying server changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// The library changed and must be saved / reloaded.
    pub library_changed: bool,
    /// The server uses a newer record format than this build understands.
    pub needs_update: bool,
}

/// Whether the local copy of `name` has edits the server hasn't seen yet.
fn has_unsent_edit(name: &str, library: &KudzuLibrary, index: &SyncIndex, device_id: &str) -> bool {
    match (
        encode(name, library, index, device_id),
        index.records.get(name),
    ) {
        (Some(local), Some(known)) => !same_json(&local.payload, &known.payload),
        (Some(_), None) => true,
        _ => false,
    }
}

/// Applies records fetched from (or returned by) the server.
///
/// A snippet that was edited here more recently than the server's copy keeps the local
/// version (it will be sent and win). Otherwise the server's copy replaces the local one.
pub fn apply_remote(
    library: &mut KudzuLibrary,
    index: &mut SyncIndex,
    records: &[RemoteRecord],
    deleted: &[String],
    device_id: &str,
) -> Applied {
    let mut out = Applied::default();
    for rec in records {
        match rec.kind {
            RecordKind::Meta => {
                if let Ok(m) = serde_json::from_str::<MetaPayload>(&rec.payload) {
                    index.server_sync_version = index.server_sync_version.max(m.sync_version);
                }
                index.records.insert(
                    rec.name.clone(),
                    IndexedRecord {
                        kind: rec.kind,
                        payload: rec.payload.clone(),
                    },
                );
            }
            RecordKind::Snippet => {
                let Ok(remote) = serde_json::from_str::<SnippetRecord>(&rec.payload) else {
                    continue;
                };
                let unsent = has_unsent_edit(&rec.name, library, index, device_id);
                let pos = library.snippets.iter().position(|s| s.id == remote.id);
                let keep_local = unsent
                    && pos.is_some_and(|i| library.snippets[i].updated_at > remote.updated_at);
                index.records.insert(
                    rec.name.clone(),
                    IndexedRecord {
                        kind: rec.kind,
                        payload: rec.payload.clone(),
                    },
                );
                if keep_local {
                    continue;
                }
                match pos {
                    Some(i) if library.snippets[i] == remote => {}
                    Some(i) => {
                        library.snippets[i] = remote;
                        out.library_changed = true;
                    }
                    None => {
                        library.snippets.push(remote);
                        out.library_changed = true;
                    }
                }
            }
            RecordKind::Group => {
                let Ok(remote) = serde_json::from_str::<GroupRecord>(&rec.payload) else {
                    continue;
                };
                // Groups carry no timestamp: an unsent local edit wins, otherwise the server does.
                let unsent = has_unsent_edit(&rec.name, library, index, device_id);
                let pos = library.groups.iter().position(|g| g.id == remote.id);
                index.records.insert(
                    rec.name.clone(),
                    IndexedRecord {
                        kind: rec.kind,
                        payload: rec.payload.clone(),
                    },
                );
                if unsent && pos.is_some() {
                    continue;
                }
                match pos {
                    Some(i) if library.groups[i] == remote => {}
                    Some(i) => {
                        library.groups[i] = remote;
                        out.library_changed = true;
                    }
                    None => {
                        library.groups.push(remote);
                        out.library_changed = true;
                    }
                }
            }
            RecordKind::Usage => {
                let Ok(remote) = serde_json::from_str::<UsagePayload>(&rec.payload) else {
                    continue;
                };
                index.records.insert(
                    rec.name.clone(),
                    IndexedRecord {
                        kind: rec.kind,
                        payload: rec.payload.clone(),
                    },
                );
                if remote.device_id == device_id {
                    // Our own record coming back (e.g. after a reinstall kept the device id):
                    // never lose counts, take the larger of each.
                    let mut changed = false;
                    for row in remote.rows {
                        match library
                            .usage
                            .iter_mut()
                            .find(|u| u.device_id == device_id && u.snippet_id == row.snippet_id)
                        {
                            Some(u) if u.count >= row.count => {}
                            Some(u) => {
                                *u = row;
                                changed = true;
                            }
                            None => {
                                library.usage.push(row);
                                changed = true;
                            }
                        }
                    }
                    out.library_changed |= changed;
                } else {
                    let before: Vec<&UsageRecord> = library
                        .usage
                        .iter()
                        .filter(|u| u.device_id == remote.device_id)
                        .collect();
                    let mut rows = remote.rows;
                    rows.iter_mut()
                        .for_each(|r| r.device_id = remote.device_id.clone());
                    if before.len() != rows.len() || before.iter().zip(&rows).any(|(a, b)| *a != b)
                    {
                        library.usage.retain(|u| u.device_id != remote.device_id);
                        library.usage.extend(rows);
                        out.library_changed = true;
                    }
                }
            }
        }
    }
    for name in deleted {
        index.records.remove(name);
        if let Some(id) = name
            .strip_prefix("snippet-")
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            let before = library.snippets.len();
            library.snippets.retain(|s| s.id != id);
            out.library_changed |= library.snippets.len() != before;
        } else if let Some(id) = name
            .strip_prefix("group-")
            .and_then(|s| Uuid::parse_str(s).ok())
        {
            let before = library.groups.len();
            library.groups.retain(|g| g.id != id);
            if library.groups.len() != before {
                out.library_changed = true;
                // Children and snippets of a group deleted elsewhere move up rather than vanish.
                for g in library
                    .groups
                    .iter_mut()
                    .filter(|g| g.parent_id == Some(id))
                {
                    g.parent_id = None;
                }
                for s in library
                    .snippets
                    .iter_mut()
                    .filter(|s| s.group_id == Some(id))
                {
                    s.group_id = None;
                }
            }
        }
    }
    out.needs_update = index.blocked_by_newer_format();
    out
}

/// After the first full fetch on this Mac, merges duplicates (the same starter snippets
/// created on two Macs, or one library imported twice). Returns true if the library changed;
/// the deletions this causes reach the server through [`local_changes`].
pub fn initial_merge(library: &mut KudzuLibrary, index: &mut SyncIndex) -> bool {
    if index.initial_merge_done {
        return false;
    }
    index.initial_merge_done = true;
    let (merged, _report) = crate::interchange::dedupe::dedupe(library, &HashMap::new());
    if merged == *library {
        return false;
    }
    *library = merged;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    const ME: &str = "mac-a";
    const OTHER: &str = "mac-b";

    fn uuid(n: u64) -> Uuid {
        Uuid::parse_str(&format!("00000000-0000-0000-0000-{n:012}")).unwrap()
    }

    fn snippet(n: u64, trigger: &str, minutes: i64) -> SnippetRecord {
        let mut s = SnippetRecord::new(
            format!("S{n}"),
            vec![trigger.into()],
            format!("content {n}"),
        );
        s.id = uuid(n);
        let t = Utc.timestamp_opt(1_780_000_000, 0).unwrap() + Duration::minutes(minutes);
        s.created_at = t;
        s.updated_at = t;
        s
    }

    fn lib(snippets: Vec<SnippetRecord>) -> KudzuLibrary {
        KudzuLibrary {
            snippets,
            ..Default::default()
        }
    }

    /// Simulates a full successful send of `changes`.
    fn send(
        library: &KudzuLibrary,
        index: &mut SyncIndex,
        changes: &LocalChanges,
    ) -> Vec<RemoteRecord> {
        let mut sent = vec![];
        for name in &changes.saves {
            let rec = encode(name, library, index, ME).unwrap();
            record_saved(index, &rec);
            sent.push(rec);
        }
        for name in &changes.deletes {
            record_deleted(index, name);
        }
        sent
    }

    #[test]
    fn first_sync_uploads_everything_then_nothing() {
        let library = lib(vec![snippet(1, ";a", 0), snippet(2, ";b", 0)]);
        let mut index = SyncIndex::default();
        let changes = local_changes(&library, &index, ME);
        assert_eq!(
            changes.saves,
            vec![
                snippet_name(uuid(1)),
                snippet_name(uuid(2)),
                META_RECORD.to_string()
            ]
        );
        send(&library, &mut index, &changes);
        assert!(local_changes(&library, &index, ME).is_empty());
    }

    #[test]
    fn edits_and_deletions_are_detected() {
        let mut library = lib(vec![snippet(1, ";a", 0), snippet(2, ";b", 0)]);
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        send(&library, &mut index, &c);

        library.snippets[0].content = "edited".into();
        library.snippets.remove(1);
        let changes = local_changes(&library, &index, ME);
        assert_eq!(changes.saves, vec![snippet_name(uuid(1))]);
        assert_eq!(changes.deletes, vec![snippet_name(uuid(2))]);
    }

    #[test]
    fn unknown_fields_from_newer_versions_survive_an_edit() {
        let mut library = lib(vec![snippet(1, ";a", 0)]);
        let mut index = SyncIndex::default();
        // A newer Kudzu wrote the snippet with a field this build doesn't know.
        let mut json: Value = serde_json::to_value(&library.snippets[0]).unwrap();
        json["color"] = Value::String("green".into());
        let remote = RemoteRecord {
            kind: RecordKind::Snippet,
            name: snippet_name(uuid(1)),
            payload: json.to_string(),
        };
        apply_remote(&mut library, &mut index, &[remote], &[], ME);

        library.snippets[0].content = "edited here".into();
        let sent = encode(&snippet_name(uuid(1)), &library, &index, ME).unwrap();
        let v: Value = serde_json::from_str(&sent.payload).unwrap();
        assert_eq!(v["color"], "green");
        assert_eq!(v["content"], "edited here");
    }

    #[test]
    fn clearing_an_optional_field_syncs() {
        let mut s = snippet(1, ";a", 0);
        s.group_id = Some(uuid(9));
        let mut library = lib(vec![s]);
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        send(&library, &mut index, &c);
        library.snippets[0].group_id = None;
        let rec = encode(&snippet_name(uuid(1)), &library, &index, ME).unwrap();
        let v: Value = serde_json::from_str(&rec.payload).unwrap();
        assert!(v.get("groupID").is_none());
    }

    #[test]
    fn remote_changes_apply_without_echoing_back() {
        let mut library = lib(vec![snippet(1, ";a", 0)]);
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        send(&library, &mut index, &c);

        let mut theirs = snippet(1, ";a", 5);
        theirs.content = "from the other Mac".into();
        let rec = RemoteRecord {
            kind: RecordKind::Snippet,
            name: snippet_name(uuid(1)),
            payload: serde_json::to_string(&theirs).unwrap(),
        };
        let applied = apply_remote(&mut library, &mut index, &[rec], &[], ME);
        assert!(applied.library_changed);
        assert_eq!(library.snippets[0].content, "from the other Mac");
        assert!(
            local_changes(&library, &index, ME).is_empty(),
            "applying remote data must not trigger a re-send"
        );
    }

    #[test]
    fn newer_local_edit_wins_a_conflict_older_one_loses() {
        let mut library = lib(vec![snippet(1, ";a", 0)]);
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        send(&library, &mut index, &c);

        // Edited here at minute 10, unsent; the server has an edit from minute 5.
        library.snippets[0].content = "mine".into();
        library.snippets[0].updated_at += Duration::minutes(10);
        let mut theirs = snippet(1, ";a", 5);
        theirs.content = "theirs".into();
        let rec = RemoteRecord {
            kind: RecordKind::Snippet,
            name: snippet_name(uuid(1)),
            payload: serde_json::to_string(&theirs).unwrap(),
        };
        apply_remote(
            &mut library,
            &mut index,
            std::slice::from_ref(&rec),
            &[],
            ME,
        );
        assert_eq!(library.snippets[0].content, "mine");
        assert_eq!(
            local_changes(&library, &index, ME).saves,
            vec![snippet_name(uuid(1))],
            "still needs sending"
        );

        // Now an older local edit (minute 2) loses to the server's minute-5 version.
        let mut library = lib(vec![snippet(1, ";a", 0)]);
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        send(&library, &mut index, &c);
        library.snippets[0].content = "mine, older".into();
        library.snippets[0].updated_at += Duration::minutes(2);
        apply_remote(&mut library, &mut index, &[rec], &[], ME);
        assert_eq!(library.snippets[0].content, "theirs");
    }

    #[test]
    fn remote_group_deletion_keeps_its_snippets() {
        let mut g = GroupRecord::new("Work");
        g.id = uuid(50);
        let mut s = snippet(1, ";a", 0);
        s.group_id = Some(g.id);
        let mut library = KudzuLibrary {
            groups: vec![g],
            snippets: vec![s],
            ..Default::default()
        };
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        send(&library, &mut index, &c);

        let applied = apply_remote(&mut library, &mut index, &[], &[group_name(uuid(50))], ME);
        assert!(applied.library_changed);
        assert!(library.groups.is_empty());
        assert_eq!(library.snippets.len(), 1);
        assert_eq!(library.snippets[0].group_id, None);
    }

    #[test]
    fn usage_is_per_mac_and_never_overwritten() {
        let mut library = lib(vec![snippet(1, ";a", 0)]);
        library.usage.push(UsageRecord {
            snippet_id: uuid(1),
            device_id: ME.into(),
            count: 3,
            chars_saved: 30,
            last_used_at: None,
        });
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        assert!(c.saves.contains(&usage_name(ME)));
        send(&library, &mut index, &c);

        // The other Mac's counts arrive; ours are untouched and nothing is re-sent.
        let theirs = UsagePayload {
            device_id: OTHER.into(),
            rows: vec![UsageRecord {
                snippet_id: uuid(1),
                device_id: OTHER.into(),
                count: 7,
                chars_saved: 70,
                last_used_at: None,
            }],
        };
        let rec = RemoteRecord {
            kind: RecordKind::Usage,
            name: usage_name(OTHER),
            payload: serde_json::to_string(&theirs).unwrap(),
        };
        apply_remote(&mut library, &mut index, &[rec], &[], ME);
        let total: i64 = library.usage.iter().map(|u| u.count).sum();
        assert_eq!(total, 10);
        assert!(local_changes(&library, &index, ME).is_empty());

        // Their record never appears in our deletes even though we don't "own" it.
        library.usage.retain(|u| u.device_id == ME);
        assert!(local_changes(&library, &index, ME).deletes.is_empty());
    }

    #[test]
    fn a_newer_sync_format_stops_sending() {
        let mut library = lib(vec![snippet(1, ";a", 0)]);
        let mut index = SyncIndex::default();
        let meta = RemoteRecord {
            kind: RecordKind::Meta,
            name: META_RECORD.into(),
            payload: r#"{"syncVersion":2}"#.into(),
        };
        let applied = apply_remote(&mut library, &mut index, &[meta], &[], ME);
        assert!(applied.needs_update);
        library.snippets[0].content = "edited".into();
        assert!(local_changes(&library, &index, ME).is_empty());
    }

    #[test]
    fn meta_is_written_once_and_never_downgraded() {
        let library = lib(vec![]);
        let mut index = SyncIndex::default();
        let c = local_changes(&library, &index, ME);
        assert_eq!(c.saves, vec![META_RECORD.to_string()]);
        send(&library, &mut index, &c);
        assert_eq!(index.server_sync_version, SYNC_VERSION);
        assert!(local_changes(&library, &index, ME).is_empty());
    }

    #[test]
    fn initial_merge_folds_duplicate_starter_snippets() {
        // Both Macs created the same starter snippet with different ids.
        let mut library = lib(vec![snippet(1, ";kudzu", 0), snippet(2, ";kudzu", 1)]);
        library.snippets[1].content = library.snippets[0].content.clone();
        let mut index = SyncIndex::default();
        assert!(initial_merge(&mut library, &mut index));
        assert_eq!(library.snippets.len(), 1);
        assert!(!initial_merge(&mut library, &mut index), "runs once");
    }

    #[test]
    fn records_for_missing_items_are_skipped() {
        let library = lib(vec![]);
        assert!(encode(&snippet_name(uuid(1)), &library, &SyncIndex::default(), ME).is_none());
        assert_eq!(
            RecordKind::from_record_type("KudzuSnippet"),
            Some(RecordKind::Snippet)
        );
        assert_eq!(RecordKind::from_record_type("Nope"), None);
    }
}
