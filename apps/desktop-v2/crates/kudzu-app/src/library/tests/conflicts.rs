use super::*;
use std::sync::mpsc;

#[test]
fn conflicts_debounce_from_the_latest_change_and_publish_on_a_worker() {
    let h = Harness::new(KudzuLibrary::default());
    let read = h.service.conflicts();
    let observed = read.clone();
    let (tx, rx) = mpsc::channel();
    let _sub = read.subscribe(move || {
        tx.send((
            observed.get(),
            std::thread::current().name().unwrap_or("").to_owned(),
        ))
        .unwrap();
    });
    let (a, _) = h
        .service
        .create_snippet(SnippetDraft {
            triggers: vec![";a".into()],
            ..Default::default()
        })
        .unwrap();
    h.clock.advance(Duration::from_millis(200));
    let (b, revision) = h
        .service
        .create_snippet(SnippetDraft {
            triggers: vec![";A".into()],
            ..Default::default()
        })
        .unwrap();
    h.clock.advance(Duration::from_millis(249));
    assert!(rx.recv_timeout(Duration::from_millis(20)).is_err());
    h.clock.advance(Duration::from_millis(1));
    let (conflicts, worker) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(worker, "kudzu-conflicts");
    assert_eq!(conflicts.revision, revision);
    assert_eq!(conflicts.items.len(), 1);
    assert_eq!(conflicts.items[0].snippet_id, b.id);
    assert_eq!(conflicts.items[0].other_snippet_id, a.id);
    let mut disabled = b;
    disabled.is_enabled = false;
    let (_, revision) = h.service.update_snippet(disabled).unwrap();
    h.clock.advance(Duration::from_millis(250));
    let (conflicts, _) = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(conflicts.revision, revision);
    assert!(conflicts.items.is_empty());
    assert!(conflicts.error.is_none());
}

#[test]
fn stopped_scheduler_reports_a_conflict_error_without_rejecting_the_edit() {
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let scheduler = Arc::new(crate::schedule::Scheduler::new(clock.clone()).unwrap());
    let service = LibraryService::new(
        KudzuLibrary::default(),
        "device-a".into(),
        clock,
        Arc::new(FixedIds::new(100)),
        writer.register(Memory, Revision(0)).unwrap(),
        scheduler.clone(),
    )
    .unwrap();
    scheduler.shutdown();
    let (_, revision) = service.create_snippet(SnippetDraft::default()).unwrap();
    assert_eq!(revision, Revision(2));
    assert_eq!(service.watch().get().snippets.len(), 1);
    assert_eq!(
        service.conflicts().get().error,
        Some(LibraryError::Schedule(
            crate::schedule::ScheduleError::Stopped
        ))
    );
    writer.shutdown().unwrap();
}
