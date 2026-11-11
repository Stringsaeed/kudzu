use super::*;
use crate::{
    fixtures::{
        apps::FakeApps, clock::FakeClock, pasteboard::FakePasteboard, thumbnails::FakeThumbnailer,
    },
    persist::Writer,
    settings::{Settings, Variant},
    watch::Publisher,
};
use kudzu_core::clipboard::{
    query::{ClipFilter, ClipQuery},
    uti,
};
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
    store.trash(&removed, false).unwrap();
    assert!(!store.blob(entry.id).exists());
    store.trash(&removed, true).unwrap();
    assert!(store.blob(entry.id).exists());
    store.trash(&removed, false).unwrap();
    store.write(&store.blob(Uuid::new_v4()), b"orphan").unwrap();
    store.clean(&history).unwrap();
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

#[test]
fn deletion_keeps_blob_until_removal_is_durable() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-durable-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(60),
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
    board.external_copy(text("kept"));
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let deleted = service.delete(&[id]).unwrap();
    assert!(
        service.store.blob(id).exists(),
        "unsaved deletion must retain the blob"
    );
    service.restore(deleted).unwrap();
    assert_eq!(service.text(id).unwrap().as_deref(), Some("kept"));
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_search_redacts_secrets_and_cache_follows_revision() {
    use std::sync::atomic::Ordering;
    let dir = std::env::temp_dir().join(format!("kudzu-clip-agent-{}", Uuid::new_v4()));
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
    board.external_copy(text("hello password=superSecret123 world"));
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    assert!(service
        .agent_query(ClipFilter::All, &ClipQuery::parse("superSecret123"))
        .is_empty());
    let shown = service.agent_query(ClipFilter::All, &ClipQuery::parse("hello"));
    assert_eq!(shown.len(), 1);
    assert!(!shown[0]
        .excerpt
        .as_ref()
        .unwrap()
        .contains("superSecret123"));
    service.query(ClipFilter::All, &ClipQuery::parse("hello"));
    assert_eq!(service.cache_builds.load(Ordering::Relaxed), 1);
    service.set_secret(id, Some(false)).unwrap();
    assert_eq!(
        service
            .agent_query(ClipFilter::All, &ClipQuery::parse("superSecret123"))
            .len(),
        1
    );
    assert_eq!(service.cache_builds.load(Ordering::Relaxed), 2);
    service.set_secret(id, Some(true)).unwrap();
    assert!(service
        .agent_query(ClipFilter::All, &ClipQuery::parse("superSecret123"))
        .is_empty());
    assert!(service
        .agent_query(ClipFilter::All, &ClipQuery::parse(""))
        .pop()
        .unwrap()
        .excerpt
        .is_none());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

fn wait_saved(watch: &Watch<SaveState>, predicate: impl Fn(SaveState) -> bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    let _subscription = watch.subscribe(move || {
        let _ = tx.send(());
    });
    while !predicate(*watch.get()) {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("save did not settle");
    }
}

#[test]
#[cfg(unix)]
fn failed_and_interrupted_index_saves_preserve_recoverable_files() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("kudzu-clip-io-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_millis(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
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
    board.external_copy(text("retained"));
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let revision = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    wait_saved(&service.saved(), |s| s.durable >= revision);
    let old_index = std::fs::read(dir.join("history.json")).unwrap();
    let deleted = service.delete(&[id]).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let removal = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    wait_saved(&service.saved(), |s| s.error.is_some());
    assert_eq!(std::fs::read(dir.join("history.json")).unwrap(), old_index);
    assert!(service.store.blob(id).exists());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    clock.advance(Duration::from_millis(900));
    wait_saved(&service.saved(), |s| s.durable >= removal);
    service.collect().unwrap();
    assert!(!service.store.blob(id).exists());
    service.restore(deleted).unwrap();
    assert!(service.store.blob(id).exists());
    let restored = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    wait_saved(&service.saved(), |s| s.durable >= restored);
    service.delete(&[id]).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    assert!(writer.shutdown().is_err());
    assert!(service.store.blob(id).exists());
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let writer2 = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let reopened = make(&writer2);
    assert_eq!(reopened.text(id).unwrap().as_deref(), Some("retained"));
    reopened.shutdown().unwrap();
    writer2.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn index_writes_are_spaced_900ms_even_when_writer_debounce_is_zero() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-rate-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_millis(1)).unwrap();
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
    wait_saved(&service.saved(), |s| s.durable >= Revision(1));
    board.external_copy(text("one"));
    service.poll().unwrap();
    let first = service.watch().revision();
    clock.advance(Duration::from_millis(899));
    assert!(service.saved().get().durable < first);
    clock.advance(Duration::from_millis(1));
    wait_saved(&service.saved(), |s| s.durable >= first);
    board.external_copy(text("two"));
    service.poll().unwrap();
    let second = service.watch().revision();
    clock.advance(Duration::from_millis(899));
    assert!(service.saved().get().durable < second);
    clock.advance(Duration::from_millis(1));
    wait_saved(&service.saved(), |s| s.durable >= second);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn startup_rescans_old_secrets_and_prunes_and_clear_preserves_pins() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-start-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let clock = Arc::new(FakeClock::default());
    let mut h = History::new();
    let at: chrono::DateTime<chrono::Utc> = clock.now().into();
    let old = h
        .ingest(
            &classify(&text("old"), None).unwrap(),
            None,
            None,
            at - chrono::Duration::days(8),
        )
        .entry()
        .id;
    let secret = h
        .ingest(
            &classify(&text("password=verySecret123"), None).unwrap(),
            None,
            None,
            at,
        )
        .entry()
        .id;
    h.entries
        .iter_mut()
        .find(|e| e.id == secret)
        .unwrap()
        .secret = None;
    h.secret_rules = 0;
    std::fs::write(dir.join("history.json"), serde_json::to_vec(&h).unwrap()).unwrap();
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let board = Arc::new(FakePasteboard::default());
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
    assert!(service.watch().get().get(old).is_none());
    assert!(service.watch().get().get(secret).unwrap().secret.is_some());
    service.pin(secret, true).unwrap();
    board.external_copy(text("unpinned"));
    service.poll().unwrap();
    assert_eq!(service.clear(false).unwrap(), 1);
    assert!(service.watch().get().get(secret).is_some());
    assert_eq!(service.clear(true).unwrap(), 1);
    assert!(service.watch().get().entries.is_empty());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn runtime_prunes_on_ten_minute_timer_and_retention_change() {
    use crate::schedule::Scheduler;
    let dir = std::env::temp_dir().join(format!("kudzu-clip-prune-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let scheduler = Scheduler::new(clock.clone()).unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = Arc::new(
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            board.clone(),
            Arc::new(FakeThumbnailer::default()),
            Arc::new(FakeApps::default()),
            Arc::new(FakeApps::default()),
            clock.clone(),
            &writer,
        )
        .unwrap(),
    );
    service.start(&scheduler).unwrap();
    board.external_copy(text("first"));
    service.poll().unwrap();
    let watch = service.watch();
    let (tx, rx) = std::sync::mpsc::channel();
    let _subscription = watch.subscribe(move || {
        let _ = tx.send(());
    });
    clock.advance(Duration::from_secs(8 * 24 * 3600));
    while !watch.get().entries.is_empty() {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("scheduled prune");
    }
    board.external_copy(text("second"));
    service.poll().unwrap();
    clock.advance(Duration::from_secs(2 * 24 * 3600));
    let mut s = (*settings.watch().get()).clone();
    s.clipboard_retention_days = 1;
    settings.replace(Arc::new(s)).notification.notify();
    while !watch.get().entries.is_empty() {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("retention prune");
    }
    service.shutdown().unwrap();
    scheduler.shutdown();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn original_flavors_target_gone_and_settle_leave_selected_copy() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-paste-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    *board.paste_allowed.lock().unwrap() = true;
    let apps = Arc::new(FakeApps::default());
    let app = AppInfo {
        bundle_id: "example.target".into(),
        name: "Target".into(),
        pid: Some(42),
        path: None,
    };
    *apps.frontmost.lock().unwrap() = Some(app.clone());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = Arc::new(
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            board.clone(),
            Arc::new(FakeThumbnailer::default()),
            apps.clone(),
            apps.clone(),
            clock,
            &writer,
        )
        .unwrap(),
    );
    let mut items = text("formatted");
    items[0].push((uti::HTML.into(), b"<b>formatted</b>".to_vec()));
    board.external_copy(items.clone());
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let picker = service.begin_pick().unwrap();
    board.external_copy(text("other"));
    let start = Instant::now();
    assert_eq!(
        picker.paste(id, Flavor::Original).unwrap(),
        PasteOutcome::Pasted
    );
    assert!(start.elapsed() >= Duration::from_millis(40));
    assert_eq!(board.snapshot.lock().unwrap().items, items);
    assert_eq!(*board.pasted_into.lock().unwrap(), vec![Some(app)]);
    *apps.failure.lock().unwrap() = Some(PortError::TargetGone);
    assert_eq!(
        picker.paste(id, Flavor::Original),
        Err(ClipboardError::TargetGone)
    );
    assert_eq!(board.snapshot.lock().unwrap().items, items);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn image_file_threshold_and_media_limits() {
    struct Icons(Mutex<Vec<u32>>);
    impl AppCatalog for Icons {
        fn installed(&self) -> Result<Vec<AppInfo>, PortError> {
            Ok(vec![])
        }
        fn icon(&self, _: &str, size: u32) -> Result<Option<Vec<u8>>, PortError> {
            self.0.lock().unwrap().push(size);
            Ok(Some(vec![7]))
        }
    }
    let dir = std::env::temp_dir().join(format!("kudzu-clip-media-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("picture.png");
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_FILE_THUMB + 1).unwrap();
    let board = Arc::new(FakePasteboard::default());
    let thumb = Arc::new(FakeThumbnailer::default());
    let icons = Arc::new(Icons(Mutex::new(vec![])));
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.join("store"),
        settings.watch(),
        board.clone(),
        thumb.clone(),
        Arc::new(FakeApps::default()),
        icons.clone(),
        clock,
        &writer,
    )
    .unwrap();
    let file_url = format!("file://{}", path.display());
    board.external_copy(vec![vec![(uti::FILE_URL.into(), file_url.into_bytes())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    assert!(thumb.requests.lock().unwrap().is_empty());
    assert_eq!(
        service.media(id, MediaKind::Icon(0)).unwrap().bytes,
        vec![7]
    );
    assert_eq!(*icons.0.lock().unwrap(), vec![64]);
    file.set_len(MAX_IMAGE_FILE + 1).unwrap();
    assert!(service.media(id, MediaKind::File(0)).is_none());
    file.set_len(4).unwrap();
    assert_eq!(
        service.media(id, MediaKind::File(0)).unwrap().bytes.len(),
        4
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clipboard_records_while_expansion_is_paused() {
    use crate::expansion::testing::Harness;
    let harness = Harness::new(kudzu_core::library::model::KudzuLibrary::default());
    let expansion = harness.expansion();
    expansion.set_paused(true).unwrap();
    let dir = std::env::temp_dir().join(format!("kudzu-clip-paused-{}", Uuid::new_v4()));
    let service = ClipboardService::open(
        dir.clone(),
        harness.settings.watch(),
        harness.pasteboard.clone(),
        Arc::new(FakeThumbnailer::default()),
        harness.apps.clone(),
        harness.apps.clone(),
        harness.clock.clone(),
        &harness.writer,
    )
    .unwrap();
    harness
        .pasteboard
        .external_copy(text("recorded while paused"));
    service.poll().unwrap();
    assert_eq!(service.watch().get().entries.len(), 1);
    service.shutdown().unwrap();
    expansion.stop_feed();
    harness.writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn picker_waits_up_to_800ms_for_target_and_does_not_restore_clipboard() {
    use std::sync::atomic::{AtomicBool, Ordering};
    struct NeverActivates {
        app: AppInfo,
        activated: AtomicBool,
    }
    impl TargetApp for NeverActivates {
        fn frontmost(&self) -> Result<Option<AppInfo>, PortError> {
            Ok((!self.activated.load(Ordering::SeqCst)).then(|| self.app.clone()))
        }
        fn activate(&self, _: &AppInfo) -> Result<(), PortError> {
            self.activated.store(true, Ordering::SeqCst);
            Ok(())
        }
        fn selection(&self) -> Result<Option<String>, PortError> {
            Ok(None)
        }
        fn caret(&self) -> Result<Option<kudzu_core::system::Rect>, PortError> {
            Ok(None)
        }
    }
    let dir = std::env::temp_dir().join(format!("kudzu-clip-timeout-{}", Uuid::new_v4()));
    let target = Arc::new(NeverActivates {
        app: AppInfo {
            bundle_id: "example.target".into(),
            name: "Target".into(),
            pid: Some(2),
            path: None,
        },
        activated: AtomicBool::new(false),
    });
    let board = Arc::new(FakePasteboard::default());
    *board.paste_allowed.lock().unwrap() = true;
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = Arc::new(
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            board.clone(),
            Arc::new(FakeThumbnailer::default()),
            target,
            Arc::new(FakeApps::default()),
            clock,
            &writer,
        )
        .unwrap(),
    );
    board.external_copy(text("chosen"));
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let picker = service.begin_pick().unwrap();
    board.external_copy(text("old"));
    let start = Instant::now();
    assert_eq!(
        picker.paste(id, Flavor::Original),
        Err(ClipboardError::TargetGone)
    );
    assert!(start.elapsed() >= Duration::from_millis(800));
    assert_eq!(board.snapshot.lock().unwrap().items, text("chosen"));
    assert!(board.pasted_into.lock().unwrap().is_empty());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn failed_trash_move_is_reported_and_replayed_after_restart() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-move-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
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
    board.external_copy(text("recoverable"));
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let captured = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    wait_saved(&service.saved(), |s| s.durable >= captured);
    service.delete(&[id]).unwrap();
    let removed = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    wait_saved(&service.saved(), |s| s.durable >= removed);
    let obstacle = dir.join("trash").join(format!("{id}.kzc"));
    std::fs::create_dir(&obstacle).unwrap();
    assert!(service.collect().is_err());
    assert!(service.storage_error().get().is_some());
    assert!(service.store.blob(id).exists());
    assert!(dir.join("cleanup.json").exists());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    std::fs::remove_dir(obstacle).unwrap();
    let writer2 = Writer::new(
        clock.clone(),
        Duration::from_millis(900),
        Duration::from_secs(1),
    )
    .unwrap();
    let reopened = make(&writer2);
    assert!(reopened.watch().get().get(id).is_none());
    assert!(!reopened.store.blob(id).exists());
    reopened.shutdown().unwrap();
    writer2.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shutdown_respects_clipboard_write_interval() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-shutdown-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_secs(1)).unwrap();
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
    wait_saved(&service.saved(), |s| s.durable >= Revision(1));
    board.external_copy(text("last"));
    service.poll().unwrap();
    let revision = service.watch().revision();
    service.shutdown().unwrap();
    let start = Instant::now();
    writer.shutdown().unwrap();
    assert!(start.elapsed() >= Duration::from_millis(900));
    assert!(service.saved().get().durable >= revision);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn scheduled_pruning_waits_for_ten_minute_boundary() {
    use crate::schedule::Scheduler;
    let dir = std::env::temp_dir().join(format!("kudzu-clip-prune-boundary-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let clock = Arc::new(FakeClock::default());
    let at: chrono::DateTime<chrono::Utc> = clock.now().into();
    let mut history = History::new();
    history.ingest(
        &classify(&text("expires"), None).unwrap(),
        None,
        None,
        at - chrono::Duration::days(1) + chrono::Duration::seconds(1),
    );
    std::fs::write(
        dir.join("history.json"),
        serde_json::to_vec(&history).unwrap(),
    )
    .unwrap();
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let scheduler = Scheduler::new(clock.clone()).unwrap();
    let mut initial = Settings::for_variant(Variant::Dev);
    initial.clipboard_retention_days = 1;
    let settings = Publisher::new(initial);
    let service = Arc::new(
        ClipboardService::open(
            dir.clone(),
            settings.watch(),
            Arc::new(FakePasteboard::default()),
            Arc::new(FakeThumbnailer::default()),
            Arc::new(FakeApps::default()),
            Arc::new(FakeApps::default()),
            clock.clone(),
            &writer,
        )
        .unwrap(),
    );
    service.start(&scheduler).unwrap();
    let watch = service.watch();
    let (tx, rx) = std::sync::mpsc::channel();
    let _subscription = watch.subscribe(move || {
        let _ = tx.send(());
    });
    clock.advance(Duration::from_secs(599));
    assert_eq!(watch.get().entries.len(), 1);
    clock.advance(Duration::from_secs(1));
    while !watch.get().entries.is_empty() {
        rx.recv_timeout(Duration::from_secs(5))
            .expect("ten minute prune");
    }
    service.shutdown().unwrap();
    scheduler.shutdown();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
