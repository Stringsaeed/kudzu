//! The retry rule of `run_backed_up`, which needs a library edit at an exact moment.

use super::*;
use crate::fixtures::{clock::FakeClock, ids::FixedIds};
use crate::persist::{PersistError, Revision, WriteTarget, Writer};
use crate::settings::{Settings, Variant};
use kudzu_core::interchange::backup::BackupTimeZone;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use std::cell::Cell;

struct Memory;
impl<T> WriteTarget<T> for Memory {
    fn write(&self, _: &T) -> Result<(), PersistError> {
        Ok(())
    }
}

struct Folder(PathBuf);
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Rig {
    inner: Arc<Inner>,
    library: Arc<LibraryService>,
    _writer: Writer,
    folder: Folder,
}

fn rig() -> Rig {
    let folder = Folder(std::env::temp_dir().join(format!("kudzu-data-{}", uuid::Uuid::new_v4())));
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(30),
        Duration::from_secs(1),
    )
    .unwrap();
    let library = Arc::new(
        LibraryService::new(
            KudzuLibrary {
                snippets: vec![SnippetRecord::new("Mine", vec![";m".into()], "mine")],
                ..KudzuLibrary::default()
            },
            "device".into(),
            clock.clone(),
            Arc::new(FixedIds::new(1)),
            writer.register(Memory, Revision(0)).unwrap(),
            Arc::new(Scheduler::new(clock.clone()).unwrap()),
        )
        .unwrap(),
    );
    let settings = Arc::new(
        SettingsService::new(
            Settings::for_variant(Variant::Production),
            writer.register(Memory, Revision(0)).unwrap(),
        )
        .unwrap(),
    );
    let service = DataService::new(
        DataConfig {
            backups: folder.0.join("backups"),
            home: folder.0.join("home"),
            time_zone: BackupTimeZone::Fixed(chrono::FixedOffset::east_opt(0).unwrap()),
        },
        library.clone(),
        settings,
        clock,
    );
    Rig {
        inner: service.inner,
        library,
        _writer: writer,
        folder,
    }
}

fn edit(library: &LibraryService, name: &str) {
    library
        .transact(|lib| {
            lib.snippets
                .push(SnippetRecord::new(name, vec![format!(";{name}")], name));
            Ok(())
        })
        .unwrap();
}

#[test]
fn an_edit_between_backup_and_commit_restarts_against_the_new_library() {
    let rig = rig();
    let attempts = Cell::new(0);

    let (backup, applied_to) = rig
        .inner
        .run_backed_up(
            BackupReason::PreImport,
            |attempt| {
                attempts.set(attempt + 1);
                if attempt == 0 {
                    edit(&rig.library, "agent");
                }
            },
            |library| library.snippets.len(),
        )
        .unwrap();

    assert_eq!(attempts.get(), 2);
    assert_eq!(applied_to, 2, "the change saw the agent's edit");
    // (Backups keep whole seconds, so compare what the library held, not its timestamps.)
    let names = |library: &KudzuLibrary| {
        library
            .snippets
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        names(&rig.inner.store.load(&backup.path).unwrap()),
        names(&rig.library.snapshot().1),
        "the surviving backup is of exactly the library the change replaced"
    );
    assert_eq!(
        rig.inner.store.list().unwrap(),
        vec![backup],
        "the abandoned attempt's backup is gone"
    );
}

#[test]
fn a_library_that_never_settles_gives_up_cleanly() {
    let rig = rig();
    let mut count = 0;
    let error = rig
        .inner
        .run_backed_up(
            BackupReason::PreImport,
            |_| {
                count += 1;
                edit(&rig.library, &format!("busy{count}"));
            },
            |library| library.snippets.clear(),
        )
        .unwrap_err();
    assert_eq!(error, DataError::LibraryBusy);
    assert!(
        !rig.library.snapshot().1.snippets.is_empty(),
        "the change was never applied"
    );
    assert!(
        rig.inner.store.list().unwrap().is_empty(),
        "no stray backups"
    );
    drop(rig.folder);
}

#[test]
fn a_change_that_alters_nothing_still_returns_its_backup() {
    let rig = rig();
    let (backup, ()) = rig
        .inner
        .run_backed_up(BackupReason::PreRestore, |_| {}, |_| {})
        .unwrap();
    assert_eq!(rig.inner.store.list().unwrap(), vec![backup]);
}
