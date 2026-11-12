//! What the history screens (S58) need from the service: seeding a scene's history and keeping
//! a deleted batch around until its Undo succeeds.
use super::*;
use crate::{
    fixtures::{
        apps::FakeApps, clock::FakeClock, pasteboard::FakePasteboard, thumbnails::FakeThumbnailer,
    },
    persist::Writer,
    settings::{Settings, Variant},
    watch::Publisher,
};
use kudzu_core::clipboard::uti;
use std::{sync::Arc, time::Duration};

fn open(dir: &std::path::Path) -> (ClipboardService, Arc<FakePasteboard>, Writer) {
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.to_path_buf(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    (service, board, writer)
}

#[test]
fn a_scene_can_replace_the_history_and_watchers_see_it() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-seed-{}", Uuid::new_v4()));
    let (service, board, writer) = open(&dir);
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"first".to_vec())]]);
    service.poll().unwrap();
    let mut seeded = service.watch().get().as_ref().clone();
    seeded.entries.clear();
    let before = service.watch().revision();
    service.replace_history(seeded).unwrap();
    assert!(service.watch().get().entries.is_empty());
    assert_ne!(service.watch().revision(), before);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_deleted_batch_can_be_kept_for_a_second_undo_attempt() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-undo-{}", Uuid::new_v4()));
    let (service, board, writer) = open(&dir);
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"keep".to_vec())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let deleted = service.delete(&[id]).unwrap();
    let again = deleted.clone();
    assert_eq!(again.len(), 1);
    service.restore(deleted).unwrap();
    assert!(service.watch().get().get(id).is_some());
    // A batch whose ids are already back restores nothing and is not an error.
    service.restore(again).unwrap();
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_pick_session_names_its_target_and_can_give_focus_back() {
    use crate::fixtures::apps::FakeApps;
    use kudzu_core::system::AppInfo;
    let dir = std::env::temp_dir().join(format!("kudzu-clip-pick-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    let target = AppInfo {
        bundle_id: "com.test.Notes".into(),
        name: "Notes".into(),
        path: None,
        pid: Some(4242),
    };
    *apps.frontmost.lock().unwrap() = Some(target.clone());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = Arc::new(
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            ClipboardPorts {
                board: board.clone(),
                thumbnails: Arc::new(FakeThumbnailer::default()),
                target: apps.clone(),
                icons: apps.clone(),
                clock,
            },
            &writer,
        )
        .unwrap(),
    );
    let session = service.begin_pick().unwrap();
    assert_eq!(session.target().map(|a| a.name.as_str()), Some("Notes"));
    let task = session.restore_target().unwrap();
    let started = std::time::Instant::now();
    while task.take().is_none() {
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the restore never finished"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(apps.activations.lock().unwrap().as_slice(), &[target]);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn copy_then_paste_copied_is_the_same_as_paste_and_lets_the_caller_hide_in_between() {
    use crate::fixtures::apps::FakeApps;
    let dir = std::env::temp_dir().join(format!("kudzu-clip-pick2-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    *apps.frontmost.lock().unwrap() = Some(kudzu_core::system::AppInfo {
        bundle_id: "com.test.Notes".into(),
        name: "Notes".into(),
        path: None,
        pid: Some(7),
    });
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = Arc::new(
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            ClipboardPorts {
                board: board.clone(),
                thumbnails: Arc::new(FakeThumbnailer::default()),
                target: apps.clone(),
                icons: apps.clone(),
                clock,
            },
            &writer,
        )
        .unwrap(),
    );
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"hello".to_vec())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    *board.paste_allowed.lock().unwrap() = true;
    let session = service.begin_pick().unwrap();
    service.copy(id, Flavor::Original).unwrap();
    assert!(
        board.pasted_into.lock().unwrap().is_empty(),
        "copying does not paste"
    );
    assert_eq!(session.paste_copied().unwrap(), PasteOutcome::Pasted);
    assert_eq!(board.pasted_into.lock().unwrap().len(), 1);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
