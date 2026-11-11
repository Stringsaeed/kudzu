//! Byte-exact golden tests for every format Kudzu persists: the library as Kudzu JSON, the
//! clipboard index, the sync index and record payloads, form memory, and backup file names.
//!
//! Each fixture under `tests/fixtures/golden/` is what the current build writes for a fixed
//! value. The tests compare bytes, so a serde attribute, a field order or a date format can't
//! drift unnoticed while the crate is restructured. Run with `KUDZU_GOLDEN_WRITE=1` to rewrite
//! the fixtures on purpose (then review the diff).

mod interchange_support;

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use kudzu_core::clipboard::{ClipEntry, ClipKind, ClipSource, History, ImageSize, SECRET_RULES};
use kudzu_core::formatting::{FormatSpan, FormatStyle, Formatting};
use kudzu_core::interchange::backup::{BackupReason, BackupStore, BackupTimeZone};
use kudzu_core::interchange::kudzu_json;
use kudzu_core::secrets::SecretKind;
use kudzu_core::sync::{self, IndexedRecord, RecordKind, SyncIndex};
use kudzu_core::template::FormMemory;
use kudzu_core::{
    AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary,
    SnippetRecord, UsageRecord,
};
use uuid::Uuid;

const DEVICE: &str = "device-A";

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/golden/{name}"))
}

/// Compares `actual` with the fixture, or rewrites the fixture when `KUDZU_GOLDEN_WRITE` is set.
fn assert_golden(name: &str, actual: &[u8]) {
    let path = fixture_path(name);
    if std::env::var_os("KUDZU_GOLDEN_WRITE").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing golden fixture {}: {e}", path.display()));
    assert_eq!(
        String::from_utf8_lossy(actual),
        String::from_utf8_lossy(&expected),
        "{name} changed; rerun with KUDZU_GOLDEN_WRITE=1 only if the format is meant to change"
    );
}

fn library() -> KudzuLibrary {
    let work = GroupRecord {
        id: id(0x10),
        name: "Work".into(),
        parent_id: None,
        sort_index: 0,
        is_enabled: true,
        insertion: Some(InsertionMethod::Paste),
        rule: Some(AppRule {
            mode: AppRuleMode::Deny,
            bundle_ids: vec!["com.apple.Terminal".into()],
            insertion: None,
        }),
    };
    let mail = GroupRecord {
        id: id(0x11),
        name: "Mail".into(),
        parent_id: Some(id(0x10)),
        sort_index: 1,
        is_enabled: false,
        insertion: None,
        rule: None,
    };
    let signature = SnippetRecord {
        id: id(0x20),
        name: "Signature".into(),
        triggers: vec![";sig".into(), ";signature".into()],
        content: "Best regards,\n{{field name=\"Name\" default=\"Ada\"}}".into(),
        formatting: Some(Formatting {
            source: "Best regards,\n{{field name=\"Name\" default=\"Ada\"}}".into(),
            spans: vec![
                FormatSpan {
                    start: 0,
                    end: 4,
                    style: FormatStyle::Bold,
                },
                FormatSpan {
                    start: 5,
                    end: 13,
                    style: FormatStyle::Link {
                        url: "https://kudzu.site".into(),
                    },
                },
            ],
        }),
        group_id: Some(id(0x11)),
        fire_mode: FireMode::WholeWord,
        case_mode: CaseMode::Exact,
        insertion: Some(InsertionMethod::Type),
        rule: Some(AppRule {
            mode: AppRuleMode::Allow,
            bundle_ids: vec!["com.apple.mail".into(), "com.tinyspeck.slackmacgap".into()],
            insertion: Some(InsertionMethod::Paste),
        }),
        is_favorite: true,
        is_enabled: false,
        created_at: at(2026, 1, 2, 3, 4, 5),
        updated_at: at(2026, 6, 7, 8, 9, 10),
    };
    let date = SnippetRecord {
        id: id(0x21),
        name: "Today".into(),
        triggers: vec![";date".into()],
        content: "{{date format=\"EEEE, d MMMM yyyy\"}}".into(),
        formatting: None,
        group_id: None,
        fire_mode: FireMode::Immediate,
        case_mode: CaseMode::Propagate,
        insertion: None,
        rule: None,
        is_favorite: false,
        is_enabled: true,
        created_at: at(2025, 12, 31, 23, 59, 59),
        updated_at: at(2025, 12, 31, 23, 59, 59),
    };
    KudzuLibrary {
        format_version: KudzuLibrary::CURRENT_FORMAT_VERSION,
        groups: vec![work, mail],
        snippets: vec![signature, date],
        usage: vec![
            UsageRecord {
                snippet_id: id(0x20),
                device_id: DEVICE.into(),
                count: 12,
                chars_saved: 340,
                last_used_at: Some(at(2026, 6, 8, 9, 0, 0)),
            },
            UsageRecord {
                snippet_id: id(0x21),
                device_id: "device-B".into(),
                count: 1,
                chars_saved: 20,
                last_used_at: None,
            },
        ],
    }
}

#[test]
fn library_json_is_byte_exact() {
    let library = library();
    let json = kudzu_json::encode(&library).unwrap();
    assert_golden("library.json", json.as_bytes());

    let decoded = kudzu_json::decode(&json).unwrap();
    assert_eq!(decoded, library);
    assert_eq!(kudzu_json::encode(&decoded).unwrap(), json);
}

fn history() -> History {
    let text = ClipEntry {
        id: id(0x30),
        kind: ClipKind::Code,
        text: Some("git status".into()),
        text_len: 10,
        line_count: 1,
        files: vec![],
        image: None,
        color: None,
        rich: false,
        types: vec!["public.utf8-plain-text".into()],
        size: 10,
        source: Some(ClipSource {
            bundle_id: "com.apple.Terminal".into(),
            name: "Terminal".into(),
            path: Some("/System/Applications/Utilities/Terminal.app".into()),
        }),
        hash: "00000000deadbeef".into(),
        first_copied_at: at(2026, 11, 1, 10, 0, 0),
        copied_at: at(2026, 11, 1, 12, 30, 0),
        copy_count: 2,
        timings: vec![at(2026, 11, 1, 10, 0, 0), at(2026, 11, 1, 12, 30, 0)],
        last_used_at: Some(at(2026, 11, 1, 13, 0, 0)),
        use_count: 1,
        pinned: true,
        secret: None,
        secret_override: Some(false),
    };
    let image = ClipEntry {
        id: id(0x31),
        kind: ClipKind::Image,
        text: None,
        text_len: 0,
        line_count: 0,
        files: vec![],
        image: Some(ImageSize {
            width: 640,
            height: 480,
        }),
        color: None,
        rich: false,
        types: vec!["public.png".into(), "public.tiff".into()],
        size: 2048,
        source: None,
        hash: "0123456789abcdef".into(),
        first_copied_at: at(2026, 11, 1, 9, 0, 0),
        copied_at: at(2026, 11, 1, 9, 0, 0),
        copy_count: 1,
        timings: vec![at(2026, 11, 1, 9, 0, 0)],
        last_used_at: None,
        use_count: 0,
        pinned: false,
        secret: None,
        secret_override: None,
    };
    let password = ClipEntry {
        id: id(0x32),
        kind: ClipKind::Text,
        text: Some("hunter2!X".into()),
        text_len: 9,
        line_count: 1,
        files: vec!["/Users/me/notes.txt".into()],
        image: None,
        color: Some("#1e90ff".into()),
        rich: true,
        types: vec![],
        size: 9,
        source: None,
        hash: "ffffffffffffffff".into(),
        first_copied_at: at(2026, 10, 31, 8, 0, 0),
        copied_at: at(2026, 10, 31, 8, 0, 0),
        copy_count: 1,
        timings: vec![],
        last_used_at: None,
        use_count: 0,
        pinned: false,
        secret: Some(SecretKind::Password),
        secret_override: Some(true),
    };
    History {
        version: 1,
        secret_rules: SECRET_RULES,
        entries: vec![text, image, password],
    }
}

#[test]
fn clipboard_index_is_byte_exact() {
    let history = history();
    // The app writes the index compactly (`serde_json::to_vec`).
    let bytes = serde_json::to_vec(&history).unwrap();
    assert_golden("clipboard-index.json", &bytes);

    let decoded: History = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, history);
}

#[test]
fn sync_payloads_are_byte_exact() {
    let library = library();
    let mut index = SyncIndex::default();
    index.server_sync_version = sync::SYNC_VERSION;
    index.initial_merge_done = true;
    // A base payload with a key this build doesn't know: it must survive re-encoding.
    index.records.insert(
        sync::snippet_name(id(0x20)),
        IndexedRecord {
            kind: RecordKind::Snippet,
            payload: r#"{"id":"00000000-0000-0000-0000-000000000020","futureField":{"keep":true},"name":"old"}"#.into(),
        },
    );
    index.records.insert(
        sync::usage_name(DEVICE),
        IndexedRecord {
            kind: RecordKind::Usage,
            payload: r#"{"deviceID":"device-A","rows":[]}"#.into(),
        },
    );

    let mut out = String::new();
    for name in [
        sync::snippet_name(id(0x20)),
        sync::snippet_name(id(0x21)),
        sync::group_name(id(0x10)),
        sync::group_name(id(0x11)),
        sync::usage_name(DEVICE),
        sync::META_RECORD.to_string(),
    ] {
        let record = sync::encode(&name, &library, &index, DEVICE).unwrap();
        out.push_str(&format!(
            "{} {} {}\n",
            record.kind.record_type(),
            record.name,
            record.payload
        ));
    }
    assert_golden("sync-payloads.txt", out.as_bytes());

    // The persisted index (`serde_json::to_vec`), with a BTreeMap so its order is stable.
    let bytes = serde_json::to_vec(&index).unwrap();
    assert_golden("sync-index.json", &bytes);
    let decoded: SyncIndex = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, index);
}

#[test]
fn form_memory_is_byte_exact() {
    // One snippet with one field: the maps have a single entry each, so the order is fixed.
    let mut memory = FormMemory::default();
    memory.record(
        HashMap::from([("Name".to_string(), "Ada".to_string())]),
        id(0x20),
    );
    let bytes = serde_json::to_vec(&memory).unwrap();
    assert_golden("form-memory.json", &bytes);

    let decoded: FormMemory = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, memory);
}

#[test]
fn backup_names_are_byte_exact() {
    let dir = interchange_support::TempDir::new("golden-backups");
    let zone = BackupTimeZone::Fixed(FixedOffset::east_opt(2 * 3600).unwrap());
    let store = BackupStore::with_time_zone(dir.path(), zone);
    let now = at(2026, 3, 4, 23, 5, 6);
    let mut names = Vec::new();
    for reason in [
        BackupReason::Daily,
        BackupReason::PreImport,
        BackupReason::PreImport,
        BackupReason::PreRestore,
        BackupReason::Manual,
        BackupReason::PreUpdate,
    ] {
        let info = store.write(&library(), reason, now).unwrap();
        assert_eq!(info.created_at, now);
        assert_eq!(info.reason, reason);
        names.push(info.filename);
    }
    let listed: Vec<String> = store
        .list()
        .unwrap()
        .into_iter()
        .map(|i| i.filename)
        .collect();
    let mut out = names.join("\n");
    out.push_str("\n--\n");
    out.push_str(&listed.join("\n"));
    out.push('\n');
    assert_golden("backup-names.txt", out.as_bytes());

    let json = std::fs::read_to_string(dir.path().join(&names[0])).unwrap();
    assert_eq!(json, kudzu_json::encode(&library()).unwrap());
}
