use super::*;
use kudzu_core::library::model::GroupRecord;
use std::sync::{mpsc, Barrier};

#[test]
fn concurrent_collision_checks_and_creates_are_one_transaction() {
    let h = Harness::new(KudzuLibrary::default());
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (0..2)
        .map(|n| {
            let service = h.service.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                service.transact(|library| {
                    if library
                        .snippets
                        .iter()
                        .any(|s| s.triggers.contains(&";same".into()))
                    {
                        return Err(LibraryError::TriggerTaken);
                    }
                    let mut snippet = SnippetRecord::new("Same", vec![";same".into()], "content");
                    snippet.id = Uuid::from_u128(n + 1);
                    library.snippets.push(snippet);
                    Ok(())
                })
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| **r == Err(LibraryError::TriggerTaken))
            .count(),
        1
    );
    assert_eq!(h.service.watch().get().snippets.len(), 1);
    assert_eq!(h.service.watch().revision(), Revision(2));
}

#[test]
fn failed_or_panicking_closure_leaves_no_groups_and_future_edits_work() {
    let h = Harness::new(KudzuLibrary::default());
    let before = h.service.snapshot();
    let result: Result<((), Revision), _> = h.service.transact(|library| {
        library.groups.push(GroupRecord::new("Never committed"));
        Err(LibraryError::TriggerTaken)
    });
    assert_eq!(result, Err(LibraryError::TriggerTaken));
    assert_eq!(h.service.snapshot(), before);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        h.service.transact::<()>(|library| {
            library
                .groups
                .push(GroupRecord::new("Also never committed"));
            panic!("transaction failed")
        })
    }))
    .is_err());
    assert_eq!(h.service.snapshot(), before);
    h.service.create_group("Committed".into(), None).unwrap();
    assert_eq!(h.service.watch().get().groups.len(), 1);
}

#[test]
fn closure_uses_the_latest_commit_even_when_its_caller_has_an_old_snapshot() {
    let h = Harness::new(KudzuLibrary::default());
    let old = h.service.watch().get();
    let worker = h.service.clone();
    std::thread::spawn(move || worker.create_group("Concurrent edit".into(), None).unwrap())
        .join()
        .unwrap();
    let (count, revision) = h
        .service
        .transact(|library| {
            assert_eq!(library.groups[0].name, "Concurrent edit");
            library.groups[0].is_enabled = false;
            Ok(library.groups.len())
        })
        .unwrap();
    assert_eq!(count, 1);
    assert!(old.groups.is_empty());
    assert_eq!(revision, Revision(3));
}

#[test]
fn notifications_run_after_unlock_and_stopped_writer_rejects_without_mutation() {
    let h = Harness::new(KudzuLibrary::default());
    let reader = h.service.clone();
    let (tx, rx) = mpsc::channel();
    let _sub = h.service.watch().subscribe(move || {
        tx.send(reader.transact(|_| Ok(())).unwrap().1).unwrap();
    });
    let writer = h.service.clone();
    let thread = std::thread::spawn(move || writer.create_group("Group".into(), None).unwrap());
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        Revision(2)
    );
    thread.join().unwrap();
    h.writer.shutdown().unwrap();
    let before = h.service.snapshot();
    assert!(matches!(
        h.service.create_group("Too late".into(), None),
        Err(LibraryError::Persistence(PersistError::Stopped))
    ));
    assert_eq!(h.service.snapshot(), before);
}
