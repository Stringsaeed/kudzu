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
