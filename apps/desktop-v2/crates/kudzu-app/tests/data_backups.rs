//! The backup store, moved from `kudzu-core`'s `interchange_library` and `golden_formats` suites
//! (ported from the Swift `BackupStoreTests`). The assertions are unchanged; only the store's
//! home is new.

mod support;

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use kudzu_app::data::{BackupStore, DataError};
use kudzu_core::interchange::backup::{BackupReason, BackupTimeZone};
use kudzu_core::library::json as kudzu_json;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use support::{fixture, utc_zone, TempDir};

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

fn library_with(snippets: Vec<SnippetRecord>) -> KudzuLibrary {
    KudzuLibrary {
        snippets,
        ..KudzuLibrary::default()
    }
}

fn utc_store(dir: &std::path::Path) -> BackupStore {
    BackupStore::with_time_zone(dir, utc_zone())
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

#[test]
fn backup_names_are_byte_exact() {
    let dir = TempDir::new("golden-backups");
    let zone = BackupTimeZone::Fixed(FixedOffset::east_opt(2 * 3600).unwrap());
    let store = BackupStore::with_time_zone(dir.path(), zone);
    let now = utc(2026, 3, 4, 23, 5, 6);
    let library = library_with(vec![SnippetRecord::new("Golden", vec![";g".into()], "g")]);
    let mut names = Vec::new();
    for reason in [
        BackupReason::Daily,
        BackupReason::PreImport,
        BackupReason::PreImport,
        BackupReason::PreRestore,
        BackupReason::Manual,
        BackupReason::PreUpdate,
    ] {
        let info = store.write(&library, reason, now).unwrap();
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
    assert_eq!(
        out,
        std::fs::read_to_string(fixture("backup-names.txt")).unwrap()
    );

    let json = std::fs::read_to_string(dir.path().join(&names[0])).unwrap();
    assert_eq!(json, kudzu_json::encode(&library).unwrap());
}

#[test]
fn backups_are_private_to_the_owner() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new("backup-mode");
    let store = utc_store(temp.path());
    let info = store
        .write(
            &KudzuLibrary::default(),
            BackupReason::Manual,
            utc(2026, 1, 1, 0, 0, 0),
        )
        .unwrap();
    let mode = std::fs::metadata(&info.path).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "backups hold snippet text");
}

#[test]
fn store_errors_say_what_happened_without_naming_paths_or_content() {
    let temp = TempDir::new("backup-errors");
    // A file where the folder should be: the store can't create it.
    let blocker = temp.path().join("backups");
    std::fs::write(&blocker, "secret snippet text").unwrap();
    let error = utc_store(&blocker)
        .write(
            &KudzuLibrary::default(),
            BackupReason::Manual,
            utc(2026, 1, 1, 0, 0, 0),
        )
        .unwrap_err();
    assert!(matches!(error, DataError::BackupWrite(_)), "{error:?}");
    let shown = format!("{error} {error:?}");
    assert!(!shown.contains("secret"), "{shown}");
    assert!(!shown.contains(temp.path().to_str().unwrap()), "{shown}");

    let garbled = temp
        .path()
        .join("kudzu-backup-2026-01-01T000000-daily.json");
    std::fs::write(&garbled, "{ secret snippet text").unwrap();
    let error = utc_store(temp.path()).load(&garbled).unwrap_err();
    let shown = format!("{error} {error:?}");
    assert!(!shown.contains("secret"), "{shown}");
}

#[test]
fn load_reads_only_backups_in_the_store_folder() {
    let temp = TempDir::new("backup-load-scope");
    let store = utc_store(&temp.path().join("backups"));
    let now = utc(2026, 11, 1, 12, 0, 0);
    let mine = library_with(vec![SnippetRecord::new("Mine", vec![";m".into()], "m")]);
    let info = store.write(&mine, BackupReason::Manual, now).unwrap();

    // A well-named file elsewhere is never read, even with the same name as a listed one.
    let elsewhere = temp.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let outside = elsewhere.join(&info.filename);
    std::fs::write(
        &outside,
        "{\"formatVersion\":1,\"groups\":[],\"snippets\":[]}",
    )
    .unwrap();
    assert_eq!(store.load(&outside).unwrap().snippets[0].name, "Mine");

    // Not in the folder at all: refused, not read.
    let stray = elsewhere.join("kudzu-backup-2026-01-01T000000-daily.json");
    std::fs::write(
        &stray,
        "{\"formatVersion\":1,\"groups\":[],\"snippets\":[]}",
    )
    .unwrap();
    assert_eq!(store.load(&stray).unwrap_err(), DataError::BackupMissing);

    // Names that aren't a backup's are refused, including traversal and arbitrary files.
    for path in [
        temp.path().join("backups/../elsewhere/notes.txt"),
        std::path::PathBuf::from("/etc/hosts"),
        std::path::PathBuf::from(".."),
    ] {
        assert_eq!(
            store.load(&path).unwrap_err(),
            DataError::BackupMissing,
            "{path:?}"
        );
    }
}
