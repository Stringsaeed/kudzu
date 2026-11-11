//! Import, detection and export through the data service. The cases that exercised
//! `kudzu-core`'s coordinator move here with their assertions unchanged; the new ones pin what
//! the service adds: parse before mutating, the pre-import backup first, and merging against the
//! library as it is when the import commits.

mod support;

use kudzu_app::data::{
    detect_import_source, group_warnings, load_import, DataError, DataService, ImportSummary,
};
use kudzu_core::interchange::backup::BackupReason;
use kudzu_core::interchange::detect::ImportSource;
use kudzu_core::interchange::espanso::{self, ImportWarning};
use kudzu_core::library::json as kudzu_json;
use kudzu_core::library::starter;
use kudzu_core::{GroupRecord, KudzuLibrary, SnippetRecord};
use support::{fixture, wait, Rig, TempDir};

fn library_with(snippets: Vec<SnippetRecord>) -> KudzuLibrary {
    KudzuLibrary {
        snippets,
        ..KudzuLibrary::default()
    }
}

fn import(data: &DataService, path: std::path::PathBuf) -> Result<ImportSummary, DataError> {
    wait(data.import(path).unwrap())
}

#[test]
fn groups_warnings_by_message_most_frequent_first() {
    let w = |file: &str, message: &str| ImportWarning {
        file: file.into(),
        line: Some(1),
        match_index: None,
        trigger: None,
        message: message.into(),
    };
    let grouped = group_warnings(&[
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
    let rig = Rig::new(KudzuLibrary::default());
    let match_dir = rig
        .dir
        .path()
        .join("home/Library/Application Support/espanso/match");
    std::fs::create_dir_all(&match_dir).unwrap();
    std::fs::write(
        match_dir.join("base.yml"),
        "matches:\n  - trigger: \":hi\"\n    replace: \"Hello there\"\n  - trigger: \":bye\"\n    replace: \"Goodbye\"\n",
    )
    .unwrap();
    let detected = wait(rig.data.detect_espanso().unwrap());
    assert_eq!(detected.len(), 1);
    assert_eq!(detected[0].snippet_count, 2);
    assert_eq!(detected[0].group_count, 1);
}

#[test]
fn detection_reports_only_folders_with_snippets() {
    let rig = Rig::new(KudzuLibrary::default());
    let empty = rig
        .dir
        .path()
        .join("home/Library/Application Support/espanso/match");
    std::fs::create_dir_all(&empty).unwrap();
    std::fs::write(empty.join("base.yml"), "matches: []\n").unwrap();
    assert!(wait(rig.data.detect_espanso().unwrap()).is_empty());
    // No Espanso at all is not an error either.
    let bare = Rig::new(KudzuLibrary::default());
    assert!(wait(bare.data.detect_espanso().unwrap()).is_empty());
}

const ESPANSO: &str = "matches:\n  - trigger: \";hi\"\n    replace: \"Hello\"\n  - trigger: \";conflict\"\n    replace: \"Incoming conflict\"\n  - trigger: \";md1\"\n    markdown: \"~~Struck 1~~\"\n  - trigger: \";md2\"\n    markdown: \"~~Struck 2~~\"\n";

fn espanso_import_rig() -> (Rig, std::path::PathBuf) {
    let existing = library_with(vec![
        SnippetRecord::new("Existing Hi", vec![";hi".into()], "Hello"),
        SnippetRecord::new("Existing Conflict", vec![";conflict".into()], "Existing"),
    ]);
    let rig = Rig::new(existing);
    let espanso_dir = rig.dir.path().join("espanso/match");
    std::fs::create_dir_all(&espanso_dir).unwrap();
    std::fs::write(espanso_dir.join("base.yml"), ESPANSO).unwrap();
    (rig, espanso_dir)
}

#[test]
fn import_espanso_backs_up_merges_dedupes_and_summarizes() {
    let (rig, espanso_dir) = espanso_import_rig();

    let summary = import(&rig.data, espanso_dir).unwrap();

    assert_eq!(summary.source, ImportSource::Espanso);
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

    let list = rig.data.backups().get();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].reason, BackupReason::PreImport);
    assert_eq!(list[0], summary.backup);
    let store =
        kudzu_app::data::BackupStore::with_time_zone(rig.backups_dir(), support::utc_zone());
    assert_eq!(store.load(&list[0].path).unwrap().snippets.len(), 2);

    // 2 original + 4 imported - 1 merged duplicate.
    let (_, merged) = rig.library.snapshot();
    assert_eq!(merged.snippets.len(), 5);
    // The summary's net-new counts are what the library actually gained.
    assert_eq!(summary.added_snippets, 3);
    assert_eq!(summary.added_groups, 1);
}

#[test]
fn a_single_espanso_file_imports_too() {
    let (rig, espanso_dir) = espanso_import_rig();
    let summary = import(&rig.data, espanso_dir.join("base.yml")).unwrap();
    assert_eq!(summary.source, ImportSource::Espanso);
    assert_eq!(summary.imported_snippets, 4);
}

#[test]
fn import_kudzu_json_then_restore_with_safety_backup() {
    let original = library_with(vec![SnippetRecord::new(
        "Original",
        vec![";orig".into()],
        "Original content",
    )]);
    let rig = Rig::new(original);
    let incoming = KudzuLibrary {
        groups: vec![GroupRecord::new("Imported Group")],
        snippets: vec![SnippetRecord::new(
            "New",
            vec![";new".into()],
            "New content",
        )],
        ..KudzuLibrary::default()
    };
    let file = rig.dir.path().join("incoming.json");
    std::fs::write(&file, kudzu_json::encode(&incoming).unwrap()).unwrap();

    let summary = import(&rig.data, file).unwrap();
    assert_eq!(summary.source, ImportSource::Kudzu);
    assert_eq!(summary.imported_snippets, 1);
    let (_, merged) = rig.library.snapshot();
    assert_eq!(merged.snippets.len(), 2);
    assert_eq!(merged.groups.len(), 1);

    let before = rig.data.backups().get();
    assert_eq!(before.len(), 1);
    rig.clock.advance(std::time::Duration::from_secs(100));
    wait(rig.data.restore(before[0].clone()).unwrap()).unwrap();
    let (_, restored) = rig.library.snapshot();
    assert_eq!(
        restored
            .snippets
            .iter()
            .map(|s| s.triggers.clone())
            .collect::<Vec<_>>(),
        vec![vec![";orig".to_string()]]
    );
    let after = rig.data.backups().get();
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].reason, BackupReason::PreRestore);
    let store =
        kudzu_app::data::BackupStore::with_time_zone(rig.backups_dir(), support::utc_zone());
    assert_eq!(store.load(&after[0].path).unwrap().snippets.len(), 2);
}

#[test]
fn export_espanso_and_ensure_getting_started_once() {
    let mut library = KudzuLibrary::default();
    assert!(starter::ensure_getting_started_snippets(
        &mut library,
        chrono::Utc::now()
    ));
    let rig = Rig::new(library);
    let out = rig.dir.path().join("exported-espanso");
    let count = wait(rig.data.export_espanso(out.clone()).unwrap()).unwrap();
    assert_eq!(count, 1, "the file count");
    assert_eq!(Rig::file_names(&out), vec!["Getting Started.yml"]);
    let reimported = espanso::import_dir(&out).unwrap();
    assert_eq!(reimported.library.snippets.len(), 3);
    assert!(reimported.warnings.is_empty(), "{:?}", reimported.warnings);
}

#[test]
fn json_export_is_the_library_and_is_named_by_date() {
    let rig = Rig::new(library_with(vec![SnippetRecord::new(
        "Exported",
        vec![";x".into()],
        "x",
    )]));
    // The fake clock starts at 2026-11-11 12:00 UTC.
    assert_eq!(rig.data.export_json_name(), "kudzu-2026-11-11.json");
    let file = rig.dir.path().join(rig.data.export_json_name());
    wait(rig.data.export_json(file.clone()).unwrap()).unwrap();
    let (_, current) = rig.library.snapshot();
    assert_eq!(
        std::fs::read_to_string(file).unwrap(),
        kudzu_json::encode(&current).unwrap()
    );
}

#[test]
fn detection_and_loading_by_extension() {
    let dir = TempDir::new("te");
    let file = dir.path().join("Work.textexpander");
    std::fs::copy(fixture("work.textexpander"), &file).unwrap();
    assert_eq!(
        detect_import_source(&file).unwrap(),
        ImportSource::TextExpander
    );
    assert_eq!(load_import(&file).unwrap().library.snippets.len(), 5);
    let other = dir.path().join("notes.txt");
    std::fs::write(&other, "x").unwrap();
    assert!(detect_import_source(&other).is_err());
    assert_eq!(
        detect_import_source(&other).unwrap_err(),
        DataError::UnsupportedFile
    );
}

#[test]
fn detection_tells_raycast_from_kudzu_json() {
    let dir = TempDir::new("raycast");
    let raycast_file = dir.path().join("snippets.json");
    let kudzu_file = dir.path().join("library.json");
    std::fs::copy(fixture("raycast-export.json"), &raycast_file).unwrap();
    std::fs::write(
        &kudzu_file,
        "{\"formatVersion\":1,\"groups\":[],\"snippets\":[]}",
    )
    .unwrap();
    assert_eq!(
        detect_import_source(&raycast_file).unwrap(),
        ImportSource::Raycast
    );
    assert_eq!(
        detect_import_source(&kudzu_file).unwrap(),
        ImportSource::Kudzu
    );
    assert_eq!(
        load_import(&raycast_file).unwrap().library.snippets.len(),
        9
    );
}

#[test]
fn text_expander_and_raycast_files_import_through_the_service() {
    let rig = Rig::new(KudzuLibrary::default());
    let te = rig.dir.path().join("Work.textexpander");
    std::fs::copy(fixture("work.textexpander"), &te).unwrap();
    let summary = import(&rig.data, te).unwrap();
    assert_eq!(summary.source, ImportSource::TextExpander);
    assert_eq!(summary.added_snippets, 5);

    let raycast = rig.dir.path().join("snippets.json");
    std::fs::copy(fixture("raycast-export.json"), &raycast).unwrap();
    let summary = import(&rig.data, raycast).unwrap();
    assert_eq!(summary.source, ImportSource::Raycast);
    assert_eq!(summary.imported_snippets, 9);
    assert_eq!(rig.library.snapshot().1.snippets.len(), 14);
}

// MARK: - What the service adds

#[test]
fn a_bad_file_changes_nothing_and_takes_no_backup() {
    let rig = Rig::new(library_with(vec![SnippetRecord::new(
        "Mine",
        vec![";m".into()],
        "mine",
    )]));
    let (before_revision, before) = rig.library.snapshot();
    let bad = rig.dir.path().join("broken.json");
    std::fs::write(&bad, "{ \"formatVersion\": 1, \"snippets\": SECRET-BODY").unwrap();

    let error = import(&rig.data, bad).unwrap_err();

    assert!(matches!(error, DataError::Invalid(_)), "{error:?}");
    let shown = format!("{error} {error:?}");
    assert!(!shown.contains("SECRET-BODY"), "{shown}");
    let (after_revision, after) = rig.library.snapshot();
    assert_eq!(after_revision, before_revision);
    assert_eq!(after, before);
    assert!(rig.data.backups().get().is_empty());
    assert!(Rig::file_names(&rig.backups_dir()).is_empty());

    let unsupported = rig.dir.path().join("notes.txt");
    std::fs::write(&unsupported, "x").unwrap();
    assert_eq!(
        import(&rig.data, unsupported).unwrap_err(),
        DataError::UnsupportedFile
    );
    let missing = rig.dir.path().join("missing.json");
    assert!(matches!(
        import(&rig.data, missing).unwrap_err(),
        DataError::Read(_)
    ));
}

#[test]
fn a_newer_library_format_is_refused_with_both_versions() {
    let rig = Rig::new(KudzuLibrary::default());
    let future = KudzuLibrary {
        format_version: KudzuLibrary::CURRENT_FORMAT_VERSION + 1,
        ..KudzuLibrary::default()
    };
    let file = rig.dir.path().join("future.json");
    std::fs::write(&file, kudzu_json::encode(&future).unwrap()).unwrap();
    let error = import(&rig.data, file).unwrap_err();
    assert_eq!(
        error,
        DataError::UnsupportedVersion {
            found: KudzuLibrary::CURRENT_FORMAT_VERSION + 1,
            max_supported: KudzuLibrary::CURRENT_FORMAT_VERSION,
        }
    );
}

#[test]
fn the_pre_import_backup_must_succeed_before_anything_merges() {
    let (rig, espanso_dir) = espanso_import_rig();
    // A file where the backups folder should be: no backup can be written.
    std::fs::write(rig.backups_dir(), "in the way").unwrap();
    let (before_revision, before) = rig.library.snapshot();

    let error = import(&rig.data, espanso_dir).unwrap_err();

    assert!(matches!(error, DataError::BackupRequired(_)), "{error:?}");
    let (after_revision, after) = rig.library.snapshot();
    assert_eq!((after_revision, after), (before_revision, before));
}

#[test]
fn a_restore_needs_its_safety_backup_too() {
    use std::os::unix::fs::PermissionsExt;
    let rig = Rig::new(library_with(vec![SnippetRecord::new(
        "Mine",
        vec![";m".into()],
        "mine",
    )]));
    let saved = wait(rig.data.backup_now().unwrap()).unwrap();
    rig.library
        .transact(|lib| {
            lib.snippets.clear();
            Ok(())
        })
        .unwrap();
    let (revision, current) = rig.library.snapshot();
    // Readable but not writable: the backup can be loaded, a safety copy can't be written.
    let folder = rig.backups_dir();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500)).unwrap();

    let result = wait(rig.data.restore(saved).unwrap());

    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(
        matches!(result, Err(DataError::BackupRequired(_))),
        "{result:?}"
    );
    let (after_revision, after) = rig.library.snapshot();
    assert_eq!((after_revision, after), (revision, current));
}

#[test]
fn imports_do_not_prune_backups() {
    let (rig, espanso_dir) = espanso_import_rig();
    let store =
        kudzu_app::data::BackupStore::with_time_zone(rig.backups_dir(), support::utc_zone());
    for day in 1..=16 {
        store
            .write(
                &KudzuLibrary::default(),
                BackupReason::Daily,
                chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 10, day, 9, 0, 0).unwrap(),
            )
            .unwrap();
    }
    import(&rig.data, espanso_dir).unwrap();
    assert_eq!(
        store.list().unwrap().len(),
        17,
        "only daily and manual prune"
    );
}
