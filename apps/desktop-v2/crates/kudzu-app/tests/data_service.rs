//! Backups, restore, the schedule and the version marker through the data service.

mod support;

use std::time::Duration;

use kudzu_app::data::{BackupStore, DataError};
use kudzu_core::interchange::backup::BackupReason;
use kudzu_core::{KudzuLibrary, SnippetRecord};
use support::{utc_zone, wait, wait_for, Rig};

fn one_snippet(name: &str, trigger: &str) -> KudzuLibrary {
    KudzuLibrary {
        snippets: vec![SnippetRecord::new(name, vec![trigger.into()], "body")],
        ..KudzuLibrary::default()
    }
}

fn store(rig: &Rig) -> BackupStore {
    BackupStore::with_time_zone(rig.backups_dir(), utc_zone())
}

#[test]
fn backup_now_writes_a_manual_backup_and_lists_it_newest_first() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let first = wait(rig.data.backup_now().unwrap()).unwrap();
    assert_eq!(first.reason, BackupReason::Manual);
    rig.clock.advance(Duration::from_secs(5));
    let second = wait(rig.data.backup_now().unwrap()).unwrap();

    let listed = rig.data.backups().get();
    assert_eq!(listed.as_slice(), [second, first.clone()]);
    // Backups keep whole seconds, so compare the content, not the timestamps.
    let saved = store(&rig).load(&first.path).unwrap();
    assert_eq!(saved.snippets[0].name, "Mine");
    assert_eq!(
        saved.snippets[0].id,
        rig.library.snapshot().1.snippets[0].id
    );
}

#[test]
fn manual_backups_prune_to_the_last_fourteen() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    for _ in 0..16 {
        rig.clock.advance(Duration::from_secs(1));
        wait(rig.data.backup_now().unwrap()).unwrap();
    }
    assert_eq!(rig.data.backups().get().len(), 14);
    assert_eq!(store(&rig).list().unwrap().len(), 14);
}

#[test]
fn restore_takes_a_pre_restore_backup_first_and_replaces_the_library() {
    let rig = Rig::new(one_snippet("Before", ";before"));
    let saved = wait(rig.data.backup_now().unwrap()).unwrap();
    rig.library
        .transact(|lib| {
            lib.snippets = vec![SnippetRecord::new("After", vec![";after".into()], "after")];
            Ok(())
        })
        .unwrap();
    rig.clock.advance(Duration::from_secs(60));

    let safety = wait(rig.data.restore(saved).unwrap()).unwrap();

    assert_eq!(safety.reason, BackupReason::PreRestore);
    assert_eq!(rig.library.snapshot().1.snippets[0].name, "Before");
    assert_eq!(
        store(&rig).load(&safety.path).unwrap().snippets[0].name,
        "After"
    );
    assert_eq!(rig.data.backups().get()[0], safety);
}

#[test]
fn restore_loads_the_backup_before_taking_the_safety_copy() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let broken = rig
        .backups_dir()
        .join("kudzu-backup-2026-01-01T000000-daily.json");
    std::fs::create_dir_all(rig.backups_dir()).unwrap();
    std::fs::write(&broken, "{ nope").unwrap();
    let info = store(&rig).list().unwrap().remove(0);

    let error = wait(rig.data.restore(info).unwrap()).unwrap_err();

    assert!(matches!(error, DataError::BackupUnreadable), "{error:?}");
    assert_eq!(
        Rig::file_names(&rig.backups_dir()).len(),
        1,
        "no safety copy"
    );
    assert_eq!(rig.library.snapshot().1.snippets[0].name, "Mine");
}

#[test]
fn restore_only_reads_files_the_store_lists() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let outside = rig
        .dir
        .path()
        .join("kudzu-backup-2026-01-01T000000-daily.json");
    std::fs::write(
        &outside,
        "{\"formatVersion\":1,\"groups\":[],\"snippets\":[]}",
    )
    .unwrap();
    let forged = kudzu_core::interchange::backup::parse_info(&outside, utc_zone()).unwrap();

    let error = wait(rig.data.restore(forged).unwrap()).unwrap_err();

    assert_eq!(error, DataError::BackupMissing);
    assert_eq!(rig.library.snapshot().1.snippets.len(), 1);
}

#[test]
fn the_daily_backup_runs_at_startup_and_every_six_hours() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let _timer = rig.data.start(&rig.scheduler).unwrap();

    // Startup: no daily backup exists for today yet.
    wait_for(&rig.data.backups(), |list| list.len() == 1);
    assert_eq!(rig.data.backups().get()[0].reason, BackupReason::Daily);

    // Six hours later it is still the same local day: the check runs and finds one.
    rig.clock.advance(Duration::from_secs(6 * 3600));
    // A day later it is a new day, so the next check backs up again.
    rig.clock.advance(Duration::from_secs(18 * 3600));
    wait_for(&rig.data.backups(), |list| list.len() == 2);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(rig.data.backups().get().len(), 2, "one per calendar day");
}

#[test]
fn the_daily_backup_prunes_to_fourteen() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let seed = store(&rig);
    for day in 1..=16 {
        seed.write(
            &KudzuLibrary::default(),
            BackupReason::Daily,
            chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 10, day, 9, 0, 0).unwrap(),
        )
        .unwrap();
    }
    let _timer = rig.data.start(&rig.scheduler).unwrap();
    wait_for(&rig.data.backups(), |list| {
        list.len() == 14 && list[0].created_at.date_naive().to_string() == "2026-11-11"
    });
}

#[test]
fn stopping_the_timer_stops_the_daily_backup() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let timer = rig.data.start(&rig.scheduler).unwrap();
    wait_for(&rig.data.backups(), |list| list.len() == 1);
    drop(timer);
    rig.clock.advance(Duration::from_secs(48 * 3600));
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(rig.data.backups().get().len(), 1);
}

#[test]
fn a_failed_daily_backup_leaves_the_library_alone_and_does_not_panic() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    std::fs::write(rig.backups_dir(), "in the way").unwrap();
    let _timer = rig.data.start(&rig.scheduler).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert!(rig.data.backups().get().is_empty());
    assert_eq!(rig.library.snapshot().1.snippets.len(), 1);
}

#[test]
fn backs_up_once_per_new_version() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let count = || {
        store(&rig)
            .list()
            .unwrap()
            .iter()
            .filter(|b| b.reason == BackupReason::PreUpdate)
            .count()
    };

    rig.data.note_app_version("0.2.0").unwrap(); // first launch with version tracking: nothing to protect yet
    assert_eq!(count(), 0);
    rig.data.note_app_version("0.2.0").unwrap();
    assert_eq!(count(), 0);
    rig.clock.advance(Duration::from_secs(1));
    rig.data.note_app_version("0.3.0").unwrap();
    assert_eq!(count(), 1);
    assert_eq!(
        rig.settings.watch().get().last_run_version.as_deref(),
        Some("0.3.0")
    );
    assert_eq!(rig.data.backups().get().len(), 1);

    // The version is remembered, so the same version does not back up again.
    rig.clock.advance(Duration::from_secs(1));
    rig.data.note_app_version("0.3.0").unwrap();
    assert_eq!(count(), 1);
}

#[test]
fn a_failed_pre_update_backup_is_retried_on_the_next_launch() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let count = || {
        store(&rig)
            .list()
            .unwrap()
            .iter()
            .filter(|b| b.reason == BackupReason::PreUpdate)
            .count()
    };
    let recorded = || rig.settings.watch().get().last_run_version.clone();
    rig.data.note_app_version("0.2.0").unwrap();

    // First launch of 0.3.0: the backup can't be written, so the version is not marked done.
    std::fs::write(rig.backups_dir(), "in the way").unwrap();
    let error = rig.data.note_app_version("0.3.0").unwrap_err();
    assert!(matches!(error, DataError::BackupWrite(_)), "{error:?}");
    assert_eq!(recorded().as_deref(), Some("0.2.0"));

    // Second launch: the folder works again, so the backup is taken and the version recorded.
    std::fs::remove_file(rig.backups_dir()).unwrap();
    rig.clock.advance(Duration::from_secs(1));
    rig.data.note_app_version("0.3.0").unwrap();
    assert_eq!(count(), 1);
    assert_eq!(recorded().as_deref(), Some("0.3.0"));

    // Third launch: already done, nothing more is taken.
    rig.clock.advance(Duration::from_secs(1));
    rig.data.note_app_version("0.3.0").unwrap();
    assert_eq!(count(), 1);
}

#[test]
fn only_one_backup_or_import_runs_at_a_time() {
    let rig = Rig::new(one_snippet("Mine", ";m"));
    let first = rig.data.backup_now().unwrap();
    // The second call races the first; either it is refused as busy or it ran after the first.
    match rig.data.backup_now() {
        Err(DataError::Busy) => {}
        Ok(second) => {
            wait(second).unwrap();
        }
        Err(other) => panic!("{other:?}"),
    }
    wait(first).unwrap();
    // Once idle again, work is accepted.
    rig.clock.advance(Duration::from_secs(1));
    wait(rig.data.backup_now().unwrap()).unwrap();
}
