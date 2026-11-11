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

fn text(value: &str) -> Vec<PasteItem> {
    vec![vec![(uti::PLAIN_TEXT.into(), value.as_bytes().to_vec())]]
}

#[test]
fn captures_folds_and_preserves_override_and_undo() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-test-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    board.external_copy(text("launch"));
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        board.clone(),
        Arc::new(FakeThumbnailer::default()),
        Arc::new(FakeApps::default()),
        Arc::new(FakeApps::default()),
        clock.clone(),
        &writer,
    )
    .unwrap();
    service.poll().unwrap();
    assert!(service.watch().get().entries.is_empty());
    board.external_copy(text("hello"));
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    service.set_secret(id, Some(true)).unwrap();
    board.external_copy(text("hello"));
    service.poll().unwrap();
    assert_eq!(service.watch().get().entries[0].copy_count, 2);
    assert!(service.watch().get().entries[0].is_secret());
    let deleted = service.delete(&[id]).unwrap();
    service.restore(deleted).unwrap();
    assert_eq!(service.text(id).unwrap().as_deref(), Some("hello"));
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn private_marker_on_second_item_is_not_recorded() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-test-{}", Uuid::new_v4()));
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
        dir.clone(),
        settings.watch(),
        board.clone(),
        Arc::new(FakeThumbnailer::default()),
        Arc::new(FakeApps::default()),
        Arc::new(FakeApps::default()),
        clock.clone(),
        &writer,
    )
    .unwrap();
    let mut items = text("private");
    items.push(vec![(uti::PRIVATE_MARKERS[0].into(), vec![])]);
    board.external_copy(items);
    service.poll().unwrap();
    assert!(service.watch().get().entries.is_empty());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn store_persists_restores_and_collects_trash_and_orphans() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-store-{}", Uuid::new_v4()));
    let store = Store::open(dir.clone()).unwrap();
    let mut history = History::new();
    let items = text("alpha");
    let entry = history
        .ingest(
            &classify(&items, None).unwrap(),
            None,
            None,
            chrono::Utc::now(),
        )
        .entry()
        .clone();
    store
        .write(&store.blob(entry.id), &clipboard::encode_items(&items))
        .unwrap();
    assert_eq!(
        clipboard::text_of(&store.items(entry.id).unwrap()).as_deref(),
        Some("alpha")
    );
    let removed = history.remove(&[entry.id]);
    store.trash(&removed, false);
    assert!(!store.blob(entry.id).exists());
    store.trash(&removed, true);
    assert!(store.blob(entry.id).exists());
    store.trash(&removed, false);
    store.write(&store.blob(Uuid::new_v4()), b"orphan").unwrap();
    store.clean(&history);
    assert_eq!(std::fs::read_dir(dir.join("trash")).unwrap().count(), 0);
    assert_eq!(std::fs::read_dir(dir.join("items")).unwrap().count(), 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn corrupt_index_is_quarantined_and_directory_is_private() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-corrupt-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("history.json"), b"{invalid").unwrap();
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
        dir.clone(),
        settings.watch(),
        board,
        Arc::new(FakeThumbnailer::default()),
        Arc::new(FakeApps::default()),
        Arc::new(FakeApps::default()),
        clock,
        &writer,
    )
    .unwrap();
    assert!(service.watch().get().entries.is_empty());
    assert!(std::fs::read_dir(&dir).unwrap().flatten().any(|e| e
        .file_name()
        .to_string_lossy()
        .starts_with("history-corrupt-")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn plain_copy_own_changes_and_relaunch_keep_secret_override() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-copy-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let make = |writer: &Writer| {
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            board.clone(),
            Arc::new(FakeThumbnailer::default()),
            Arc::new(FakeApps::default()),
            Arc::new(FakeApps::default()),
            clock.clone(),
            writer,
        )
        .unwrap()
    };
    let service = make(&writer);
    let mut items = text("hello");
    items[0].push((uti::HTML.into(), b"<b>hello</b>".to_vec()));
    board.external_copy(items);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    service.set_secret(id, Some(true)).unwrap();
    service.copy(id, Flavor::Plain).unwrap();
    assert_eq!(board.snapshot.lock().unwrap().items, text("hello"));
    service.poll().unwrap();
    assert_eq!(service.watch().get().entries[0].copy_count, 1);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let writer2 = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let reopened = make(&writer2);
    assert_eq!(
        reopened.watch().get().entries[0].secret_override,
        Some(true)
    );
    reopened.shutdown().unwrap();
    writer2.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn history_alignment_compares_full_text_not_only_excerpt() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-align-{}", Uuid::new_v4()));
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
        dir.clone(),
        settings.watch(),
        board.clone(),
        Arc::new(FakeThumbnailer::default()),
        Arc::new(FakeApps::default()),
        Arc::new(FakeApps::default()),
        clock,
        &writer,
    )
    .unwrap();
    board.external_copy(text("older"));
    service.poll().unwrap();
    let newest = format!("{}a", "x".repeat(2048));
    board.external_copy(text(&newest));
    service.poll().unwrap();
    let unrecorded = format!("{}b", "x".repeat(2048));
    assert_eq!(
        service.history_text(1, Some(&unrecorded)).as_deref(),
        Some(newest.as_str())
    );
    assert_eq!(
        service.history_text(1, Some(&newest)).as_deref(),
        Some("older")
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn recording_exclusions_thumbnails_and_copy_fallback() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-capture-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    let thumb = Arc::new(FakeThumbnailer::default());
    *thumb.output.lock().unwrap() = Some(crate::ports::Thumbnail {
        png: vec![1],
        width: 900,
        height: 600,
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
            board.clone(),
            thumb.clone(),
            apps.clone(),
            apps.clone(),
            clock,
            &writer,
        )
        .unwrap(),
    );
    let mut off = (*settings.watch().get()).clone();
    off.clipboard_enabled = false;
    settings
        .replace(Arc::new(off.clone()))
        .notification
        .notify();
    board.external_copy(text("off"));
    service.poll().unwrap();
    off.clipboard_enabled = true;
    off.clipboard_ignored_apps = vec!["blocked".into()];
    settings.replace(Arc::new(off)).notification.notify();
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "blocked".into(),
        name: "Blocked".into(),
        pid: Some(4),
        path: None,
    });
    board.external_copy(text("excluded"));
    service.poll().unwrap();
    assert!(service.watch().get().entries.is_empty());
    *apps.frontmost.lock().unwrap() = None;
    board.external_copy(vec![vec![(uti::PNG.into(), vec![1, 2, 3])]]);
    service.poll().unwrap();
    assert_eq!(*thumb.requests.lock().unwrap(), vec![(3, 480)]);
    let id = service.watch().get().entries[0].id;
    assert_eq!(
        service.media(id, MediaKind::Thumbnail).unwrap().bytes,
        vec![1]
    );
    let picker = service.begin_pick().unwrap();
    assert_eq!(
        picker.paste(id, Flavor::Original).unwrap(),
        PasteOutcome::Copied
    );
    assert!(board.pasted_into.lock().unwrap().is_empty());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn large_blob_is_metadata_only_and_fails_to_copy() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-large-{}", Uuid::new_v4()));
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
        dir.clone(),
        settings.watch(),
        board.clone(),
        Arc::new(FakeThumbnailer::default()),
        Arc::new(FakeApps::default()),
        Arc::new(FakeApps::default()),
        clock,
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(
        "application/example".into(),
        vec![3; MAX_BLOB as usize + 1],
    )]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    assert!(!service.store.blob(id).exists());
    assert_eq!(
        service.copy(id, Flavor::Original),
        Err(ClipboardError::TooLarge)
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
