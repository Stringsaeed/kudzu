use super::*;
use crate::fixtures::clock::FakeClock;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

#[test]
fn shutdown_times_out_without_claiming_durability_or_waiting_again() {
    struct Blocked {
        entered: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl WriteTarget<u64> for Blocked {
        fn write(&self, _: &u64) -> Result<(), PersistError> {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            Ok(())
        }
    }
    let (entered, began) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let writer = Writer::new(
        Arc::new(FakeClock::default()),
        Duration::ZERO,
        Duration::from_secs(1),
    )
    .unwrap();
    let persisted = writer
        .register(
            Blocked {
                entered,
                release: Mutex::new(wait),
            },
            Revision(0),
        )
        .unwrap();
    persisted.submit(Revision(1), Arc::new(1)).unwrap();
    began.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        writer.shutdown_with_timeout(Duration::from_millis(20)),
        Err(PersistError::Timeout)
    );
    assert_eq!(persisted.saved().get().durable, Revision(0));
    assert_eq!(
        persisted.submit(Revision(2), Arc::new(2)),
        Err(PersistError::Stopped)
    );
    assert_eq!(writer.shutdown(), Err(PersistError::Timeout));
    release.send(()).unwrap();
}

#[test]
fn panicking_target_reports_failure_and_retries_without_killing_the_writer() {
    struct Once(AtomicUsize);
    impl WriteTarget<u64> for Once {
        fn write(&self, _: &u64) -> Result<(), PersistError> {
            assert_ne!(self.0.fetch_add(1, Ordering::SeqCst), 0, "target failed");
            Ok(())
        }
    }
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_secs(1)).unwrap();
    let persisted = writer
        .register(Once(AtomicUsize::new(0)), Revision(0))
        .unwrap();
    let watch = persisted.saved();
    let read = watch.clone();
    let _bad = watch.subscribe(|| panic!("save waker failed"));
    let (tx, rx) = mpsc::channel();
    let _good = watch.subscribe(move || {
        tx.send(*read.get()).unwrap();
    });
    persisted.submit(Revision(1), Arc::new(10)).unwrap();
    let failed = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("target panic killed writer");
    assert_eq!(failed.error, Some(PersistError::Panicked));
    assert_eq!(failed.durable, Revision(0));
    clock.advance(Duration::from_secs(1));
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        SaveState {
            durable: Revision(1),
            error: None
        }
    );
    writer.shutdown().unwrap();
}

#[test]
fn continuous_edits_cannot_postpone_a_save_past_five_seconds() {
    struct Sent(mpsc::Sender<u64>);
    impl WriteTarget<u64> for Sent {
        fn write(&self, value: &u64) -> Result<(), PersistError> {
            self.0.send(*value).unwrap();
            Ok(())
        }
    }
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(
        clock.clone(),
        Duration::from_secs(2),
        Duration::from_secs(1),
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let persisted = writer.register(Sent(tx), Revision(0)).unwrap();
    persisted.submit(Revision(1), Arc::new(1)).unwrap();
    for revision in 2..=5 {
        clock.advance(Duration::from_secs(1));
        persisted
            .submit(Revision(revision), Arc::new(revision))
            .unwrap();
    }
    assert!(rx.try_recv().is_err());
    clock.advance(Duration::from_secs(1));
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(2))
            .expect("debounce starved persistence"),
        5
    );
    writer.shutdown().unwrap();
}

#[test]
fn retry_delay_starts_after_a_slow_failed_write() {
    struct SlowFailure {
        clock: Arc<FakeClock>,
        writes: AtomicUsize,
        tx: mpsc::Sender<()>,
    }
    impl WriteTarget<u64> for SlowFailure {
        fn write(&self, _: &u64) -> Result<(), PersistError> {
            if self.writes.fetch_add(1, Ordering::SeqCst) == 0 {
                self.clock.advance(Duration::from_secs(10));
                return Err(PersistError::Encode);
            }
            self.tx.send(()).unwrap();
            Ok(())
        }
    }
    let clock = Arc::new(FakeClock::default());
    let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_secs(5)).unwrap();
    let (tx, rx) = mpsc::channel();
    let persisted = writer
        .register(
            SlowFailure {
                clock: clock.clone(),
                writes: AtomicUsize::new(0),
                tx,
            },
            Revision(0),
        )
        .unwrap();
    let saved = persisted.saved();
    let read = saved.clone();
    let (failed, failure) = mpsc::channel();
    let _sub = saved.subscribe(move || {
        if read.get().error.is_some() {
            failed.send(()).unwrap();
        }
    });
    persisted.submit(Revision(1), Arc::new(10)).unwrap();
    failure.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(30)).is_err(),
        "retry ran immediately after the slow write"
    );
    clock.advance(Duration::from_secs(4));
    assert!(rx.recv_timeout(Duration::from_millis(30)).is_err());
    clock.advance(Duration::from_secs(1));
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
    writer.shutdown().unwrap();
}

#[test]
fn open_sweeps_only_abandoned_atomic_write_temporaries() {
    let dir = Folder::new();
    fs::write(dir.0.join(".kudzu-abandoned.tmp"), "unfinished").unwrap();
    fs::write(dir.0.join("unrelated.tmp"), "keep").unwrap();
    fs::create_dir(dir.0.join(".kudzu-directory.tmp")).unwrap();
    JsonFile::<u64>::open(dir.0.join("settings.json"), || 0).unwrap();
    assert!(!dir.0.join(".kudzu-abandoned.tmp").exists());
    assert!(dir.0.join("unrelated.tmp").exists());
    assert!(dir.0.join(".kudzu-directory.tmp").is_dir());
}

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("kudzu-persist-review-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unexpected_writer_exit_rejects_new_submissions() {
    struct BrokenClock;
    impl Clock for BrokenClock {
        fn now(&self) -> std::time::SystemTime {
            std::time::SystemTime::UNIX_EPOCH
        }
        fn monotonic(&self) -> Duration {
            assert_ne!(
                std::thread::current().name(),
                Some("kudzu-persist"),
                "clock failed"
            );
            Duration::ZERO
        }
        fn subscribe(&self, _: Arc<dyn Fn() + Send + Sync>) -> Subscription {
            Subscription::default()
        }
    }
    struct Memory;
    impl WriteTarget<u64> for Memory {
        fn write(&self, _: &u64) -> Result<(), PersistError> {
            Ok(())
        }
    }
    let writer = Writer::new(
        Arc::new(BrokenClock),
        Duration::ZERO,
        Duration::from_secs(1),
    )
    .unwrap();
    let persisted = writer.register(Memory, Revision(0)).unwrap();
    persisted.submit(Revision(1), Arc::new(1)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while persisted.submit(Revision(1), Arc::new(1)).is_ok() {
        assert!(
            std::time::Instant::now() < deadline,
            "dead worker still accepts writes"
        );
        std::thread::yield_now();
    }
    assert_eq!(
        persisted.submit(Revision(2), Arc::new(2)),
        Err(PersistError::Stopped)
    );
    assert_eq!(writer.shutdown(), Err(PersistError::WorkerStopped));
}

#[test]
fn opening_another_store_preserves_a_live_atomic_write() {
    let dir = Folder::new();
    let live = temporary::Temporary::reserve(&dir.0).unwrap();
    fs::write(live.path(), b"writing").unwrap();
    JsonFile::<u64>::open(dir.0.join("settings.json"), || 0).unwrap();
    assert_eq!(fs::read(live.path()).unwrap(), b"writing");
    let path = live.path().to_owned();
    drop(live);
    assert!(!path.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn rename_failure_preserves_the_old_file_and_cleans_the_temporary() {
    let dir = Folder::new();
    let path = dir.0.join("library.json");
    let (file, _) = JsonFile::open(path.clone(), || 0u64).unwrap();
    file.write(&10).unwrap();
    struct Immutable(PathBuf);
    impl Drop for Immutable {
        fn drop(&mut self) {
            let _ = std::process::Command::new("chflags")
                .arg("nouchg")
                .arg(&self.0)
                .status();
        }
    }
    assert!(std::process::Command::new("chflags")
        .arg("uchg")
        .arg(&path)
        .status()
        .unwrap()
        .success());
    let immutable = Immutable(path.clone());
    assert!(matches!(file.write(&20), Err(PersistError::Io(_))));
    assert_eq!(fs::read_to_string(path).unwrap(), "10");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    drop(immutable);
}
