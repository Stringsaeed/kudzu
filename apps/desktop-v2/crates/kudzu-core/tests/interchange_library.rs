//! Kudzu JSON, backups and the import coordinator, ported from the Swift `KudzuJSONTests`,
//! `BackupStoreTests` and `ImportCoordinatorTests`.

mod interchange_support;

/// Named so `cargo test interchange` selects these.
mod interchange {
    use crate::interchange_support::TempDir;
    use chrono::{DateTime, FixedOffset, TimeZone, Utc};
    use kudzu_core::interchange::backup::{BackupReason, BackupStore, BackupTimeZone};
    use kudzu_core::interchange::espanso::{self, ImportWarning};
    use kudzu_core::interchange::{coordinator, merge, InterchangeError};
    use kudzu_core::library::json as kudzu_json;
    use kudzu_core::library::starter::{self, SAMPLE_TRIGGER};
    use kudzu_core::{
        AppRule, AppRuleMode, CaseMode, FireMode, GroupRecord, InsertionMethod, KudzuLibrary,
        SnippetRecord, UsageRecord,
    };
    use uuid::Uuid;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    fn library_with(snippets: Vec<SnippetRecord>) -> KudzuLibrary {
        KudzuLibrary {
            snippets,
            ..KudzuLibrary::default()
        }
    }

    // MARK: - Kudzu JSON

    #[test]
    fn json_encodes_sorted_pretty_iso_dates_and_round_trips() {
        let date = Utc.timestamp_opt(1_762_000_000, 0).unwrap();
        let group_id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let snippet_id = Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let group = GroupRecord {
            id: group_id,
            insertion: Some(InsertionMethod::Paste),
            rule: Some(AppRule {
                mode: AppRuleMode::Allow,
                bundle_ids: vec!["com.apple.mail".into()],
                insertion: Some(InsertionMethod::Paste),
            }),
            ..GroupRecord::new("Work")
        };
        let snippet = SnippetRecord {
            id: snippet_id,
            group_id: Some(group_id),
            fire_mode: FireMode::WholeWord,
            case_mode: CaseMode::Propagate,
            insertion: Some(InsertionMethod::Automatic),
            is_favorite: true,
            created_at: date,
            updated_at: date,
            ..SnippetRecord::new(
                "Signature",
                vec![";sig".into(), ":sig".into()],
                "Best regards,\n{{cursor}}",
            )
        };
        let usage = UsageRecord {
            snippet_id,
            device_id: "mac-1".into(),
            count: 42,
            chars_saved: 500,
            last_used_at: Some(date),
        };
        let library = KudzuLibrary {
            groups: vec![group],
            snippets: vec![snippet],
            usage: vec![usage],
            ..KudzuLibrary::default()
        };

        let json = kudzu_json::encode(&library).unwrap();
        assert!(json.contains('\n'));
        let pos = |k: &str| json.find(&format!("\"{k}\"")).unwrap();
        assert!(pos("formatVersion") < pos("groups"));
        assert!(pos("groups") < pos("snippets"));
        assert!(pos("snippets") < pos("usage"));
        // Keys inside records are sorted too.
        assert!(pos("caseMode") < pos("content"));
        assert!(json.contains("2025-11-01T12:26:40Z"));
        assert_eq!(kudzu_json::decode(&json).unwrap(), library);
    }

    #[test]
    fn json_rejects_newer_format_versions() {
        let future = KudzuLibrary {
            format_version: KudzuLibrary::CURRENT_FORMAT_VERSION + 1,
            ..KudzuLibrary::default()
        };
        let json = kudzu_json::encode(&future).unwrap();
        match kudzu_json::decode(&json) {
            Err(InterchangeError::UnsupportedFormatVersion {
                found,
                max_supported,
            }) => {
                assert_eq!(found, KudzuLibrary::CURRENT_FORMAT_VERSION + 1);
                assert_eq!(max_supported, KudzuLibrary::CURRENT_FORMAT_VERSION);
            }
            other => panic!("expected UnsupportedFormatVersion, got {other:?}"),
        }
        assert!(matches!(
            kudzu_json::decode("{nope"),
            Err(InterchangeError::Decoding(_))
        ));
    }

    // MARK: - Backups

    fn utc_store(dir: &std::path::Path) -> BackupStore {
        BackupStore::with_time_zone(
            dir,
            BackupTimeZone::Fixed(FixedOffset::east_opt(0).unwrap()),
        )
    }

    #[test]
    fn backup_writes_expected_filename_and_loads_back() {
        let temp = TempDir::new("backup-write");
        let store = utc_store(temp.path());
        let now = utc(2026, 11, 1, 14, 30, 5);
        let library = library_with(vec![SnippetRecord {
            created_at: now,
            updated_at: now,
            ..SnippetRecord::new("Test", vec![";t".into()], "Hello")
        }]);

        let info = store.write(&library, BackupReason::PreImport, now).unwrap();
        assert_eq!(
            info.filename,
            "kudzu-backup-2026-11-01T143005-preImport.json"
        );

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].filename,
            "kudzu-backup-2026-11-01T143005-preImport.json"
        );
        assert_eq!(list[0].reason, BackupReason::PreImport);
        assert_eq!(list[0].created_at, now);
        assert_eq!(list[0], info);
        assert_eq!(store.load(&list[0].path).unwrap(), library);
    }

    #[test]
    fn backup_lists_newest_first_and_prunes() {
        let temp = TempDir::new("backup-prune");
        let store = utc_store(temp.path());
        for day in 1..=17 {
            let reason = if day % 3 == 0 {
                BackupReason::Manual
            } else {
                BackupReason::Daily
            };
            store
                .write(
                    &KudzuLibrary::default(),
                    reason,
                    utc(2026, 10, day, 9, 0, 0),
                )
                .unwrap();
        }
        let all = store.list().unwrap();
        assert_eq!(all.len(), 17);
        assert_eq!(
            all.first().unwrap().filename,
            "kudzu-backup-2026-10-17T090000-daily.json"
        );
        assert_eq!(
            all.last().unwrap().filename,
            "kudzu-backup-2026-10-01T090000-daily.json"
        );

        assert_eq!(store.prune(14).unwrap().len(), 3);
        let remaining = store.list().unwrap();
        assert_eq!(remaining.len(), 14);
        assert_eq!(
            remaining.last().unwrap().filename,
            "kudzu-backup-2026-10-04T090000-daily.json"
        );
    }

    #[test]
    fn needs_daily_backup_uses_the_store_time_zone() {
        let temp = TempDir::new("backup-daily");
        // UTC-5, like New York in November.
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        let store = BackupStore::with_time_zone(temp.path(), BackupTimeZone::Fixed(tz));
        let local = |d, h, m| {
            tz.with_ymd_and_hms(2026, 11, d, h, m, 0)
                .unwrap()
                .with_timezone(&Utc)
        };
        let (morning, evening, next_day) = (local(1, 8, 15), local(1, 21, 45), local(2, 0, 5));

        assert!(store.needs_daily_backup(morning));
        store
            .write(&KudzuLibrary::default(), BackupReason::PreImport, morning)
            .unwrap();
        assert!(
            store.needs_daily_backup(evening),
            "a preImport backup doesn't count"
        );
        let daily = store
            .write(&KudzuLibrary::default(), BackupReason::Daily, morning)
            .unwrap();
        assert_eq!(daily.filename, "kudzu-backup-2026-11-01T081500-daily.json");
        // 21:45 local is already the next day in UTC.
        assert!(!store.needs_daily_backup(evening));
        assert!(store.needs_daily_backup(next_day));
    }

    #[test]
    fn same_second_backups_are_never_overwritten() {
        let temp = TempDir::new("backup-same-second");
        let store = utc_store(temp.path());
        let now = utc(2026, 11, 1, 12, 0, 0);
        let first = library_with(vec![SnippetRecord::new("Original", vec![";o".into()], "o")]);
        let second = library_with(vec![SnippetRecord::new("Restored", vec![";r".into()], "r")]);

        let a = store.write(&first, BackupReason::Manual, now).unwrap();
        let b = store.write(&second, BackupReason::Manual, now).unwrap();
        assert_ne!(a.path, b.path);
        assert_eq!(b.filename, "kudzu-backup-2026-11-01T120000-manual.2.json");
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            listed[0].filename, b.filename,
            "the later write sorts first"
        );
        assert_eq!(listed[0].reason, BackupReason::Manual);
        assert_eq!(store.load(&a.path).unwrap().snippets[0].name, "Original");
    }

    #[test]
    fn backup_list_ignores_foreign_files_and_missing_dirs() {
        let temp = TempDir::new("backup-foreign");
        assert!(utc_store(&temp.path().join("nope"))
            .list()
            .unwrap()
            .is_empty());
        for name in [
            "notes.txt",
            "kudzu-backup-bad-daily.json",
            "kudzu-backup-2026-11-01T120000-weird.json",
        ] {
            std::fs::write(temp.path().join(name), "{}").unwrap();
        }
        assert!(utc_store(temp.path()).list().unwrap().is_empty());
    }

    // MARK: - Coordinator

    #[test]
    fn groups_warnings_by_message_most_frequent_first() {
        let w = |file: &str, message: &str| ImportWarning {
            file: file.into(),
            line: Some(1),
            match_index: None,
            trigger: None,
            message: message.into(),
        };
        let grouped = coordinator::group_warnings(&[
            w("c.yml", "Image match skipped."),
            w("a.yml", "Markdown match imported as plain text."),
            w("b.yml", "Markdown match imported as plain text."),
        ]);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].message, "Markdown match imported as plain text.");
        assert_eq!(grouped[0].count, 2);
        assert_eq!(grouped[1].message, "Image match skipped.");
        assert_eq!(grouped[1].count, 1);
    }

    #[test]
    fn detects_and_previews_espanso_dirs_in_home() {
        let home = TempDir::new("coordinator-home");
        let match_dir = home
            .path()
            .join("Library/Application Support/espanso/match");
        std::fs::create_dir_all(&match_dir).unwrap();
        std::fs::write(
            match_dir.join("base.yml"),
            "matches:\n  - trigger: \":hi\"\n    replace: \"Hello there\"\n  - trigger: \":bye\"\n    replace: \"Goodbye\"\n",
        )
        .unwrap();
        let detected = coordinator::detect_espanso_dirs(home.path());
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].snippet_count, 2);
        assert_eq!(detected[0].group_count, 1);
    }

    #[test]
    fn import_espanso_backs_up_merges_dedupes_and_summarizes() {
        let temp = TempDir::new("coordinator-import");
        let espanso_dir = temp.path().join("espanso/match");
        std::fs::create_dir_all(&espanso_dir).unwrap();
        std::fs::write(
            espanso_dir.join("base.yml"),
            "matches:\n  - trigger: \";hi\"\n    replace: \"Hello\"\n  - trigger: \";conflict\"\n    replace: \"Incoming conflict\"\n  - trigger: \";md1\"\n    markdown: \"~~Struck 1~~\"\n  - trigger: \";md2\"\n    markdown: \"~~Struck 2~~\"\n",
        )
        .unwrap();
        let existing = library_with(vec![
            SnippetRecord::new("Existing Hi", vec![";hi".into()], "Hello"),
            SnippetRecord::new("Existing Conflict", vec![";conflict".into()], "Existing"),
        ]);
        let backups = utc_store(&temp.path().join("Backups"));

        let result = espanso::import_dir(&espanso_dir).unwrap();
        let (merged, summary) =
            coordinator::import_espanso(&existing, &result, &backups, Utc::now()).unwrap();

        assert_eq!(summary.imported_snippets, 4);
        assert_eq!(summary.imported_groups, 1);
        assert_eq!(summary.dedupe_report.merged_snippets, 1);
        assert_eq!(
            summary
                .dedupe_report
                .conflicts
                .iter()
                .map(|c| c.trigger.as_str())
                .collect::<Vec<_>>(),
            vec![";conflict"]
        );
        assert_eq!(summary.warnings.len(), 2);
        assert_eq!(summary.grouped_warnings.len(), 1);
        assert_eq!(summary.grouped_warnings[0].count, 2);

        let list = backups.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].reason, BackupReason::PreImport);
        assert_eq!(list[0], summary.backup);
        assert_eq!(backups.load(&list[0].path).unwrap().snippets.len(), 2);

        // 2 original + 4 imported - 1 merged duplicate.
        assert_eq!(merged.snippets.len(), 5);
    }

    #[test]
    fn import_kudzu_json_then_restore_with_safety_backup() {
        let temp = TempDir::new("coordinator-json");
        let backups = utc_store(&temp.path().join("Backups"));
        let original = library_with(vec![SnippetRecord::new(
            "Original",
            vec![";orig".into()],
            "Original content",
        )]);
        let incoming = KudzuLibrary {
            groups: vec![GroupRecord::new("Imported Group")],
            snippets: vec![SnippetRecord::new(
                "New",
                vec![";new".into()],
                "New content",
            )],
            ..KudzuLibrary::default()
        };
        let json = kudzu_json::encode(&incoming).unwrap();

        let t1 = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let (merged, summary) =
            coordinator::import_kudzu_json(&original, &json, &backups, t1).unwrap();
        assert_eq!(summary.imported_snippets, 1);
        assert_eq!(merged.snippets.len(), 2);
        assert_eq!(merged.groups.len(), 1);

        let before = backups.list().unwrap();
        assert_eq!(before.len(), 1);
        let t2 = Utc.timestamp_opt(1_700_000_100, 0).unwrap();
        let (restored, safety) =
            coordinator::restore_backup(&merged, &before[0].path, &backups, t2).unwrap();
        assert_eq!(
            restored
                .snippets
                .iter()
                .map(|s| s.triggers.clone())
                .collect::<Vec<_>>(),
            vec![vec![";orig".to_string()]]
        );
        let after = backups.list().unwrap();
        assert_eq!(after.len(), 2);
        assert_eq!(after[0].reason, BackupReason::PreRestore);
        assert_eq!(after[0], safety);
        assert_eq!(backups.load(&after[0].path).unwrap().snippets.len(), 2);
    }

    #[test]
    fn merge_prefers_newer_edits_takes_usage_maxima_and_drops_bad_links() {
        let t0 = utc(2026, 1, 1, 0, 0, 0);
        let t1 = utc(2026, 1, 2, 0, 0, 0);
        let root = GroupRecord::new("Root");
        let snippet = SnippetRecord {
            created_at: t0,
            updated_at: t0,
            ..SnippetRecord::new("Old", vec![";a".into()], "A")
        };
        let existing = KudzuLibrary {
            groups: vec![root.clone()],
            snippets: vec![snippet.clone()],
            usage: vec![UsageRecord {
                snippet_id: snippet.id,
                device_id: "m".into(),
                count: 5,
                chars_saved: 10,
                last_used_at: Some(t0),
            }],
            ..KudzuLibrary::default()
        };

        // An incoming child whose parent is the existing root, and a group pointing at a missing parent.
        let child = GroupRecord {
            parent_id: Some(root.id),
            ..GroupRecord::new("Child")
        };
        let orphan = GroupRecord {
            parent_id: Some(Uuid::new_v4()),
            ..GroupRecord::new("Orphan")
        };
        let edited = SnippetRecord {
            name: "New".into(),
            updated_at: t1,
            group_id: Some(Uuid::new_v4()),
            ..snippet.clone()
        };
        let stale = SnippetRecord {
            name: "Stale".into(),
            ..snippet.clone()
        };
        let incoming = KudzuLibrary {
            groups: vec![child.clone(), orphan.clone()],
            snippets: vec![edited, stale],
            usage: vec![UsageRecord {
                snippet_id: snippet.id,
                device_id: "m".into(),
                count: 3,
                chars_saved: 20,
                last_used_at: Some(t1),
            }],
            ..KudzuLibrary::default()
        };

        let (merged, report) = merge::merge_library(&existing, &incoming);
        assert!(report.is_empty());
        assert_eq!(
            merged
                .groups
                .iter()
                .find(|g| g.id == child.id)
                .unwrap()
                .parent_id,
            Some(root.id)
        );
        assert_eq!(
            merged
                .groups
                .iter()
                .find(|g| g.id == orphan.id)
                .unwrap()
                .parent_id,
            None
        );
        assert_eq!(merged.snippets.len(), 1);
        assert_eq!(merged.snippets[0].name, "New");
        assert_eq!(
            merged.snippets[0].group_id, None,
            "unknown group links are dropped"
        );
        assert_eq!(
            merged.usage,
            vec![UsageRecord {
                snippet_id: snippet.id,
                device_id: "m".into(),
                count: 5,
                chars_saved: 20,
                last_used_at: Some(t1)
            }]
        );
    }

    #[test]
    fn merge_breaks_parent_cycles() {
        let a = GroupRecord::new("A");
        let b = GroupRecord {
            parent_id: Some(a.id),
            ..GroupRecord::new("B")
        };
        let a_child_of_b = GroupRecord {
            parent_id: Some(b.id),
            ..a.clone()
        };
        let incoming = KudzuLibrary {
            groups: vec![a_child_of_b, b.clone()],
            ..KudzuLibrary::default()
        };
        let (merged, _) = merge::merge_library(&KudzuLibrary::default(), &incoming);
        let parent = |id| merged.groups.iter().find(|g| g.id == id).unwrap().parent_id;
        assert_eq!(parent(a.id), Some(b.id));
        assert_eq!(parent(b.id), None);
    }

    #[test]
    fn export_espanso_and_ensure_getting_started_once() {
        let temp = TempDir::new("coordinator-export");
        let mut library = KudzuLibrary::default();
        let now = Utc::now();
        assert!(starter::ensure_getting_started_snippets(&mut library, now));
        assert!(!starter::ensure_getting_started_snippets(&mut library, now));
        assert_eq!(
            library
                .groups
                .iter()
                .map(|g| g.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Getting Started"]
        );
        assert_eq!(library.snippets.len(), 3);
        let sample = library
            .snippets
            .iter()
            .find(|s| s.triggers == [SAMPLE_TRIGGER])
            .unwrap();
        assert_eq!(sample.content, "Kudzu is working!");

        let out = temp.path().join("exported-espanso");
        let files = coordinator::export_espanso_to_dir(&library, &out).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap(), "Getting Started.yml");
        let reimported = espanso::import_dir(&out).unwrap();
        assert_eq!(reimported.library.snippets.len(), 3);
        assert!(reimported.warnings.is_empty(), "{:?}", reimported.warnings);
    }

    #[test]
    fn getting_started_reuses_an_existing_group_and_respects_existing_sample() {
        let mut library = KudzuLibrary {
            groups: vec![GroupRecord::new("getting started")],
            ..KudzuLibrary::default()
        };
        assert!(starter::ensure_getting_started_snippets(
            &mut library,
            Utc::now()
        ));
        assert_eq!(library.groups.len(), 1);
        assert!(library
            .snippets
            .iter()
            .all(|s| s.group_id == Some(library.groups[0].id)));

        let mut taken = library_with(vec![SnippetRecord::new(
            "Mine",
            vec![";KUDZU".into()],
            "mine",
        )]);
        assert!(!starter::ensure_getting_started_snippets(
            &mut taken,
            Utc::now()
        ));
        assert_eq!(taken.snippets.len(), 1);
    }
}
