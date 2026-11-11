use super::*;
use crate::error::PortError;
use crate::fixtures::clock::FakeClock;
use crate::fixtures::cloudkit::{CloudScript, FakeCloud};
use crate::fixtures::ids::FixedIds;
use crate::library::{LibraryService, SnippetDraft};
use crate::persist::{PersistError, Revision, WriteTarget, Writer};
use crate::ports::{AccountChange, CloudAvailability, CloudBatch, CloudSend, CloudSendFailure};
use crate::settings::{Settings, SettingsService, Theme, Variant};
use chrono::{TimeZone, Utc};
use kudzu_core::library::model::KudzuLibrary;
use kudzu_core::library::starter::ensure_getting_started_snippets;
use kudzu_core::sync::{RecordKind, RemoteRecord, SyncIndex, META_RECORD};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct Memory;
impl<T: Send + Sync> WriteTarget<T> for Memory {
    fn write(&self, _: &T) -> Result<(), PersistError> {
        Ok(())
    }
}

struct TempDir(PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Harness {
    sync: SyncService,
    library: Arc<LibraryService>,
    settings: SettingsService,
    cloud: Arc<FakeCloud>,
    clock: Arc<FakeClock>,
    scheduler: Arc<Scheduler>,
    /// Kept alive so library and settings registrations outlive the sync worker.
    _writer: Writer,
    dir: TempDir,
}

impl Harness {
    fn open(production: bool, enabled: bool) -> Self {
        let dir = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
        fs::create_dir_all(&dir.0).unwrap();
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_millis(1)).unwrap();
        let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
        let library = Arc::new(
            LibraryService::new(
                KudzuLibrary::default(),
                "device-a".into(),
                clock.clone(),
                Arc::new(FixedIds::new(100)),
                writer.register(Memory, Revision(0)).unwrap(),
                scheduler.clone(),
            )
            .unwrap(),
        );
        let settings = SettingsService::new(
            Settings {
                icloud_sync: enabled,
                ..Settings::for_variant(Variant::Production)
            },
            writer.register(Memory, Revision(0)).unwrap(),
        )
        .unwrap();
        let cloud = Arc::new(FakeCloud::default());
        let sync = SyncService::open(
            SyncConfig {
                production,
                device_id: "device-a".into(),
                directory: dir.0.clone(),
            },
            library.clone(),
            settings.watch(),
            cloud.clone(),
            clock.clone(),
            scheduler.clone(),
        )
        .unwrap();
        Self {
            sync,
            library,
            settings,
            cloud,
            clock,
            scheduler,
            _writer: writer,
            dir,
        }
    }

    fn settle(&self) {
        self.sync.settle();
    }

    fn generation(&self) -> u64 {
        self.cloud
            .state
            .lock()
            .unwrap()
            .active
            .expect("sync is running")
    }

    fn deliver(&self, generation: u64, batch: CloudBatch) {
        self.cloud.deliver(generation, batch);
        self.settle();
    }

    fn ack(&self) {
        let generation = self.generation();
        let pending = {
            let mut state = self.cloud.state.lock().unwrap();
            std::mem::take(&mut state.sends)
        };
        let events = self.cloud.events(generation).expect("callback");
        for (_, batch) in pending {
            events.sent(
                generation,
                CloudSend {
                    saved: batch.records,
                    deleted: batch.deleted,
                    ..CloudSend::default()
                },
            );
        }
        self.settle();
    }

    fn full_fetch(&self) {
        let generation = self.generation();
        self.deliver(
            generation,
            CloudBatch {
                full_fetch: true,
                ..CloudBatch::default()
            },
        );
    }

    fn uploaded(&self) -> Vec<String> {
        self.cloud
            .state
            .lock()
            .unwrap()
            .sends
            .iter()
            .flat_map(|(_, batch)| {
                batch
                    .records
                    .iter()
                    .map(|record| record.name.clone())
                    .chain(batch.deleted.clone())
            })
            .collect()
    }
}

fn snippet(harness: &Harness, trigger: &str, content: &str) -> Uuid {
    harness
        .library
        .create_snippet(SnippetDraft {
            triggers: vec![trigger.into()],
            content: content.into(),
            ..SnippetDraft::default()
        })
        .unwrap()
        .0
        .id
}

fn remote_snippet(id: Uuid, trigger: &str, content: &str, updated: i64) -> RemoteRecord {
    let mut record =
        kudzu_core::library::model::SnippetRecord::new("", vec![trigger.into()], content);
    record.id = id;
    record.updated_at = Utc.timestamp_opt(updated, 0).unwrap();
    record.created_at = record.updated_at;
    RemoteRecord {
        kind: RecordKind::Snippet,
        name: kudzu_core::sync::snippet_name(id),
        payload: serde_json::to_string(&record).unwrap(),
    }
}

fn wait_fetch(harness: &Harness) {
    let before = harness.cloud.state.lock().unwrap().fetches.len();
    let start = Instant::now();
    loop {
        harness.settle();
        if harness.cloud.state.lock().unwrap().fetches.len() > before {
            return;
        }
        if start.elapsed() > Duration::from_secs(3) {
            panic!("fetch tick did not run");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn kind(status: &SyncStatus) -> &'static str {
    match status {
        SyncStatus::Off => "off",
        SyncStatus::Unavailable { .. } => "unavailable",
        SyncStatus::NoAccount { .. } => "no-account",
        SyncStatus::Starting => "starting",
        SyncStatus::Syncing => "syncing",
        SyncStatus::UpToDate { .. } => "up-to-date",
        SyncStatus::NeedsUpdate => "needs-update",
        SyncStatus::Error { .. } => "error",
    }
}

#[test]
fn dev_gate_matches_the_shipped_wording() {
    assert_eq!(
        dev_gate(false, true),
        Some(SyncStatus::Unavailable {
            reason: DEV_UNAVAILABLE.into(),
        })
    );
    assert_eq!(dev_gate(false, false), Some(SyncStatus::Off));
    assert_eq!(dev_gate(true, true), None);
    assert_eq!(dev_gate(true, false), None);
}

#[test]
fn dev_and_unentitled_builds_stay_unavailable() {
    let dev = Harness::open(false, true);
    dev.settle();
    assert_eq!(
        *dev.sync.status().get(),
        SyncStatus::Unavailable {
            reason: DEV_UNAVAILABLE.into(),
        }
    );
    assert!(dev.cloud.state.lock().unwrap().starts.is_empty());
    let dev_cloud = dev.cloud.clone();
    drop(dev);
    assert_eq!(dev_cloud.state.lock().unwrap().stops, 0);

    let off = Harness::open(false, false);
    off.settle();
    assert_eq!(*off.sync.status().get(), SyncStatus::Off);
    assert!(off.cloud.state.lock().unwrap().starts.is_empty());
    let off_cloud = off.cloud.clone();
    drop(off);
    assert_eq!(off_cloud.state.lock().unwrap().stops, 0);

    let cloud = Arc::new(FakeCloud::default());
    cloud.fail_start(Some(PortError::Unavailable));
    let dir = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
    fs::create_dir_all(&dir.0).unwrap();
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(30),
        Duration::from_secs(1),
    )
    .unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let library = Arc::new(
        LibraryService::new(
            KudzuLibrary::default(),
            "device-a".into(),
            clock.clone(),
            Arc::new(FixedIds::new(1)),
            writer.register(Memory, Revision(0)).unwrap(),
            scheduler.clone(),
        )
        .unwrap(),
    );
    let settings = SettingsService::new(
        Settings::for_variant(Variant::Production),
        writer.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: dir.0.clone(),
        },
        library,
        settings.watch(),
        cloud.clone(),
        clock,
        scheduler,
    )
    .unwrap();
    sync.settle();
    assert_eq!(
        *sync.status().get(),
        SyncStatus::Unavailable {
            reason: UNENTITLED.into(),
        }
    );
    assert!(cloud.state.lock().unwrap().starts.is_empty());
    drop(sync);
    assert_eq!(cloud.state.lock().unwrap().stops, 0);
    writer.shutdown().unwrap();
}

#[test]
fn every_status_is_reachable_and_errors_use_fixed_sentences() {
    let harness = Harness::open(true, false);
    harness.settle();
    assert_eq!(*harness.sync.status().get(), SyncStatus::Off);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let status = harness.sync.status();
    let _subscription = status.subscribe({
        let seen = seen.clone();
        let status = status.clone();
        move || seen.lock().unwrap().push((*status.get()).clone())
    });
    harness.sync.set_enabled(true);
    harness.settle();
    let generation = harness.generation();
    let events = harness.cloud.events(generation).unwrap();
    events.availability(generation, crate::ports::CloudAvailability::NoAccount);
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: NO_ACCOUNT.into(),
        }
    );
    events.availability(generation, crate::ports::CloudAvailability::Restricted);
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: RESTRICTED.into(),
        }
    );
    events.availability(
        generation,
        crate::ports::CloudAvailability::TemporarilyUnavailable,
    );
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: TEMPORARILY_UNAVAILABLE.into(),
        }
    );
    events.availability(generation, CloudAvailability::Unavailable);
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::Unavailable {
            reason: UNENTITLED.into(),
        }
    );
    events.availability(generation, crate::ports::CloudAvailability::Available);
    harness.settle();
    harness.full_fetch();
    harness.ack();
    assert!(matches!(
        *harness.sync.status().get(),
        SyncStatus::UpToDate { .. }
    ));
    if let SyncStatus::UpToDate { last_sync } = *harness.sync.status().get() {
        assert_eq!(last_sync, harness.clock.now());
    }
    events.sent(
        generation,
        CloudSend {
            failure: Some(CloudSendFailure::QuotaExceeded),
            ..CloudSend::default()
        },
    );
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::Error {
            message: QUOTA.into(),
        }
    );
    events.sent(
        generation,
        CloudSend {
            failure: Some(CloudSendFailure::Permanent),
            ..CloudSend::default()
        },
    );
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::Error {
            message: PERMANENT_FAILURE.into(),
        }
    );
    events.failed(generation, PortError::TimedOut);
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::Error {
            message: TEMPORARILY_UNAVAILABLE.into(),
        }
    );
    events.fetched(
        generation,
        CloudBatch {
            records: vec![RemoteRecord {
                kind: RecordKind::Meta,
                name: META_RECORD.into(),
                payload: r#"{"syncVersion":2}"#.into(),
            }],
            ..CloudBatch::default()
        },
    );
    harness.settle();
    assert_eq!(*harness.sync.status().get(), SyncStatus::NeedsUpdate);

    let mut kinds = BTreeSet::from(["off"]);
    for status in seen.lock().unwrap().iter() {
        kinds.insert(kind(status));
    }
    assert_eq!(
        kinds,
        BTreeSet::from([
            "off",
            "unavailable",
            "starting",
            "syncing",
            "no-account",
            "up-to-date",
            "error",
            "needs-update",
        ])
    );
}

#[test]
fn edits_queue_without_usage_and_usage_waits_for_the_twelfth_minute() {
    let harness = Harness::open(true, true);
    harness.settle();
    let id = snippet(&harness, ";a", "local");
    harness.settle();
    harness.full_fetch();
    harness.ack();
    let before = harness.uploaded().len();
    harness
        .library
        .update_snippet({
            let mut record = harness
                .library
                .watch()
                .get()
                .snippets
                .iter()
                .find(|snippet| snippet.id == id)
                .unwrap()
                .clone();
            record.content = "edited".into();
            record
        })
        .unwrap();
    harness.library.record_expansion(id, 4).unwrap();
    harness.settle();
    let names = harness.uploaded();
    let queued = &names[before..];
    assert!(queued.iter().any(|name| name.starts_with("snippet-")));
    assert!(queued.iter().all(|name| !name.starts_with("usage-")));

    harness.ack();
    let usage = kudzu_core::sync::usage_name("device-a");
    for tick in 1..=4 {
        let start = harness.cloud.state.lock().unwrap().sends.len();
        harness.clock.advance(FETCH_INTERVAL);
        wait_fetch(&harness);
        let sends = harness.cloud.state.lock().unwrap().sends.clone();
        let fresh = &sends[start..];
        let has_usage = fresh.iter().any(|(_, batch)| {
            batch.records.iter().any(|record| record.name == usage)
                || batch.deleted.iter().any(|name| name == &usage)
        });
        assert_eq!(has_usage, tick == 4, "tick {tick}");
    }
}

#[test]
fn a_merge_keeps_a_local_edit_and_does_not_echo_the_server_copy() {
    let harness = Harness::open(true, true);
    harness.settle();
    let id = snippet(&harness, ";a", "local");
    harness.settle();
    harness.full_fetch();
    harness.ack();
    let generation = harness.generation();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let library = harness.library.clone();
    let editor = thread::spawn(move || {
        library
            .transact(move |library| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                let snippet = library
                    .snippets
                    .iter_mut()
                    .find(|snippet| snippet.id == id)
                    .unwrap();
                snippet.content = "during".into();
                snippet.updated_at = Utc.timestamp_opt(1_900_000_000, 0).unwrap();
                Ok(())
            })
            .unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let marker = harness.cloud.state.lock().unwrap().sends.len();
    let server_b = Uuid::from_u128(50);
    harness.cloud.deliver(
        generation,
        CloudBatch {
            records: vec![
                remote_snippet(id, ";a", "server", 1_700_000_000),
                remote_snippet(server_b, ";b", "remote", 1_700_000_000),
            ],
            ..CloudBatch::default()
        },
    );
    thread::sleep(Duration::from_millis(50));
    release_tx.send(()).unwrap();
    editor.join().unwrap();
    harness.settle();

    let library = harness.library.watch().get();
    assert_eq!(
        library
            .snippets
            .iter()
            .find(|snippet| snippet.id == id)
            .unwrap()
            .content,
        "during"
    );
    assert!(library
        .snippets
        .iter()
        .any(|snippet| snippet.id == server_b));
    let sent = harness.cloud.state.lock().unwrap().sends.clone();
    let names: Vec<String> = sent
        .iter()
        .skip(marker)
        .flat_map(|(_, batch)| batch.records.iter().map(|record| record.name.clone()))
        .collect();
    assert!(names
        .iter()
        .all(|name| name != &kudzu_core::sync::snippet_name(server_b)));
    let echoed = sent.iter().skip(marker).any(|(_, batch)| {
        batch.records.iter().any(|record| {
            record.name == kudzu_core::sync::snippet_name(id) && !record.payload.contains("during")
        })
    });
    assert!(!echoed);
    assert!(sent.iter().skip(marker).any(|(_, batch)| {
        batch
            .records
            .iter()
            .any(|record| record.payload.contains("during"))
    }));
    let index: SyncIndex =
        serde_json::from_slice(&fs::read(harness.dir.0.join("index.json")).unwrap()).unwrap();
    assert!(index
        .records
        .contains_key(&kudzu_core::sync::snippet_name(server_b)));
    assert!(index.initial_merge_done);
}

#[test]
fn stale_generations_after_stop_restart_and_account_change_are_ignored() {
    let harness = Harness::open(true, true);
    harness.settle();
    let first = harness.generation();
    harness.sync.set_enabled(false);
    harness.settle();
    harness.deliver(
        first,
        CloudBatch {
            records: vec![remote_snippet(Uuid::from_u128(7), ";old", "nope", 10)],
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    assert!(harness.library.watch().get().snippets.is_empty());
    let fetches = harness.cloud.state.lock().unwrap().fetches.clone();
    harness.clock.advance(FETCH_INTERVAL);
    thread::sleep(Duration::from_millis(30));
    harness.settle();
    assert_eq!(harness.cloud.state.lock().unwrap().fetches, fetches);

    harness.sync.set_enabled(true);
    harness.settle();
    let second = harness.generation();
    assert_ne!(first, second);
    harness.clock.advance(FETCH_INTERVAL);
    wait_fetch(&harness);
    assert!(harness
        .cloud
        .state
        .lock()
        .unwrap()
        .fetches
        .iter()
        .skip(fetches.len())
        .all(|generation| *generation == second));

    snippet(&harness, ";keep", "library");
    harness.settle();
    harness.full_fetch();
    let events = harness.cloud.events(second).unwrap();
    events.state(second, b"token".to_vec());
    harness.settle();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"token"
    );
    let before = harness.library.watch().get().snippets.len();
    events.account_changed(second, AccountChange::Switch);
    harness.settle();
    assert_eq!(harness.library.watch().get().snippets.len(), before);
    let index: SyncIndex =
        serde_json::from_slice(&fs::read(harness.dir.0.join("index.json")).unwrap()).unwrap();
    assert!(!index.initial_merge_done);
    assert!(index.records.is_empty());
    assert!(!harness.dir.0.join("engine-state.data").exists());
    let third = harness.generation();
    let marker = harness.uploaded().len();
    harness.deliver(
        second,
        CloudBatch {
            records: vec![remote_snippet(Uuid::from_u128(8), ";stale", "nope", 10)],
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    assert_eq!(harness.library.watch().get().snippets.len(), before);
    assert!(harness.uploaded()[marker..]
        .iter()
        .all(|name| !name.starts_with("snippet-")));
    harness.deliver(
        third,
        CloudBatch {
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    assert!(harness.uploaded()[marker..]
        .iter()
        .any(|name| name.starts_with("snippet-")));
}

#[test]
fn a_failed_checkpoint_is_reused_without_losing_the_library() {
    let harness = Harness::open(true, true);
    harness.settle();
    let generation = harness.generation();
    harness
        .cloud
        .events(generation)
        .unwrap()
        .state(generation, b"token-1".to_vec());
    harness.settle();
    harness.full_fetch();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"token-1"
    );
    let id = Uuid::from_u128(9);
    harness.deliver(
        generation,
        CloudBatch {
            records: vec![remote_snippet(id, ";c", "kept", 20)],
            ..CloudBatch::default()
        },
    );
    // A fetch with no following state event must not move the token.
    harness
        .cloud
        .events(generation)
        .unwrap()
        .state(generation, Vec::new());
    harness.settle();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"token-1"
    );
    assert!(harness
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.id == id));
    let index: SyncIndex =
        serde_json::from_slice(&fs::read(harness.dir.0.join("index.json")).unwrap()).unwrap();
    assert!(index
        .records
        .contains_key(&kudzu_core::sync::snippet_name(id)));

    let directory = harness.dir.0.clone();
    let library = harness.library.clone();
    let cloud = harness.cloud.clone();
    let clock = harness.clock.clone();
    let scheduler = harness.scheduler.clone();
    let settings = harness.settings.watch();
    drop(harness.sync);
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory,
        },
        library,
        settings,
        cloud.clone(),
        clock,
        scheduler,
    )
    .unwrap();
    sync.settle();
    let starts = cloud.state.lock().unwrap().starts.clone();
    assert_eq!(starts.last().unwrap().1, Some(b"token-1".to_vec()));
    drop(sync);
}

#[test]
fn starters_wait_for_the_first_full_fetch_and_dedupe_runs_once() {
    let harness = Harness::open(true, true);
    harness
        .library
        .transact(|library| {
            ensure_getting_started_snippets(library, Utc::now());
            Ok(())
        })
        .unwrap();
    harness.settle();
    assert!(harness
        .uploaded()
        .iter()
        .all(|name| !name.starts_with("snippet-") && !name.starts_with("group-")));
    harness.full_fetch();
    assert!(harness
        .uploaded()
        .iter()
        .any(|name| name.starts_with("snippet-")));
    assert!(harness
        .uploaded()
        .iter()
        .any(|name| name.starts_with("group-")));

    let harness = Harness::open(true, true);
    harness.settle();
    let (first, _) = harness
        .library
        .create_snippet(SnippetDraft {
            triggers: vec![";kudzu".into()],
            content: "Kudzu is working!".into(),
            ..SnippetDraft::default()
        })
        .unwrap();
    harness
        .library
        .transact({
            let first = first.clone();
            move |library| {
                let mut copy = first;
                copy.id = Uuid::from_u128(400);
                library.snippets.push(copy);
                Ok(())
            }
        })
        .unwrap();
    harness.settle();
    harness.full_fetch();
    assert_eq!(harness.library.watch().get().snippets.len(), 1);
    harness
        .library
        .transact(|library| {
            let mut copy = library.snippets[0].clone();
            copy.id = Uuid::from_u128(401);
            library.snippets.push(copy);
            Ok(())
        })
        .unwrap();
    harness.full_fetch();
    assert_eq!(harness.library.watch().get().snippets.len(), 2);
}

#[test]
fn needs_update_stops_sends_and_an_account_change_keeps_the_library() {
    let harness = Harness::open(true, true);
    harness.settle();
    let id = snippet(&harness, ";a", "stay");
    harness.settle();
    harness.full_fetch();
    harness.ack();
    let generation = harness.generation();
    harness.cloud.events(generation).unwrap().fetched(
        generation,
        CloudBatch {
            records: vec![RemoteRecord {
                kind: RecordKind::Meta,
                name: META_RECORD.into(),
                payload: r#"{"syncVersion":2}"#.into(),
            }],
            ..CloudBatch::default()
        },
    );
    harness.settle();
    assert_eq!(*harness.sync.status().get(), SyncStatus::NeedsUpdate);
    assert!(harness
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .any(|s| s.id == id));
    let sends = harness.cloud.state.lock().unwrap().sends.len();
    let fetches = harness.cloud.state.lock().unwrap().fetches.len();
    harness
        .library
        .update_snippet({
            let mut record = harness.library.watch().get().snippets[0].clone();
            record.content = "local edit".into();
            record
        })
        .unwrap();
    harness.settle();
    harness.sync.sync_now();
    harness.settle();
    assert_eq!(harness.cloud.state.lock().unwrap().sends.len(), sends);
    assert!(harness.cloud.state.lock().unwrap().fetches.len() > fetches);
    assert_eq!(*harness.sync.status().get(), SyncStatus::NeedsUpdate);
}

#[test]
fn sync_now_fetches_and_sends_and_zone_deletion_requeues_the_library() {
    let harness = Harness::open(true, true);
    harness.settle();
    let id = snippet(&harness, ";a", "body");
    harness.settle();
    harness.full_fetch();
    harness.ack();
    let fetches = harness.cloud.state.lock().unwrap().fetches.len();
    let sends = harness.cloud.state.lock().unwrap().sends.len();
    harness.sync.sync_now();
    harness.settle();
    assert!(harness.cloud.state.lock().unwrap().fetches.len() > fetches);
    assert!(harness.cloud.state.lock().unwrap().sends.len() > sends);

    let marker = harness.uploaded().len();
    harness
        .library
        .update_snippet({
            let mut record = harness.library.watch().get().snippets[0].clone();
            record.content = "queued".into();
            record
        })
        .unwrap();
    harness.settle();
    assert!(harness.uploaded()[marker..]
        .iter()
        .any(|name| name.contains(&id.to_string())));

    let generation = harness.generation();
    let before = harness.library.watch().get().snippets.len();
    let marker = harness.uploaded().len();
    harness
        .cloud
        .events(generation)
        .unwrap()
        .zone_deleted(generation);
    harness.settle();
    assert_eq!(harness.library.watch().get().snippets.len(), before);
    assert!(harness.uploaded()[marker..]
        .iter()
        .any(|name| name.starts_with("snippet-")));
    let index: SyncIndex =
        serde_json::from_slice(&fs::read(harness.dir.0.join("index.json")).unwrap()).unwrap();
    assert!(index.initial_merge_done);
    assert!(index.records.is_empty());
}

#[test]
fn settings_turn_sync_on_without_a_second_subscription() {
    let harness = Harness::open(true, false);
    harness.settle();
    assert!(harness.cloud.state.lock().unwrap().starts.is_empty());
    harness
        .settings
        .update(|settings| settings.icloud_sync = true)
        .unwrap();
    harness.settle();
    assert_eq!(harness.cloud.state.lock().unwrap().starts.len(), 1);
}

#[test]
fn a_crash_relaunch_from_disk_does_not_delete_the_remote_snippet() {
    let root = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
    fs::create_dir_all(&root.0).unwrap();
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(3600),
        Duration::from_secs(3600),
    )
    .unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let (library, _) = LibraryService::open(
        root.0.join("library.json"),
        "device-a".into(),
        clock.clone(),
        Arc::new(FixedIds::new(100)),
        &writer,
        scheduler.clone(),
    )
    .unwrap();
    let library = Arc::new(library);
    let settings = SettingsService::new(
        Settings {
            icloud_sync: true,
            ..Settings::for_variant(Variant::Production)
        },
        writer.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let cloud = Arc::new(FakeCloud::default());
    let sync_dir = root.0.join("sync");
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: sync_dir.clone(),
        },
        library,
        settings.watch(),
        cloud.clone(),
        clock,
        scheduler,
    )
    .unwrap();
    sync.settle();
    let id = Uuid::from_u128(77);
    let generation = cloud.state.lock().unwrap().active.unwrap();
    cloud.deliver(
        generation,
        CloudBatch {
            records: vec![remote_snippet(id, ";x", "from-icloud", 1_700_000_000)],
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    sync.settle_pending();

    let crash = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
    let crash_sync = crash.0.join("sync");
    fs::create_dir_all(&crash_sync).unwrap();
    for name in ["library.json", "sync/index.json", "sync/engine-state.data"] {
        let from = root.0.join(name);
        if from.is_file() {
            fs::copy(&from, crash.0.join(name)).unwrap();
        }
    }
    drop(sync);

    let clock = Arc::new(FakeClock::default());
    let writer2 = Writer::new(clock.clone(), Duration::ZERO, Duration::from_millis(1)).unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let (library, _) = LibraryService::open(
        crash.0.join("library.json"),
        "device-a".into(),
        clock.clone(),
        Arc::new(FixedIds::new(200)),
        &writer2,
        scheduler.clone(),
    )
    .unwrap();
    let settings = SettingsService::new(
        Settings {
            icloud_sync: true,
            ..Settings::for_variant(Variant::Production)
        },
        writer2.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let cloud = Arc::new(FakeCloud::default());
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: crash_sync,
        },
        Arc::new(library),
        settings.watch(),
        cloud.clone(),
        clock.clone(),
        scheduler,
    )
    .unwrap();
    sync.settle();
    // The saved token is already past the snippet, so the server does not send it again.
    // Ask for a push: a tick or Sync now is what turns the stale index into a delete.
    clock.advance(Duration::from_secs(1));
    let generation = cloud.state.lock().unwrap().active.expect("restarted");
    cloud.deliver(
        generation,
        CloudBatch {
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    sync.settle();
    sync.sync_now();
    sync.settle();
    let name = kudzu_core::sync::snippet_name(id);
    let deleted = cloud
        .state
        .lock()
        .unwrap()
        .sends
        .iter()
        .any(|(_, batch)| batch.deleted.iter().any(|deleted| deleted == &name));
    assert!(
        !deleted,
        "relaunch from the files on disk deleted the remote snippet"
    );
    drop(sync);
    writer2.shutdown().ok();
    writer.shutdown().ok();
}

#[test]
fn no_account_survives_ticks_and_fetch_failures() {
    let harness = Harness::open(true, true);
    harness.settle();
    let generation = harness.generation();
    let events = harness.cloud.events(generation).unwrap();
    events.availability(generation, crate::ports::CloudAvailability::NoAccount);
    harness.settle();
    harness.clock.advance(FETCH_INTERVAL);
    wait_fetch(&harness);
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: NO_ACCOUNT.into(),
        }
    );
    harness.cloud.state.lock().unwrap().failure = Some(PortError::TimedOut);
    harness.sync.sync_now();
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: NO_ACCOUNT.into(),
        }
    );
}

#[test]
fn an_unacknowledged_snippet_is_not_sent_again() {
    let harness = Harness::open(true, true);
    harness.settle();
    harness.full_fetch();
    harness.ack();
    let first = snippet(&harness, ";a", "one");
    harness.settle();
    let name = kudzu_core::sync::snippet_name(first);
    let once = harness
        .uploaded()
        .iter()
        .filter(|uploaded| *uploaded == &name)
        .count();
    assert_eq!(once, 1);
    snippet(&harness, ";b", "two");
    harness.settle();
    let again = harness
        .uploaded()
        .iter()
        .filter(|uploaded| *uploaded == &name)
        .count();
    assert_eq!(again, 1);
}

#[test]
fn a_partial_batch_does_not_publish_up_to_date() {
    let harness = Harness::open(true, true);
    harness.settle();
    let generation = harness.generation();
    harness.deliver(
        generation,
        CloudBatch {
            records: vec![RemoteRecord {
                kind: RecordKind::Meta,
                name: META_RECORD.into(),
                payload: r#"{"syncVersion":1}"#.into(),
            }],
            full_fetch: false,
            ..CloudBatch::default()
        },
    );
    assert!(!matches!(
        *harness.sync.status().get(),
        SyncStatus::UpToDate { .. }
    ));
}

#[test]
fn a_theme_change_does_not_fetch() {
    let harness = Harness::open(true, true);
    harness.settle();
    let fetches = harness.cloud.state.lock().unwrap().fetches.len();
    harness
        .settings
        .update(|settings| settings.theme = Theme::Light)
        .unwrap();
    harness.settle();
    assert_eq!(harness.cloud.state.lock().unwrap().fetches.len(), fetches);
}

#[test]
fn a_failed_start_is_retried_on_the_fetch_interval() {
    let cloud = Arc::new(FakeCloud::default());
    cloud.fail_start(Some(PortError::TimedOut));
    let dir = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
    fs::create_dir_all(&dir.0).unwrap();
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_millis(1)).unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let library = Arc::new(
        LibraryService::new(
            KudzuLibrary::default(),
            "device-a".into(),
            clock.clone(),
            Arc::new(FixedIds::new(1)),
            writer.register(Memory, Revision(0)).unwrap(),
            scheduler.clone(),
        )
        .unwrap(),
    );
    let settings = SettingsService::new(
        Settings::for_variant(Variant::Production),
        writer.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: dir.0.clone(),
        },
        library,
        settings.watch(),
        cloud.clone(),
        clock.clone(),
        scheduler,
    )
    .unwrap();
    sync.settle();
    assert_eq!(
        *sync.status().get(),
        SyncStatus::Error {
            message: START_FAILED.into(),
        }
    );
    assert!(cloud.state.lock().unwrap().starts.is_empty());
    cloud.fail_start(None);
    clock.advance(FETCH_INTERVAL);
    let start = Instant::now();
    loop {
        sync.settle();
        if !cloud.state.lock().unwrap().starts.is_empty() {
            break;
        }
        if start.elapsed() > Duration::from_secs(3) {
            panic!("failed start was not retried");
        }
        thread::sleep(Duration::from_millis(10));
    }
    drop(sync);
    writer.shutdown().ok();
}

#[test]
fn a_failed_index_reset_does_not_start_the_new_account() {
    let harness = Harness::open(true, true);
    harness.settle();
    harness.full_fetch();
    let starts = harness.cloud.state.lock().unwrap().starts.len();
    let _restore = RestorePerms(&harness.dir.0);
    fs::set_permissions(&harness.dir.0, fs::Permissions::from_mode(0o555)).unwrap();
    let generation = harness.generation();
    harness
        .cloud
        .events(generation)
        .unwrap()
        .account_changed(generation, AccountChange::Switch);
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::Error {
            message: SAVE_FAILED.into(),
        }
    );
    assert_eq!(harness.cloud.state.lock().unwrap().starts.len(), starts);
}

struct RestorePerms<'a>(&'a std::path::Path);
impl Drop for RestorePerms<'_> {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.0, fs::Permissions::from_mode(0o755));
    }
}

fn wait_durable(library: &LibraryService, revision: Revision) {
    let start = Instant::now();
    loop {
        let saved = library.saved().get();
        if saved.error.is_none() && saved.durable >= revision {
            return;
        }
        if start.elapsed() > Duration::from_secs(3) {
            panic!("library was not durable: {saved:?}");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Remote delete recorded in the index while `library.json` still contains the snippet.
/// Relaunch must not upload it again.
#[test]
fn a_crash_relaunch_from_disk_does_not_restore_a_remote_delete() {
    let root = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
    fs::create_dir_all(&root.0).unwrap();
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_millis(1)).unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let (library, _) = LibraryService::open(
        root.0.join("library.json"),
        "device-a".into(),
        clock.clone(),
        Arc::new(FixedIds::new(100)),
        &writer,
        scheduler.clone(),
    )
    .unwrap();
    let library = Arc::new(library);
    wait_durable(&library, Revision(1));
    let id = snippet_on(&library, ";keep", "on-disk");
    wait_durable(&library, library.watch().revision());
    let settings = SettingsService::new(
        Settings {
            icloud_sync: true,
            ..Settings::for_variant(Variant::Production)
        },
        writer.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let cloud = Arc::new(FakeCloud::default());
    let sync_dir = root.0.join("sync");
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: sync_dir.clone(),
        },
        library.clone(),
        settings.watch(),
        cloud.clone(),
        clock.clone(),
        scheduler.clone(),
    )
    .unwrap();
    sync.settle();
    let generation = cloud.state.lock().unwrap().active.unwrap();
    cloud.deliver(
        generation,
        CloudBatch {
            records: vec![remote_snippet(id, ";keep", "on-disk", 2_000_000_000)],
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    sync.settle();
    wait_durable(&library, library.watch().revision());
    sync.settle();
    let indexed: SyncIndex =
        serde_json::from_slice(&fs::read(sync_dir.join("index.json")).unwrap()).unwrap();
    let name = kudzu_core::sync::snippet_name(id);
    assert!(indexed.records.contains_key(&name));
    drop(sync);
    writer.shutdown().unwrap();

    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(3600),
        Duration::from_secs(3600),
    )
    .unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let (library, _) = LibraryService::open(
        root.0.join("library.json"),
        "device-a".into(),
        clock.clone(),
        Arc::new(FixedIds::new(100)),
        &writer,
        scheduler.clone(),
    )
    .unwrap();
    let library = Arc::new(library);
    let settings = SettingsService::new(
        Settings {
            icloud_sync: true,
            ..Settings::for_variant(Variant::Production)
        },
        writer.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let cloud = Arc::new(FakeCloud::default());
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: sync_dir.clone(),
        },
        library,
        settings.watch(),
        cloud.clone(),
        clock,
        scheduler,
    )
    .unwrap();
    sync.settle();
    let generation = cloud.state.lock().unwrap().active.unwrap();
    cloud.deliver(
        generation,
        CloudBatch {
            deleted: vec![name.clone()],
            ..CloudBatch::default()
        },
    );
    sync.settle_pending();

    let crash = TempDir(std::env::temp_dir().join(format!("kudzu-sync-{}", Uuid::new_v4())));
    let crash_sync = crash.0.join("sync");
    fs::create_dir_all(&crash_sync).unwrap();
    for file_name in ["library.json", "sync/index.json", "sync/engine-state.data"] {
        let from = root.0.join(file_name);
        if from.is_file() {
            fs::copy(&from, crash.0.join(file_name)).unwrap();
        }
    }
    drop(sync);

    let clock = Arc::new(FakeClock::default());
    let writer2 = Writer::new(clock.clone(), Duration::ZERO, Duration::from_millis(1)).unwrap();
    let scheduler = Arc::new(Scheduler::new(clock.clone()).unwrap());
    let (library, _) = LibraryService::open(
        crash.0.join("library.json"),
        "device-a".into(),
        clock.clone(),
        Arc::new(FixedIds::new(200)),
        &writer2,
        scheduler.clone(),
    )
    .unwrap();
    let settings = SettingsService::new(
        Settings {
            icloud_sync: true,
            ..Settings::for_variant(Variant::Production)
        },
        writer2.register(Memory, Revision(0)).unwrap(),
    )
    .unwrap();
    let cloud = Arc::new(FakeCloud::default());
    let sync = SyncService::open(
        SyncConfig {
            production: true,
            device_id: "device-a".into(),
            directory: crash_sync,
        },
        Arc::new(library),
        settings.watch(),
        cloud.clone(),
        clock.clone(),
        scheduler,
    )
    .unwrap();
    sync.settle();
    sync.sync_now();
    sync.settle();
    let uploaded = cloud
        .state
        .lock()
        .unwrap()
        .sends
        .iter()
        .any(|(_, batch)| batch.records.iter().any(|record| record.name == name));
    assert!(
        !uploaded,
        "relaunch from the files on disk uploaded a snippet iCloud had already deleted"
    );
    drop(sync);
    writer2.shutdown().ok();
    writer.shutdown().ok();
}

fn snippet_on(library: &LibraryService, trigger: &str, content: &str) -> Uuid {
    library
        .create_snippet(SnippetDraft {
            triggers: vec![trigger.into()],
            content: content.into(),
            ..SnippetDraft::default()
        })
        .unwrap()
        .0
        .id
}

#[test]
fn a_conflict_takes_the_newer_server_copy_and_a_requeue_sends_again() {
    let harness = Harness::open(true, true);
    harness.settle();
    let id = snippet(&harness, ";a", "local");
    harness.settle();
    harness.full_fetch();
    let generation = harness.generation();
    let events = harness.cloud.events(generation).unwrap();
    events.sent(
        generation,
        CloudSend {
            conflicts: vec![remote_snippet(id, ";a", "from-server", 2_000_000_000)],
            ..CloudSend::default()
        },
    );
    harness.settle();
    let library = harness.library.watch().get();
    let stored = library
        .snippets
        .iter()
        .find(|snippet| snippet.id == id)
        .expect("server copy");
    assert_eq!(stored.content, "from-server");
    drop(library);

    // The conflict merged the server payload into the index, so that name is no longer pending.
    // Requeue an unacknowledged local edit: the index still has the old payload.
    let pending = snippet(&harness, ";b", "pending");
    harness.settle();
    let name = kudzu_core::sync::snippet_name(pending);
    let once = harness
        .uploaded()
        .iter()
        .filter(|uploaded| *uploaded == &name)
        .count();
    assert_eq!(once, 1);
    events.sent(
        generation,
        CloudSend {
            requeue: vec![name.clone()],
            ..CloudSend::default()
        },
    );
    harness.settle();
    let again = harness
        .uploaded()
        .iter()
        .filter(|uploaded| *uploaded == &name)
        .count();
    assert!(again > once, "requeue did not send {name} again");
}

#[test]
fn a_remote_tombstone_removes_the_snippet_and_a_local_delete_is_sent() {
    let harness = Harness::open(true, true);
    harness.settle();
    let remote_id = Uuid::from_u128(15);
    let generation = harness.generation();
    harness.deliver(
        generation,
        CloudBatch {
            records: vec![remote_snippet(remote_id, ";remote", "there", 30)],
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    let remote_name = kudzu_core::sync::snippet_name(remote_id);
    harness.deliver(
        generation,
        CloudBatch {
            deleted: vec![remote_name],
            full_fetch: true,
            ..CloudBatch::default()
        },
    );
    assert!(harness
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .all(|snippet| snippet.id != remote_id));

    let local = snippet(&harness, ";gone", "local");
    harness.settle();
    harness.ack();
    harness.library.delete_snippets(&[local]).unwrap();
    harness.settle();
    let name = kudzu_core::sync::snippet_name(local);
    assert!(harness
        .cloud
        .state
        .lock()
        .unwrap()
        .sends
        .iter()
        .any(|(_, batch)| batch.deleted.iter().any(|deleted| deleted == &name)));
}

#[test]
fn zone_missing_sends_an_unacknowledged_snippet_again() {
    let harness = Harness::open(true, true);
    harness.settle();
    harness.full_fetch();
    harness.ack();
    let id = snippet(&harness, ";z", "again");
    harness.settle();
    let name = kudzu_core::sync::snippet_name(id);
    let once = harness
        .uploaded()
        .iter()
        .filter(|uploaded| *uploaded == &name)
        .count();
    assert_eq!(once, 1);
    let generation = harness.generation();
    harness.cloud.events(generation).unwrap().sent(
        generation,
        CloudSend {
            zone_missing: true,
            ..CloudSend::default()
        },
    );
    harness.settle();
    let again = harness
        .uploaded()
        .iter()
        .filter(|uploaded| *uploaded == &name)
        .count();
    assert!(again > once, "zone_missing left {name} in flight");
}

#[test]
fn an_earlier_generations_tick_does_not_fetch_after_an_account_change() {
    let harness = Harness::open(true, true);
    harness.settle();
    let first = harness.generation();
    harness
        .cloud
        .events(first)
        .unwrap()
        .account_changed(first, AccountChange::Switch);
    harness.settle();
    let fetches = harness.cloud.state.lock().unwrap().fetches.clone();
    harness.sync.enqueue_tick(first);
    harness.settle();
    assert_eq!(harness.cloud.state.lock().unwrap().fetches, fetches);
}

#[test]
fn signed_out_stays_until_the_new_account_is_available() {
    let harness = Harness::open(true, true);
    harness.settle();
    let first = harness.generation();
    harness
        .cloud
        .events(first)
        .unwrap()
        .account_changed(first, AccountChange::SignOut);
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: SIGNED_OUT.into(),
        }
    );
    harness.clock.advance(FETCH_INTERVAL);
    wait_fetch(&harness);
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::NoAccount {
            reason: SIGNED_OUT.into(),
        }
    );
    let generation = harness.generation();
    harness
        .cloud
        .events(generation)
        .unwrap()
        .availability(generation, CloudAvailability::Available);
    harness.settle();
    let status = harness.sync.status().get();
    assert!(!matches!(
        status.as_ref(),
        SyncStatus::NoAccount { reason } if reason == SIGNED_OUT
    ));
}

#[test]
fn could_not_determine_leaves_the_current_status() {
    let harness = Harness::open(true, true);
    harness.settle();
    assert_eq!(*harness.sync.status().get(), SyncStatus::Syncing);
    let generation = harness.generation();
    harness
        .cloud
        .events(generation)
        .unwrap()
        .availability(generation, CloudAvailability::CouldNotDetermine);
    harness.settle();
    assert_eq!(*harness.sync.status().get(), SyncStatus::Syncing);
}

#[test]
fn state_is_ordered_behind_the_fetch_and_empty_bytes_do_not_truncate() {
    let harness = Harness::open(true, true);
    harness.settle();
    harness.full_fetch();
    assert!(!harness.dir.0.join("engine-state.data").is_file());
    let generation = harness.generation();
    let events = harness.cloud.events(generation).unwrap();
    events.state(generation, b"after".to_vec());
    harness.settle();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"after"
    );
    events.state(generation, Vec::new());
    harness.settle();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"after"
    );
}

#[test]
fn a_state_event_from_inside_fetch_is_applied_on_the_following_turn() {
    let harness = Harness::open(true, true);
    harness.settle();
    harness
        .cloud
        .script(vec![CloudScript::State(b"from-fetch".to_vec())]);
    harness.sync.sync_now();
    harness.settle();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"from-fetch"
    );
}

#[test]
fn a_worker_panic_stops_a_started_port_and_dev_does_not() {
    let harness = Harness::open(true, true);
    harness.settle();
    assert_eq!(harness.cloud.state.lock().unwrap().stops, 0);
    harness.sync.panic_for_test();
    harness.settle();
    assert_eq!(
        *harness.sync.status().get(),
        SyncStatus::Error {
            message: WORKER_STOPPED.into(),
        }
    );
    let cloud = harness.cloud.clone();
    drop(harness);
    assert_eq!(cloud.state.lock().unwrap().stops, 1);

    let dev = Harness::open(false, true);
    dev.settle();
    dev.sync.panic_for_test();
    dev.settle();
    assert_eq!(
        *dev.sync.status().get(),
        SyncStatus::Error {
            message: WORKER_STOPPED.into(),
        }
    );
    let cloud = dev.cloud.clone();
    drop(dev);
    assert_eq!(cloud.state.lock().unwrap().stops, 0);
}

#[test]
fn turning_sync_off_keeps_the_saved_engine_state() {
    let harness = Harness::open(true, true);
    harness.settle();
    let generation = harness.generation();
    harness
        .cloud
        .events(generation)
        .unwrap()
        .state(generation, b"token".to_vec());
    harness.settle();
    harness.sync.set_enabled(false);
    harness.settle();
    assert_eq!(
        fs::read(harness.dir.0.join("engine-state.data")).unwrap(),
        b"token"
    );
}

/// A local delete that is still in flight, followed by a remote modify of the same record,
/// brings the snippet back. Tauri did the same; the merge rule is unchanged.
#[test]
fn a_local_delete_in_flight_plus_a_remote_modify_restores_the_snippet() {
    let harness = Harness::open(true, true);
    harness.settle();
    let id = snippet(&harness, ";back", "original");
    harness.settle();
    harness.full_fetch();
    harness.ack();
    harness.library.delete_snippets(&[id]).unwrap();
    harness.settle();
    assert!(harness
        .library
        .watch()
        .get()
        .snippets
        .iter()
        .all(|snippet| snippet.id != id));
    let generation = harness.generation();
    harness.deliver(
        generation,
        CloudBatch {
            records: vec![remote_snippet(
                id,
                ";back",
                "modified-remotely",
                2_000_000_000,
            )],
            ..CloudBatch::default()
        },
    );
    let library = harness.library.watch().get();
    let restored = library.snippets.iter().find(|snippet| snippet.id == id);
    assert_eq!(
        restored.as_ref().map(|snippet| snippet.content.as_str()),
        Some("modified-remotely")
    );
}
