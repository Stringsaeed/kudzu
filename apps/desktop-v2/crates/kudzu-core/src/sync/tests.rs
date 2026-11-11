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
