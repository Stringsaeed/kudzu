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
#[cfg(not(debug_assertions))]
use std::time::Instant;

#[test]
fn ignored_apps_and_own_writes_are_never_read() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
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
    .unwrap();
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "com.1password.1password".into(),
        name: "Password manager".into(),
        pid: None,
        path: None,
    });
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"private".to_vec())]]);
    service.poll().unwrap();
    assert_eq!(*board.reads.lock().unwrap(), 0);
    *apps.frontmost.lock().unwrap() = None;
    board
        .write(&[vec![(uti::PLAIN_TEXT.into(), b"own".to_vec())]])
        .unwrap();
    service.poll().unwrap();
    assert_eq!(*board.reads.lock().unwrap(), 0);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn corrupt_cleanup_file_does_not_block_startup() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("cleanup.json"), b"invalid").unwrap();
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: Arc::new(FakePasteboard::default()),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    assert!(!dir.join("cleanup.json").exists());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn history_text_does_not_wait_for_capture_io() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
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
            ClipboardPorts {
                board: board.clone(),
                thumbnails: Arc::new(FakeThumbnailer::default()),
                target: Arc::new(FakeApps::default()),
                icons: Arc::new(FakeApps::default()),
                clock,
            },
            &writer,
        )
        .unwrap(),
    );
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"older".to_vec())]]);
    service.poll().unwrap();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    *service.store.before_write.lock().unwrap() = Some(Arc::new(move || {
        entered_tx.send(()).unwrap();
        release_rx.lock().unwrap().recv().unwrap();
    }));
    board.external_copy(vec![vec![(
        "test.binary".into(),
        vec![1; 64 * 1024 * 1024],
    )]]);
    let capturing = service.clone();
    let worker = std::thread::spawn(move || capturing.poll());
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let reading = service.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        tx.send(reading.history_text(1, Some("not-current")))
            .unwrap()
    });
    let result = rx.recv_timeout(Duration::from_millis(500));
    release_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    reader.join().unwrap();
    assert_eq!(result.unwrap().as_deref(), Some("older"));
    *service.store.before_write.lock().unwrap() = None;
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agents_cannot_query_or_read_a_secret_beyond_the_excerpt() {
    use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    let text = format!("{} password=verySecret123", "ordinary ".repeat(300));
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), text.into_bytes())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    assert!(service
        .agent_query(ClipFilter::All, &ClipQuery::parse("ordinary"))
        .is_empty());
    let item = service.agent_item(id).unwrap();
    assert!(item.text.is_none());
    assert!(item.withheld.is_some());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn private_marker_on_later_item_is_checked_without_reading_data() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    board.external_copy(vec![
        vec![(uti::PLAIN_TEXT.into(), b"private".to_vec())],
        vec![(uti::PRIVATE_MARKERS[0].into(), vec![])],
    ]);
    service.poll().unwrap();
    assert_eq!(*board.reads.lock().unwrap(), 0);
    assert!(service.watch().get().entries.is_empty());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[cfg(unix)]
fn failed_capture_is_published_and_retried_without_leaving_blob() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    std::fs::set_permissions(dir.join("items"), std::fs::Permissions::from_mode(0o500)).unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"retry".to_vec())]]);
    assert!(service.poll().is_err());
    assert!(service.capture_error().get().is_some());
    assert_eq!(std::fs::read_dir(dir.join("items")).unwrap().count(), 0);
    std::fs::set_permissions(dir.join("items"), std::fs::Permissions::from_mode(0o700)).unwrap();
    service.poll().unwrap();
    assert!(service.capture_error().get().is_none());
    assert_eq!(service.watch().get().entries.len(), 1);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rescan_uses_full_blob_and_agent_item_withholds_secret_metadata() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let store = Store::open(dir.clone()).unwrap();
    let clock = Arc::new(FakeClock::default());
    let text = format!("{} password=verySecret123", "ordinary ".repeat(30_000));
    let items = vec![vec![(uti::PLAIN_TEXT.into(), text.into_bytes())]];
    let mut history = History::new();
    let id = history
        .ingest(
            &classify(&items, None).unwrap(),
            None,
            None,
            clock.now().into(),
        )
        .entry()
        .id;
    history.entries[0].secret = None;
    history.secret_rules = clipboard::SECRET_RULES;
    store
        .write(&store.blob(id), &clipboard::encode_items(&items))
        .unwrap();
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
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: Arc::new(FakePasteboard::default()),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    let item = service.agent_item(id).unwrap();
    assert!(item.withheld.is_some());
    assert!(item.text.is_none());
    assert!(item.length.is_none());
    assert!(item.lines.is_none());
    assert!(service.agent_history_text(1, Some("unrecorded")).is_none());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_item_returns_bounded_full_text_and_metadata() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "example.app".into(),
        name: "Example".into(),
        pid: None,
        path: None,
    });
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: apps.clone(),
            icons: apps,
            clock,
        },
        &writer,
    )
    .unwrap();
    let text = "plain ordinary words ".repeat(3000);
    board.external_copy(vec![vec![(
        uti::PLAIN_TEXT.into(),
        text.clone().into_bytes(),
    )]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let item = service.agent_item(id).unwrap();
    assert_eq!(
        item.text.as_ref().unwrap().chars().count(),
        search::MAX_ITEM_CHARS
    );
    assert!(item.truncated);
    assert_eq!(item.length, Some(text.chars().count()));
    assert_eq!(item.app.as_deref(), Some("Example"));
    assert_eq!(item.copy_count, 1);
    assert!(!item.pinned);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[cfg(not(debug_assertions))]
fn empty_search_5000_entries_p95_within_five_milliseconds() {
    use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
    let dir = std::env::temp_dir().join(format!("kudzu-review-perf-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let clock = Arc::new(FakeClock::default());
    let mut history = History::new();
    let items = vec![vec![(uti::PLAIN_TEXT.into(), vec![b'a'; 2048])]];
    history.ingest(
        &classify(&items, None).unwrap(),
        None,
        None,
        clock.now().into(),
    );
    let base = history.entries[0].clone();
    history.entries = (0..5000)
        .map(|_| {
            let mut e = base.clone();
            e.id = Uuid::new_v4();
            e
        })
        .collect();
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
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: Arc::new(FakePasteboard::default()),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    let query = ClipQuery::parse("");
    let mut timings = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let result = service.query(ClipFilter::All, &query);
        std::hint::black_box(&result);
        timings.push(start.elapsed());
        assert_eq!(result.indices.len(), 5000);
    }
    timings.sort();
    let p95 = timings[94];
    eprintln!("empty search p95: {p95:?}");
    assert!(p95 <= Duration::from_millis(5));
    let one_char = ClipQuery::parse("a");
    let mut cold = Vec::new();
    let mut cached = Vec::new();
    for _ in 0..30 {
        service.search.lock().unwrap().take();
        let start = Instant::now();
        let result = service.query(ClipFilter::All, &one_char);
        cold.push(start.elapsed());
        assert_eq!(std::hint::black_box(result.indices.len()), 5000);
        let start = Instant::now();
        let result = service.query(ClipFilter::All, &one_char);
        cached.push(start.elapsed());
        assert_eq!(std::hint::black_box(result.indices.len()), 5000);
    }
    cold.sort();
    cached.sort();
    let cold_p95 = cold[28];
    let cached_p95 = cached[28];
    eprintln!("one-character search cold p95: {cold_p95:?}; cached p95: {cached_p95:?}");
    assert!(
        cold_p95 <= Duration::from_millis(5),
        "cold one-character search exceeded the budget"
    );
    assert!(
        cached_p95 <= Duration::from_millis(5),
        "cached one-character search exceeded the budget"
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn recopy_of_unchanged_flavors_does_not_rewrite_blob() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    let writes = Arc::new(AtomicUsize::new(0));
    let counter = writes.clone();
    *service.store.before_write.lock().unwrap() = Some(Arc::new(move || {
        counter.fetch_add(1, Ordering::Relaxed);
    }));
    let item = vec![vec![(uti::PLAIN_TEXT.into(), b"same content".to_vec())]];
    board.external_copy(item.clone());
    service.poll().unwrap();
    board.external_copy(item);
    service.poll().unwrap();
    assert_eq!(writes.load(Ordering::Relaxed), 1);
    assert_eq!(service.watch().get().entries[0].copy_count, 2);
    *service.store.before_write.lock().unwrap() = None;
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clean_removes_wrong_extensions_and_root_temporary_files() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let store = Store::open(dir.clone()).unwrap();
    let mut history = History::new();
    let item = vec![vec![(uti::PLAIN_TEXT.into(), b"item".to_vec())]];
    let id = history
        .ingest(
            &classify(&item, None).unwrap(),
            None,
            None,
            chrono::Utc::now(),
        )
        .entry()
        .id;
    std::fs::write(store.blob(id), clipboard::encode_items(&item)).unwrap();
    let bad = dir.join("items").join(format!("{id}.bad"));
    let temp = dir.join(".kudzu-dead.tmp");
    std::fs::write(&bad, b"bad").unwrap();
    std::fs::write(&temp, b"temp").unwrap();
    store.clean(&history).unwrap();
    assert!(!bad.exists());
    assert!(!temp.exists());
    assert!(store.blob(id).exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn deleted_secret_has_no_cleanup_side_file_and_final_drain_collects() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    board.external_copy(vec![vec![(
        uti::PLAIN_TEXT.into(),
        b"password=secretPass123".to_vec(),
    )]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let deleted = service.delete(&[id]).unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted.entries()[0].id, id);
    assert!(!dir.join("cleanup.json").exists());
    assert!(service.store.blob(id).exists());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    assert!(!service.store.blob(id).exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn file_icons_are_looked_up_by_path_at_requested_size() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    let path = "/tmp/clip-icon-test.png";
    apps.icons.lock().unwrap().insert(path.into(), vec![7]);
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
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
    .unwrap();
    board.external_copy(vec![vec![(
        uti::FILE_URL.into(),
        format!("file://{path}").into_bytes(),
    )]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    assert_eq!(
        service.media(id, MediaKind::Icon(0)).unwrap().bytes,
        vec![7]
    );
    assert_eq!(*apps.icon_calls.lock().unwrap(), vec![(path.into(), 64)]);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn query_returns_stable_snapshot_indices_without_cloning_entries() {
    use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"first".to_vec())]]);
    service.poll().unwrap();
    let snapshot = service.watch().get();
    let results = service.query(ClipFilter::All, &ClipQuery::parse(""));
    assert!(Arc::ptr_eq(&snapshot, &results.history));
    assert_eq!(results.indices, vec![0]);
    let revision = results.revision;
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"second".to_vec())]]);
    service.poll().unwrap();
    assert_ne!(revision, service.watch().revision());
    assert_eq!(results.get(0).unwrap().text.as_deref(), Some("first"));
    assert!(results.get(1).is_none());
    assert_eq!(
        results.history.entries[results.indices[0]].text.as_deref(),
        Some("first")
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn copy_reports_missing_and_corrupt_saved_data() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"data".to_vec())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    std::fs::remove_file(service.store.blob(id)).unwrap();
    assert_eq!(
        service.copy(id, Flavor::Original),
        Err(ClipboardError::MissingData)
    );
    std::fs::write(service.store.blob(id), b"invalid").unwrap();
    assert_eq!(
        service.copy(id, Flavor::Original),
        Err(ClipboardError::CorruptData)
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn failed_collection_backs_off_and_final_shutdown_retries() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
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
        ClipboardPorts {
            board: board.clone(),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock: clock.clone(),
        },
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"delete me".to_vec())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let (tx, rx) = std::sync::mpsc::channel();
    let watch = service.saved();
    let _subscription = watch.subscribe(move || {
        let _ = tx.send(());
    });
    let captured = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    while watch.get().durable < captured {
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    service.delete(&[id]).unwrap();
    let removed = service.watch().revision();
    clock.advance(Duration::from_millis(900));
    while watch.get().durable < removed {
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    let obstacle = dir.join("trash").join(format!("{id}.kzc"));
    std::fs::create_dir(&obstacle).unwrap();
    assert!(service.collect().is_err());
    assert!(service.storage_error().get().is_some());
    std::fs::remove_dir(obstacle).unwrap();
    service.collect().unwrap();
    assert!(service.store.blob(id).exists());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    assert!(!service.store.blob(id).exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
#[cfg(unix)]
fn thumbnail_write_failure_removes_new_blob_and_retries() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let thumb = Arc::new(FakeThumbnailer::default());
    *thumb.output.lock().unwrap() = Some(crate::ports::Thumbnail {
        png: vec![1],
        width: 5,
        height: 5,
    });
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: thumb,
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    std::fs::set_permissions(dir.join("thumbs"), std::fs::Permissions::from_mode(0o500)).unwrap();
    board.external_copy(vec![vec![(uti::PNG.into(), vec![1, 2, 3])]]);
    assert!(service.poll().is_err());
    assert_eq!(std::fs::read_dir(dir.join("items")).unwrap().count(), 0);
    assert!(service.watch().get().entries.is_empty());
    std::fs::set_permissions(dir.join("thumbs"), std::fs::Permissions::from_mode(0o700)).unwrap();
    service.poll().unwrap();
    assert_eq!(service.watch().get().entries.len(), 1);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_list_includes_safe_file_image_and_color_metadata() {
    use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let thumb = Arc::new(FakeThumbnailer::default());
    *thumb.output.lock().unwrap() = Some(crate::ports::Thumbnail {
        png: vec![1],
        width: 640,
        height: 480,
    });
    let apps = Arc::new(FakeApps::default());
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "example.editor".into(),
        name: "Editor".into(),
        pid: None,
        path: None,
    });
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: thumb,
            target: apps.clone(),
            icons: apps,
            clock,
        },
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(
        uti::FILE_URL.into(),
        b"file:///tmp/test.png".to_vec(),
    )]]);
    service.poll().unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"#123456".to_vec())]]);
    service.poll().unwrap();
    board.external_copy(vec![vec![(uti::PNG.into(), vec![1, 2, 3])]]);
    service.poll().unwrap();
    let items = service.agent_query(ClipFilter::All, &ClipQuery::parse(""));
    assert_eq!(items.len(), 3);
    let file = items.iter().find(|i| i.kind == ClipKind::File).unwrap();
    assert_eq!(file.files, vec!["/tmp/test.png"]);
    assert_eq!(file.app.as_deref(), Some("Editor"));
    let color = items.iter().find(|i| i.kind == ClipKind::Color).unwrap();
    assert_eq!(color.color.as_deref(), Some("#123456"));
    let image = items.iter().find(|i| i.kind == ClipKind::Image).unwrap();
    assert_eq!(
        image.image,
        Some(ImageSize {
            width: 640,
            height: 480
        })
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn startup_restores_live_trashed_blob_before_rescanning_it() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let store = Store::open(dir.clone()).unwrap();
    let clock = Arc::new(FakeClock::default());
    let text = "ordinary words ".repeat(300);
    let items = vec![vec![(uti::PLAIN_TEXT.into(), text.into_bytes())]];
    let mut history = History::new();
    let entry = history
        .ingest(
            &classify(&items, None).unwrap(),
            None,
            None,
            clock.now().into(),
        )
        .entry()
        .clone();
    store
        .write(&store.blob(entry.id), &clipboard::encode_items(&items))
        .unwrap();
    store.trash(std::slice::from_ref(&entry), false).unwrap();
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
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: Arc::new(FakePasteboard::default()),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    assert!(service
        .watch()
        .get()
        .get(entry.id)
        .unwrap()
        .secret
        .is_none());
    assert!(service.agent_item(entry.id).unwrap().withheld.is_none());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn thumbnailer_errors_are_reported_and_same_copy_is_retried() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let thumb = Arc::new(FakeThumbnailer::default());
    *thumb.failure.lock().unwrap() = Some(PortError::Unavailable);
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: thumb.clone(),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(uti::PNG.into(), vec![1, 2, 3])]]);
    assert_eq!(
        service.poll(),
        Err(ClipboardError::Port(PortError::Unavailable))
    );
    assert!(service.capture_error().get().is_some());
    *thumb.failure.lock().unwrap() = None;
    service.poll().unwrap();
    assert_eq!(service.watch().get().entries.len(), 1);
    assert!(service.capture_error().get().is_none());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_metadata_and_search_do_not_reveal_secret_app_names() {
    use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "example.editor".into(),
        name: "password=secretPass123".into(),
        pid: None,
        path: None,
    });
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: apps.clone(),
            icons: apps,
            clock,
        },
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"normal".to_vec())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    assert!(service
        .agent_query(ClipFilter::All, &ClipQuery::parse("secretPass123"))
        .is_empty());
    assert!(service
        .agent_query(ClipFilter::All, &ClipQuery::parse("app:secretPass123"))
        .is_empty());
    let item = service.agent_item(id).unwrap();
    assert!(!item.app.unwrap().contains("secretPass123"));
    assert!(!item.redacted.is_empty());
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn stale_delayed_capture_does_not_replace_a_newer_change() {
    struct BlockingThumbnail {
        entered: std::sync::mpsc::Sender<()>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }
    impl Thumbnailer for BlockingThumbnail {
        fn thumbnail(
            &self,
            _: &[u8],
            _: u32,
        ) -> Result<Option<crate::ports::Thumbnail>, PortError> {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            Ok(None)
        }
    }
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let thumbnailer = Arc::new(BlockingThumbnail {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
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
            ClipboardPorts {
                board: board.clone(),
                thumbnails: thumbnailer,
                target: Arc::new(FakeApps::default()),
                icons: Arc::new(FakeApps::default()),
                clock,
            },
            &writer,
        )
        .unwrap(),
    );
    board.external_copy(vec![vec![(uti::PNG.into(), vec![1])]]);
    let earlier = service.clone();
    let worker = std::thread::spawn(move || earlier.poll());
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"newer".to_vec())]]);
    service.poll().unwrap();
    release_tx.send(()).unwrap();
    worker.join().unwrap().unwrap();
    let history = service.watch().get();
    assert_eq!(history.entries.len(), 1);
    assert_eq!(history.entries[0].text.as_deref(), Some("newer"));
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn usage_cache_refreshes_after_deferred_cleanup_without_a_new_revision() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-{}", Uuid::new_v4()));
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
        ClipboardPorts {
            board: board.clone(),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock: clock.clone(),
        },
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"temporary".to_vec())]]);
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let before = service.usage();
    assert!(before > 0);
    service.clear(true).unwrap();
    assert_eq!(service.usage(), before);
    let revision = service.watch().revision();
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    assert!(service.saved().get().durable >= revision);
    service.finish_shutdown().unwrap();
    assert_eq!(service.watch().revision(), revision);
    assert_eq!(service.usage(), 0);
    assert!(!service.store.blob(id).exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn changed_ignored_app_during_read_is_not_recorded_or_misattributed() {
    let dir = std::env::temp_dir().join(format!("kudzu-review-race-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
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
    .unwrap();
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "example.editor".into(),
        name: "Editor".into(),
        pid: Some(1),
        path: None,
    });
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"normal".to_vec())]]);
    let weak = Arc::downgrade(&board);
    let changed_apps = apps.clone();
    *board.before_read.lock().unwrap() = Some(Arc::new(move || {
        *changed_apps.frontmost.lock().unwrap() = Some(AppInfo {
            bundle_id: "com.1password.1password".into(),
            name: "Passwords".into(),
            pid: Some(2),
            path: None,
        });
        weak.upgrade()
            .unwrap()
            .external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"secret".to_vec())]]);
    }));
    service.poll().unwrap();
    assert!(service.watch().get().entries.is_empty());
    *board.before_read.lock().unwrap() = None;
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_query_returns_at_most_one_hundred_matches() {
    use kudzu_core::clipboard::query::{ClipFilter, ClipQuery};
    let dir = std::env::temp_dir().join(format!("kudzu-agent-cap-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let clock = Arc::new(FakeClock::default());
    let mut history = History::new();
    let items = vec![vec![(uti::PLAIN_TEXT.into(), b"ordinary".to_vec())]];
    history.ingest(
        &classify(&items, None).unwrap(),
        None,
        None,
        clock.now().into(),
    );
    let entry = history.entries[0].clone();
    history.entries = (0..101)
        .map(|_| {
            let mut e = entry.clone();
            e.id = Uuid::new_v4();
            e
        })
        .collect();
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
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: Arc::new(FakePasteboard::default()),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    assert_eq!(
        service
            .agent_query(ClipFilter::All, &ClipQuery::parse(""))
            .len(),
        100
    );
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn agent_history_text_is_capped_at_fifty_thousand_scalars() {
    let dir = std::env::temp_dir().join(format!("kudzu-agent-history-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
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
    let text = "plain words ".repeat(5000);
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), text.into_bytes())]]);
    service.poll().unwrap();
    let text = service.agent_history_text(1, Some("unrecorded")).unwrap();
    assert_eq!(text.chars().count(), 50_000);
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn repeated_image_neither_decodes_blob_nor_regenerates_thumbnail() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let dir = std::env::temp_dir().join(format!("kudzu-clip-recopy-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let thumbnails = Arc::new(FakeThumbnailer::default());
    *thumbnails.output.lock().unwrap() = Some(crate::ports::Thumbnail {
        png: vec![7],
        width: 5,
        height: 5,
    });
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Publisher::new(Settings::for_variant(Variant::Dev));
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: thumbnails.clone(),
            target: Arc::new(FakeApps::default()),
            icons: Arc::new(FakeApps::default()),
            clock,
        },
        &writer,
    )
    .unwrap();
    let items = vec![vec![(uti::PNG.into(), vec![1, 2, 3])]];
    board.external_copy(items.clone());
    service.poll().unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    let reads_hook = reads.clone();
    *service.store.before_items.lock().unwrap() = Some(Arc::new(move || {
        reads_hook.fetch_add(1, Ordering::Relaxed);
    }));
    board.external_copy(items);
    service.poll().unwrap();
    assert_eq!(
        (
            reads.load(Ordering::Relaxed),
            thumbnails.requests.lock().unwrap().len()
        ),
        (0, 1)
    );
    *service.store.before_items.lock().unwrap() = None;
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn copy_succeeds_when_item_is_deleted_after_pasteboard_write() {
    let dir = std::env::temp_dir().join(format!("kudzu-clip-copy-race-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
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
            ClipboardPorts {
                board: board.clone(),
                thumbnails: Arc::new(FakeThumbnailer::default()),
                target: Arc::new(FakeApps::default()),
                icons: Arc::new(FakeApps::default()),
                clock,
            },
            &writer,
        )
        .unwrap(),
    );
    let items = vec![vec![(uti::PLAIN_TEXT.into(), b"copied".to_vec())]];
    board.external_copy(items.clone());
    service.poll().unwrap();
    let id = service.watch().get().entries[0].id;
    let weak = Arc::downgrade(&service);
    *board.after_write.lock().unwrap() = Some(Arc::new(move || {
        weak.upgrade().unwrap().delete(&[id]).unwrap();
    }));
    let result = service.copy(id, Flavor::Original);
    assert_eq!(result, Ok(()));
    assert_eq!(board.snapshot.lock().unwrap().items, items);
    *board.after_write.lock().unwrap() = None;
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clipboard_root_stays_within_the_four_hundred_line_target() {
    assert!(include_str!("mod.rs").lines().count() <= 400);
}

#[test]
fn app_excluded_during_read_is_not_captured() {
    let dir = std::env::temp_dir().join(format!("kudzu-ignore-race-{}", Uuid::new_v4()));
    let board = Arc::new(FakePasteboard::default());
    let apps = Arc::new(FakeApps::default());
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(1),
        Duration::from_secs(1),
    )
    .unwrap();
    let settings = Arc::new(Publisher::new(Settings::for_variant(Variant::Dev)));
    *apps.frontmost.lock().unwrap() = Some(AppInfo {
        bundle_id: "example.editor".into(),
        name: "Editor".into(),
        pid: Some(1),
        path: None,
    });
    let service = ClipboardService::open(
        dir.clone(),
        settings.watch(),
        ClipboardPorts {
            board: board.clone(),
            thumbnails: Arc::new(FakeThumbnailer::default()),
            target: apps.clone(),
            icons: apps,
            clock,
        },
        &writer,
    )
    .unwrap();
    board.external_copy(vec![vec![(uti::PLAIN_TEXT.into(), b"ordinary".to_vec())]]);
    let new_settings = settings.clone();
    *board.before_read.lock().unwrap() = Some(Arc::new(move || {
        let mut next = (*new_settings.watch().get()).clone();
        next.clipboard_ignored_apps.push("example.editor".into());
        new_settings.replace(Arc::new(next)).notification.notify();
    }));
    service.poll().unwrap();
    assert!(service.watch().get().entries.is_empty());
    *board.before_read.lock().unwrap() = None;
    service.shutdown().unwrap();
    writer.shutdown().unwrap();
    service.finish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
