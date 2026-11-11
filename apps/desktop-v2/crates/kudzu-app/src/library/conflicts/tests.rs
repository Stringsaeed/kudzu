use super::*;
use crate::fixtures::clock::FakeClock;
use kudzu_core::library::model::SnippetRecord;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[test]
fn panicking_conflict_query_reports_an_error_and_recovers_on_the_next_edit() {
    let clock = Arc::new(FakeClock::default());
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let first = SnippetRecord::new("First", vec![";same".into()], "First body");
    let second = SnippetRecord::new("Second", vec![";same".into()], "Second body");
    let state = Arc::new(Mutex::new(Publisher::with_revision(
        Arc::new(KudzuLibrary {
            snippets: vec![first.clone(), second.clone()],
            ..Default::default()
        }),
        Revision(1),
    )));
    let panic_once = AtomicBool::new(true);
    let worker = ConflictWorker::start(state.clone(), scheduler, move |library| {
        assert!(
            !panic_once.swap(false, Ordering::SeqCst),
            "PRIVATE-CONFLICT-PAYLOAD"
        );
        find_conflicts(&library.snippets, &library.groups)
    })
    .unwrap();
    let results = worker.watch();
    let read = results.clone();
    let (tx, rx) = mpsc::channel();
    let _subscription = results.subscribe(move || {
        tx.send(read.get()).unwrap();
    });
    clock.advance(Duration::from_millis(250));
    let failed = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("query panic was not reported");
    assert_eq!(failed.error, Some(LibraryError::ConflictPanicked));
    assert!(!format!("{:?}", failed.error).contains("PRIVATE"));
    assert_eq!(failed.revision, Revision(0));
    assert!(
        rx.recv_timeout(Duration::from_millis(20)).is_err(),
        "retry must wait for another arm"
    );

    let committed = {
        let state = state.lock().unwrap_or_else(PoisonError::into_inner);
        let mut library = (*state.watch().get()).clone();
        library.snippets[0].name = "Edited after the failure".into();
        state.replace(Arc::new(library))
    };
    let revision = committed.revision;
    committed.notification.notify();
    clock.advance(Duration::from_millis(250));
    let recovered = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("conflict results froze after a panic");
    assert_eq!(recovered.revision, revision);
    assert!(recovered.error.is_none());
    assert_eq!(recovered.items.len(), 1);
    assert_eq!(recovered.items[0].snippet_id, first.id);
    assert_eq!(recovered.items[0].other_snippet_id, second.id);
}
