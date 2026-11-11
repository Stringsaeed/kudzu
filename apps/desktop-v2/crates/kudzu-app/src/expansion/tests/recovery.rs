use crate::{expansion::testing::Harness, ports::TriggerLost};
use kudzu_core::library::model::{KudzuLibrary, SnippetRecord};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[test]
fn replaced_and_dismissed_notices_cannot_release_another_recovery_payload() {
    let h = Harness::new(KudzuLibrary::default());
    let service = h.expansion();
    let id = uuid::Uuid::from_u128(27);
    let _ = service.trigger_lost(TriggerLost {
        snippet_id: id,
        trigger: ";first".into(),
    });
    let first = service.lost_trigger_notice().get().as_ref().unwrap();
    let _ = service.trigger_lost(TriggerLost {
        snippet_id: id,
        trigger: ";second".into(),
    });
    let second = service.lost_trigger_notice().get().as_ref().unwrap();
    assert_ne!(first.id, second.id);
    assert!(service.take_trigger_for_retyping(first.id).is_none());
    assert!(!service.dismiss_lost_trigger(first.id));
    assert_eq!(*service.lost_trigger_notice().get(), Some(second));
    assert!(service.dismiss_lost_trigger(second.id));
    assert!(service.lost_trigger_notice().get().is_none());
    assert!(!service.dismiss_lost_trigger(second.id));
    assert!(service.take_trigger_for_retyping(second.id).is_none());
}

#[test]
fn recovery_notifies_after_unlock_and_preserves_the_original_trigger_until_one_ui_action() {
    let snippet = SnippetRecord::new("Fixture", vec![";original".into()], "body");
    let id = snippet.id;
    let h = Harness::new(KudzuLibrary {
        snippets: vec![snippet],
        ..Default::default()
    });
    let service = h.expansion();
    let (wake, woke) = mpsc::channel();
    let weak = Arc::downgrade(&service);
    let _subscription = service.lost_trigger_notice().subscribe(move || {
        let snapshot = weak.upgrade().unwrap().lost_trigger_notice().get();
        wake.send(*snapshot).unwrap();
    });
    let producer = service.clone();
    let worker = std::thread::spawn(move || {
        let _ = producer.trigger_lost(TriggerLost {
            snippet_id: id,
            trigger: ";original".into(),
        });
    });
    let notice = woke.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
    worker.join().unwrap();
    h.library.delete_snippets(&[id]).unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let service = service.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                service.take_trigger_for_retyping(notice.id)
            })
        })
        .collect();
    let recovered: Vec<_> = workers
        .into_iter()
        .filter_map(|w| w.join().unwrap())
        .collect();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].trigger(), ";original");
    assert_eq!(woke.recv_timeout(Duration::from_secs(2)).unwrap(), None);
    assert!(service.lost_trigger_notice().get().is_none());
}
