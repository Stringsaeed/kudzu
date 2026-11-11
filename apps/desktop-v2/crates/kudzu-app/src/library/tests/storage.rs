use super::*;
use crate::persist::LoadSource;
use std::{fs, path::PathBuf};

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-library-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> PathBuf {
        self.0.join("library.json")
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn open(folder: &Folder, writer: &Writer) -> Result<(LibraryService, LoadSource), LibraryError> {
    LibraryService::open(
        folder.path(),
        "device-a".into(),
        Arc::new(FakeClock::default()),
        Arc::new(FixedIds::new(100)),
        writer,
        Arc::new(crate::schedule::Scheduler::new(Arc::new(FakeClock::default())).unwrap()),
    )
}
fn writer() -> Writer {
    Writer::new(
        Arc::new(FakeClock::default()),
        Duration::from_secs(30),
        Duration::from_secs(1),
    )
    .unwrap()
}

#[test]
fn getting_started_is_seeded_only_when_the_file_is_absent_and_uses_injected_ids() {
    let folder = Folder::new();
    let writes = writer();
    let (library, source) = open(&folder, &writes).unwrap();
    assert_eq!(source, LoadSource::Missing);
    let snapshot = library.watch().get();
    assert_eq!(snapshot.groups[0].name, "Getting Started");
    assert_eq!(snapshot.groups[0].id, Uuid::from_u128(100));
    assert_eq!(
        snapshot
            .snippets
            .iter()
            .map(|s| s.triggers[0].as_str())
            .collect::<Vec<_>>(),
        [";kudzu", ";date", ";sig"]
    );
    assert_eq!(snapshot.snippets[0].id, Uuid::from_u128(101));
    assert_eq!(snapshot.snippets[0].created_at.timestamp(), 1_794_398_400);
    assert_eq!(snapshot.snippets[0].content, "Kudzu is working!");
    assert_eq!(library.saved().get().durable, Revision(0));
    let (_, revision) = library
        .transact(|lib| {
            lib.snippets.clear();
            lib.groups.clear();
            Ok(())
        })
        .unwrap();
    writes.shutdown().unwrap();
    assert_eq!(library.saved().get().durable, revision);
    let writes = writer();
    let (again, source) = open(&folder, &writes).unwrap();
    assert_eq!(source, LoadSource::Existing);
    assert!(again.watch().get().snippets.is_empty());
    assert_eq!(again.saved().get().durable, again.watch().revision());
    writes.shutdown().unwrap();
}

#[test]
fn corrupt_library_is_quarantined_without_seed_or_content_in_errors() {
    let folder = Folder::new();
    let bytes = b"PRIVATE-BODY invalid json";
    fs::write(folder.path(), bytes).unwrap();
    let writes = writer();
    let (library, source) = open(&folder, &writes).unwrap();
    let LoadSource::Recovered(aside) = source else {
        panic!("not quarantined")
    };
    assert!(library.watch().get().snippets.is_empty());
    assert!(library.watch().get().groups.is_empty());
    writes.shutdown().unwrap();
    assert_eq!(fs::read(aside).unwrap(), bytes);
    assert_eq!(
        fs::read_to_string(folder.path()).unwrap(),
        "{\n  \"formatVersion\": 1,\n  \"groups\": [],\n  \"snippets\": [],\n  \"usage\": []\n}"
    );
}

#[test]
fn future_library_is_not_quarantined_or_overwritten() {
    let folder = Folder::new();
    let bytes = br#"{"formatVersion":999,"private":"PRIVATE-BODY"}"#;
    fs::write(folder.path(), bytes).unwrap();
    let writes = writer();
    assert!(matches!(
        open(&folder, &writes),
        Err(LibraryError::Persistence(PersistError::UnsupportedVersion))
    ));
    writes.shutdown().unwrap();
    assert_eq!(fs::read(folder.path()).unwrap(), bytes);
    assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 1);
}

#[test]
fn service_round_trips_the_s20_golden_library_byte_for_byte() {
    let golden = include_str!("../../../../kudzu-core/tests/fixtures/golden/library.json");
    let folder = Folder::new();
    fs::write(folder.path(), golden).unwrap();
    let writes = writer();
    let (library, _) = open(&folder, &writes).unwrap();
    let original = library.watch().get();
    library
        .transact(|lib| {
            lib.snippets[0].name = "Temporary edit".into();
            Ok(())
        })
        .unwrap();
    let (_, revision) = library
        .transact(|lib| {
            *lib = (*original).clone();
            Ok(())
        })
        .unwrap();
    writes.shutdown().unwrap();
    assert_eq!(library.saved().get().durable, revision);
    assert_eq!(fs::read_to_string(folder.path()).unwrap(), golden);
}

#[cfg(unix)]
#[test]
fn disk_failure_keeps_accepted_edits_visible_and_retry_makes_them_durable() {
    use std::os::unix::fs::PermissionsExt;
    let folder = Folder::new();
    fs::write(folder.path(), "{\"formatVersion\":1}").unwrap();
    let clock = Arc::new(FakeClock::default());
    let writes = Writer::new(
        clock.clone(),
        Duration::from_millis(100),
        Duration::from_secs(1),
    )
    .unwrap();
    let scheduler = Arc::new(crate::schedule::Scheduler::new(clock.clone()).unwrap());
    let (library, _) = LibraryService::open(
        folder.path(),
        "device-a".into(),
        clock.clone(),
        Arc::new(FixedIds::new(100)),
        &writes,
        scheduler,
    )
    .unwrap();
    let saved = library.saved();
    let read = saved.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let _sub = saved.subscribe(move || {
        tx.send(*read.get()).unwrap();
    });
    fs::set_permissions(&folder.0, fs::Permissions::from_mode(0o500)).unwrap();
    struct RestorePermissions(PathBuf);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    let permissions = RestorePermissions(folder.0.clone());
    let (_, revision) = library
        .create_group("Accepted while disk fails".into(), None)
        .unwrap();
    clock.advance(Duration::from_millis(100));
    let failed = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(failed.error.is_some());
    assert!(failed.durable < revision);
    assert_eq!(
        library.watch().get().groups[0].name,
        "Accepted while disk fails"
    );
    drop(permissions);
    clock.advance(Duration::from_secs(1));
    let saved = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(saved.durable, revision);
    assert!(saved.error.is_none());
    writes.shutdown().unwrap();
    let writes = writer();
    let (again, _) = open(&folder, &writes).unwrap();
    assert_eq!(
        again.watch().get().groups[0].name,
        "Accepted while disk fails"
    );
    writes.shutdown().unwrap();
}
