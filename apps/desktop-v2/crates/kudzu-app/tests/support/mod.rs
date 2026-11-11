//! Shared rig for the data service tests: temp folders, in-memory library and settings, a fake
//! clock and a blocking wait for a `Task`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use kudzu_app::data::{DataConfig, DataService};
use kudzu_app::fixtures::{clock::FakeClock, ids::FixedIds};
use kudzu_app::library::LibraryService;
use kudzu_app::persist::{PersistError, Revision, WriteTarget, Writer};
use kudzu_app::schedule::Scheduler;
use kudzu_app::settings::{Settings, SettingsService, Variant};
use kudzu_app::tasks::Task;
use kudzu_core::interchange::backup::BackupTimeZone;
use kudzu_core::KudzuLibrary;

/// A temporary directory removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("kudzu-{label}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn utc_zone() -> BackupTimeZone {
    BackupTimeZone::Fixed(FixedOffset::east_opt(0).unwrap())
}

struct Memory;
impl<T> WriteTarget<T> for Memory {
    fn write(&self, _: &T) -> Result<(), PersistError> {
        Ok(())
    }
}

/// Library, settings and data services over in-memory targets and a fake clock.
pub struct Rig {
    pub dir: TempDir,
    pub clock: Arc<FakeClock>,
    pub library: Arc<LibraryService>,
    pub settings: Arc<SettingsService>,
    pub data: DataService,
    pub scheduler: Arc<Scheduler>,
    _writer: Writer,
}

impl Rig {
    pub fn new(initial: KudzuLibrary) -> Self {
        let dir = TempDir::new("data-rig");
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
                initial,
                "device-a".into(),
                clock.clone(),
                Arc::new(FixedIds::new(1_000)),
                writer.register(Memory, Revision(0)).unwrap(),
                scheduler.clone(),
            )
            .unwrap(),
        );
        let settings = Arc::new(
            SettingsService::new(
                Settings::for_variant(Variant::Production),
                writer.register(Memory, Revision(0)).unwrap(),
            )
            .unwrap(),
        );
        let data = DataService::new(
            DataConfig {
                backups: dir.path().join("backups"),
                home: dir.path().join("home"),
                time_zone: utc_zone(),
            },
            library.clone(),
            settings.clone(),
            clock.clone(),
        );
        Self {
            dir,
            clock,
            library,
            settings,
            data,
            scheduler,
            _writer: writer,
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        kudzu_app::ports::Clock::now(&*self.clock).into()
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.dir.path().join("backups")
    }

    pub fn file_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

/// Blocks until the task finishes (5 s limit), then returns its result.
pub fn wait<T: Send + 'static>(task: Task<T>) -> T {
    let (wake, woke) = mpsc::channel();
    let ready = task.ready();
    let _subscription = ready.subscribe(move || {
        let _ = wake.send(());
    });
    while !*ready.get() {
        woke.recv_timeout(Duration::from_secs(5))
            .expect("task did not finish");
    }
    task.take().expect("task result").expect("task panicked")
}

/// Blocks until `ready(&snapshot)` holds for the watch (5 s limit).
pub fn wait_for<T: Send + Sync + 'static>(
    watch: &kudzu_app::watch::Watch<T>,
    ready: impl Fn(&T) -> bool,
) {
    let (wake, woke) = mpsc::channel();
    let _subscription = watch.subscribe(move || {
        let _ = wake.send(());
    });
    while !ready(&watch.get()) {
        woke.recv_timeout(Duration::from_secs(5))
            .expect("watch never reached the expected state");
    }
}
