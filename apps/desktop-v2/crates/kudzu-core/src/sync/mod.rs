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
//!
//! [`SyncIndex`] is persisted between launches and every payload is what CloudKit stores, so
//! both shapes are pinned by the golden tests.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::library::model::{GroupRecord, KudzuLibrary, SnippetRecord, UsageRecord};

/// The record format this build writes and fully understands. Bump only for changes older
/// builds would misread (not for new JSON keys, which are always safe).
pub const SYNC_VERSION: i64 = 1;

/// CloudKit container. Production builds sync this container; Kudzu Next never opens it.
pub const CONTAINER_ID: &str = "iCloud.sh.saeed.kudzu";

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
    let (merged, _report) = crate::library::dedupe::dedupe(library, &HashMap::new());
    if merged == *library {
        return false;
    }
    *library = merged;
    true
}

#[cfg(test)]
mod tests;
