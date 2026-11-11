//! Kudzu JSON, the backup rules and merging, ported from the Swift `KudzuJSONTests`,
//! `BackupStoreTests` and `ImportCoordinatorTests`. The backup folder and the import, restore
//! and export orchestration are tested in `kudzu-app` (`tests/data_*.rs`).

mod interchange_support;

/// Named so `cargo test interchange` selects these.
mod interchange {
    use crate::interchange_support::TempDir;
    use chrono::{DateTime, FixedOffset, TimeZone, Utc};
    use kudzu_core::interchange::backup::{self, BackupInfo, BackupReason, BackupTimeZone};
    use kudzu_core::interchange::espanso;
    use kudzu_core::interchange::{merge, InterchangeError};
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

    // MARK: - Backup rules (the folder itself is `kudzu_app::data::BackupStore`)

    fn info_at(utc_time: DateTime<Utc>, reason: BackupReason) -> BackupInfo {
        let zone = BackupTimeZone::Fixed(FixedOffset::east_opt(0).unwrap());
        let name = backup::filename(zone.utc_to_local(utc_time), reason, 1);
        backup::parse_info(std::path::Path::new(&name), zone).unwrap()
    }

    #[test]
    fn backup_names_round_trip_and_number_same_second_copies() {
        let zone = BackupTimeZone::Fixed(FixedOffset::east_opt(0).unwrap());
        let now = utc(2026, 11, 1, 14, 30, 5);
        let local = zone.utc_to_local(now);
        assert_eq!(
            backup::filename(local, BackupReason::PreImport, 1),
            "kudzu-backup-2026-11-01T143005-preImport.json"
        );
        assert_eq!(
            backup::filename(local, BackupReason::Manual, 2),
            "kudzu-backup-2026-11-01T143005-manual.2.json"
        );
        for reason in [
            BackupReason::Daily,
            BackupReason::PreImport,
            BackupReason::PreRestore,
            BackupReason::Manual,
            BackupReason::PreUpdate,
        ] {
            for sequence in [1, 2, 7] {
                let name = backup::filename(local, reason, sequence);
                let info = backup::parse_info(std::path::Path::new(&name), zone).unwrap();
                assert_eq!(
                    (info.created_at, info.reason, info.filename),
                    (now, reason, name)
                );
            }
        }
    }

    #[test]
    fn backup_parsing_ignores_foreign_names() {
        let zone = BackupTimeZone::Fixed(FixedOffset::east_opt(0).unwrap());
        for name in [
            "notes.txt",
            "kudzu-backup-bad-daily.json",
            "kudzu-backup-2026-11-01T120000-weird.json",
            "kudzu-backup-2026-11-01T1200-daily.json",
        ] {
            assert!(
                backup::parse_info(std::path::Path::new(name), zone).is_none(),
                "{name}"
            );
        }
    }

    #[test]
    fn backups_sort_newest_first_and_rotation_keeps_the_newest() {
        let mut infos: Vec<BackupInfo> = (1..=17)
            .map(|day| {
                let reason = if day % 3 == 0 {
                    BackupReason::Manual
                } else {
                    BackupReason::Daily
                };
                info_at(utc(2026, 10, day, 9, 0, 0), reason)
            })
            .collect();
        backup::sort_newest(&mut infos);
        assert_eq!(
            infos[0].filename,
            "kudzu-backup-2026-10-17T090000-daily.json"
        );
        assert_eq!(
            infos[16].filename,
            "kudzu-backup-2026-10-01T090000-daily.json"
        );

        let doomed = backup::rotation_candidates(infos, 14);
        assert_eq!(
            doomed
                .iter()
                .map(|i| i.filename.as_str())
                .collect::<Vec<_>>(),
            [
                "kudzu-backup-2026-10-03T090000-manual.json",
                "kudzu-backup-2026-10-02T090000-daily.json",
                "kudzu-backup-2026-10-01T090000-daily.json",
            ]
        );
    }

    #[test]
    fn daily_rule_uses_the_local_calendar_day_and_only_counts_daily_backups() {
        // UTC-5, like New York in November.
        let tz = FixedOffset::west_opt(5 * 3600).unwrap();
        let zone = BackupTimeZone::Fixed(tz);
        let local = |d, h, m| {
            tz.with_ymd_and_hms(2026, 11, d, h, m, 0)
                .unwrap()
                .with_timezone(&Utc)
        };
        let (morning, evening, next_day) = (local(1, 8, 15), local(1, 21, 45), local(2, 0, 5));
        let named = |when, reason| {
            let name = backup::filename(zone.utc_to_local(when), reason, 1);
            backup::parse_info(std::path::Path::new(&name), zone).unwrap()
        };

        assert!(backup::needs_daily_backup(&[], morning, zone));
        let pre_import = [named(morning, BackupReason::PreImport)];
        assert!(
            backup::needs_daily_backup(&pre_import, evening, zone),
            "a preImport backup doesn't count"
        );
        let daily = [named(morning, BackupReason::Daily)];
        // 21:45 local is already the next day in UTC.
        assert!(!backup::needs_daily_backup(&daily, evening, zone));
        assert!(backup::needs_daily_backup(&daily, next_day, zone));
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
    fn ensure_getting_started_once_and_export_round_trips_through_espanso() {
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
        let files = espanso::export(&library);
        assert_eq!(files.keys().collect::<Vec<_>>(), ["Getting Started.yml"]);
        crate::interchange_support::write_files(&out, files.iter());
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
