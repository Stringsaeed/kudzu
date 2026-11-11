//! Atomic JSON storage and a shared writer. Memory acceptance and disk durability are separate.

use crate::error::IoFailure;
use crate::ports::clock::Clock;
use crate::watch::{Publisher, Subscription, Watch};
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Revision(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PersistError {
    #[error("The file could not be saved.")]
    Io(#[from] IoFailure),
    #[error("The file could not be read.")]
    Decode,
    #[error("The data could not be saved.")]
    Encode,
    #[error("This file needs a newer version of Kudzu.")]
    UnsupportedVersion,
    #[error("The save writer has stopped.")]
    Stopped,
    #[error("The save writer stopped unexpectedly.")]
    WorkerStopped,
}
impl From<std::io::Error> for PersistError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadSource {
    Missing,
    Existing,
    Recovered(PathBuf),
}
pub struct Loaded<T> {
    pub value: T,
    pub source: LoadSource,
}

/// The only disk sink used by the writer; also permits deterministic I/O fakes in tests.
pub trait WriteTarget<T>: Send + Sync {
    fn write(&self, value: &T) -> Result<(), PersistError>;
}

type Encoder<T> = fn(&T) -> Result<Vec<u8>, PersistError>;
pub struct JsonFile<T> {
    path: PathBuf,
    encode: Encoder<T>,
}
impl<T: Serialize + DeserializeOwned> JsonFile<T> {
    pub fn open(
        path: PathBuf,
        defaults: impl FnOnce() -> T,
    ) -> Result<(Self, Loaded<T>), PersistError> {
        Self::open_with(
            path,
            defaults,
            |bytes| serde_json::from_slice(bytes).map_err(|_| PersistError::Decode),
            |value| serde_json::to_vec_pretty(value).map_err(|_| PersistError::Encode),
        )
    }
}
impl<T> JsonFile<T> {
    /// Codecs must discard raw parser messages. Only `Decode` quarantines; newer formats stay put.
    pub fn open_with(
        path: PathBuf,
        defaults: impl FnOnce() -> T,
        decode: impl FnOnce(&[u8]) -> Result<T, PersistError>,
        encode: Encoder<T>,
    ) -> Result<(Self, Loaded<T>), PersistError> {
        let loaded = match fs::read(&path) {
            Ok(bytes) => match decode(&bytes) {
                Ok(value) => Loaded {
                    value,
                    source: LoadSource::Existing,
                },
                Err(PersistError::Decode) => {
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                    let aside = path.with_file_name(format!(
                        "{stem}-corrupt-{stamp}-{}.json",
                        uuid::Uuid::new_v4()
                    ));
                    // Hard-link then unlink is no-clobber: another quarantine is never replaced.
                    fs::hard_link(&path, &aside)?;
                    fs::remove_file(&path)?;
                    sync_parent(&path)?;
                    Loaded {
                        value: defaults(),
                        source: LoadSource::Recovered(aside),
                    }
                }
                Err(error) => return Err(error),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Loaded {
                value: defaults(),
                source: LoadSource::Missing,
            },
            Err(error) => return Err(error.into()),
        };
        Ok((Self { path, encode }, loaded))
    }
}
impl<T> WriteTarget<T> for JsonFile<T> {
    fn write(&self, value: &T) -> Result<(), PersistError> {
        let bytes = (self.encode)(value)?;
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let temp = parent.join(format!(".kudzu-{}.tmp", uuid::Uuid::new_v4()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_file(&self.0);
            }
        }
        let _cleanup = Cleanup(temp.clone());
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temp, &self.path)?;
        sync_parent(&self.path)?;
        Ok(())
    }
}
fn sync_parent(path: &Path) -> Result<(), PersistError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

/// `durable` advances only after fsync + rename + directory fsync succeeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SaveState {
    pub durable: Revision,
    pub error: Option<PersistError>,
}
struct Pending<T> {
    latest: Revision,
    value: Option<(Revision, Arc<T>)>,
    due: Duration,
}
struct Slot<T> {
    pending: Mutex<Pending<T>>,
    target: Box<dyn WriteTarget<T>>,
    saved: Publisher<SaveState>,
}
trait PendingWrite: Send + Sync {
    fn due(&self) -> Option<Duration>;
    fn attempt(&self, now: Duration, retry: Duration) -> Result<(), PersistError>;
}
impl<T: Send + Sync> PendingWrite for Slot<T> {
    fn due(&self) -> Option<Duration> {
        let pending = self.pending.lock().unwrap();
        pending.value.as_ref().map(|_| pending.due)
    }
    fn attempt(&self, now: Duration, retry: Duration) -> Result<(), PersistError> {
        let Some((revision, value)) = self.pending.lock().unwrap().value.clone() else {
            return Ok(());
        };
        let result = self.target.write(&value);
        let notification = {
            let mut pending = self.pending.lock().unwrap();
            let current = self.saved.watch().get();
            let state = match result {
                Ok(()) => {
                    if pending.latest == revision {
                        pending.value = None;
                    }
                    SaveState {
                        durable: revision.max(current.durable),
                        error: None,
                    }
                }
                Err(error) => {
                    if pending.latest == revision {
                        pending.due = now.saturating_add(retry);
                    }
                    SaveState {
                        durable: current.durable,
                        error: Some(error),
                    }
                }
            };
            self.saved.replace(state)
        };
        notification.notify();
        result
    }
}
#[derive(Default)]
struct Queue {
    stopped: bool,
    slots: Vec<Arc<dyn PendingWrite>>,
}
struct Shared {
    queue: Mutex<Queue>,
    changed: Condvar,
    clock: Arc<dyn Clock>,
    debounce: Duration,
}

/// Owned by a persisted service. Submit under its transaction lock; no callbacks or I/O run here.
pub struct Persisted<T> {
    slot: Arc<Slot<T>>,
    writer: Weak<Shared>,
}
impl<T> Persisted<T> {
    pub fn saved(&self) -> Watch<SaveState> {
        self.slot.saved.watch()
    }
    /// Ignore stale/out-of-order revisions. A snapshot remains retained until a successful save.
    pub fn submit(&self, revision: Revision, value: Arc<T>) -> Result<(), PersistError> {
        let writer = self.writer.upgrade().ok_or(PersistError::Stopped)?;
        let queue = writer.queue.lock().unwrap();
        if queue.stopped {
            return Err(PersistError::Stopped);
        }
        let mut pending = self.slot.pending.lock().unwrap();
        if revision > pending.latest {
            pending.latest = revision;
            pending.value = Some((revision, value));
            pending.due = writer.clock.monotonic().saturating_add(writer.debounce);
            writer.changed.notify_one();
        }
        Ok(())
    }
}
struct Worker {
    join: Option<JoinHandle<Result<(), PersistError>>>,
    result: Result<(), PersistError>,
}

/// Composition creates one writer for all domain files and owns its shutdown.
/// Registered targets live until shutdown even when their service handle is dropped.
pub struct Writer {
    shared: Arc<Shared>,
    worker: Mutex<Worker>,
    _clock_wake: Subscription,
}
impl Writer {
    pub fn new(
        clock: Arc<dyn Clock>,
        debounce: Duration,
        retry: Duration,
    ) -> Result<Self, PersistError> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            changed: Condvar::new(),
            clock: clock.clone(),
            debounce,
        });
        let wake = Arc::downgrade(&shared);
        let subscription = clock.subscribe(Arc::new(move || {
            if let Some(shared) = wake.upgrade() {
                let _queue = shared.queue.lock().unwrap();
                shared.changed.notify_one();
            }
        }));
        let thread_state = shared.clone();
        let join = std::thread::Builder::new()
            .name("kudzu-persist".into())
            .spawn(move || writer_loop(thread_state, retry.max(Duration::from_millis(1))))?;
        Ok(Self {
            shared,
            worker: Mutex::new(Worker {
                join: Some(join),
                result: Ok(()),
            }),
            _clock_wake: subscription,
        })
    }
    /// A file has one registration and one owning service. Loaded state starts at revision zero.
    pub fn register<T: Send + Sync + 'static>(
        &self,
        target: impl WriteTarget<T> + 'static,
        durable: Revision,
    ) -> Result<Persisted<T>, PersistError> {
        let mut queue = self.shared.queue.lock().unwrap();
        if queue.stopped {
            return Err(PersistError::Stopped);
        }
        let slot = Arc::new(Slot {
            pending: Mutex::new(Pending {
                latest: durable,
                value: None,
                due: Duration::ZERO,
            }),
            target: Box::new(target),
            saved: Publisher::new(SaveState {
                durable,
                error: None,
            }),
        });
        queue.slots.push(slot.clone());
        Ok(Persisted {
            slot,
            writer: Arc::downgrade(&self.shared),
        })
    }
    /// Stop admissions, bypass debounce, attempt every pending file, and join the writer.
    /// S40 must stop producers first and handle an error before treating shutdown as durable.
    /// No worker may wait on the UI. Repeated calls return the same result.
    pub fn shutdown(&self) -> Result<(), PersistError> {
        let mut worker = self.worker.lock().unwrap();
        if let Some(join) = worker.join.take() {
            {
                let mut queue = self.shared.queue.lock().unwrap();
                queue.stopped = true;
                self.shared.changed.notify_one();
            }
            worker.result = join.join().unwrap_or(Err(PersistError::WorkerStopped));
        }
        worker.result
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
fn writer_loop(shared: Arc<Shared>, retry: Duration) -> Result<(), PersistError> {
    let mut queue = shared.queue.lock().unwrap();
    loop {
        if queue.stopped {
            let slots = queue.slots.clone();
            drop(queue);
            let mut result = Ok(());
            for slot in slots {
                if let Err(error) = slot.attempt(shared.clock.monotonic(), retry) {
                    result = Err(error);
                }
            }
            return result;
        }
        let next = queue
            .slots
            .iter()
            .filter_map(|slot| slot.due().map(|due| (due, slot.clone())))
            .min_by_key(|(due, _)| *due);
        let Some((due, slot)) = next else {
            queue = shared.changed.wait(queue).unwrap();
            continue;
        };
        let now = shared.clock.monotonic();
        if due > now {
            queue = shared.changed.wait_timeout(queue, due - now).unwrap().0;
            continue;
        }
        drop(queue);
        let _ = slot.attempt(now, retry);
        queue = shared.queue.lock().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::clock::FakeClock;
    use std::sync::mpsc;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };
    use std::time::Duration;

    #[test]
    fn delayed_save_acknowledges_only_written_revision_and_shutdown_drains() {
        struct Slow {
            file: JsonFile<u64>,
            entered: mpsc::Sender<()>,
            release: Mutex<mpsc::Receiver<()>>,
        }
        impl WriteTarget<u64> for Slow {
            fn write(&self, value: &u64) -> Result<(), PersistError> {
                if *value == 1 {
                    self.entered.send(()).unwrap();
                    self.release.lock().unwrap().recv().unwrap();
                }
                self.file.write(value)
            }
        }
        let dir = Folder::new();
        let (file, _) = JsonFile::open(dir.0.join("library.json"), || 0).unwrap();
        let (entered, began) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let writer = Arc::new(
            Writer::new(
                Arc::new(FakeClock::default()),
                Duration::ZERO,
                Duration::from_secs(1),
            )
            .unwrap(),
        );
        let saves = writer
            .register(
                Slow {
                    file,
                    entered,
                    release: Mutex::new(wait),
                },
                Revision(0),
            )
            .unwrap();
        saves.submit(Revision(1), Arc::new(1)).unwrap();
        began.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(saves.saved().get().durable, Revision(0));
        saves.submit(Revision(2), Arc::new(2)).unwrap();
        saves.submit(Revision(1), Arc::new(99)).unwrap(); // stale submit cannot replace latest
        let (done, drained) = mpsc::channel();
        let shutdown = writer.clone();
        let thread = std::thread::spawn(move || {
            done.send(shutdown.shutdown()).unwrap();
        });
        assert!(drained.recv_timeout(Duration::from_millis(20)).is_err());
        release.send(()).unwrap();
        drained
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap();
        thread.join().unwrap();
        assert_eq!(saves.saved().get().durable, Revision(2));
        assert!(saves.saved().get().error.is_none());
        let (_, loaded) = JsonFile::<u64>::open(dir.0.join("library.json"), || 0).unwrap();
        assert_eq!(loaded.value, 2);
        writer.shutdown().unwrap();
        assert_eq!(
            saves.submit(Revision(3), Arc::new(3)),
            Err(PersistError::Stopped)
        );
    }

    #[test]
    fn failed_write_retries_on_timer_and_next_change() {
        struct Fault {
            fail: Arc<AtomicBool>,
            file: JsonFile<u64>,
        }
        impl WriteTarget<u64> for Fault {
            fn write(&self, value: &u64) -> Result<(), PersistError> {
                if self.fail.load(Ordering::SeqCst) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied,
                        "PRIVATE-CONTENT",
                    )
                    .into());
                }
                self.file.write(value)
            }
        }
        let dir = Folder::new();
        let clock = Arc::new(FakeClock::default());
        let fail = Arc::new(AtomicBool::new(true));
        let (file, _) = JsonFile::open(dir.0.join("settings.json"), || 0).unwrap();
        let writer = Writer::new(clock.clone(), Duration::ZERO, Duration::from_secs(10)).unwrap();
        let saves = writer
            .register(
                Fault {
                    fail: fail.clone(),
                    file,
                },
                Revision(0),
            )
            .unwrap();
        let watch = saves.saved();
        let read = watch.clone();
        let (tx, rx) = mpsc::channel();
        let _subscription = watch.subscribe(move || {
            tx.send(read.get()).unwrap();
        });
        saves.submit(Revision(1), Arc::new(7)).unwrap();
        let failed = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(failed.durable, Revision(0));
        assert!(failed.error.is_some());
        assert!(!format!("{:?}", failed.error).contains("PRIVATE"));
        fail.store(false, Ordering::SeqCst);
        clock.advance(Duration::from_secs(10));
        let saved = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!((saved.durable, saved.error), (Revision(1), None));
        fail.store(true, Ordering::SeqCst);
        saves.submit(Revision(2), Arc::new(8)).unwrap();
        assert!(rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .error
            .is_some());
        fail.store(false, Ordering::SeqCst);
        saves.submit(Revision(3), Arc::new(9)).unwrap();
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap().durable,
            Revision(3)
        );
        writer.shutdown().unwrap();
    }

    #[test]
    fn debounce_coalesces_and_shutdown_attempts_every_file_even_after_failure() {
        let dir = Folder::new();
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(
            clock.clone(),
            Duration::from_secs(30),
            Duration::from_secs(10),
        )
        .unwrap();
        struct Refuses;
        impl WriteTarget<u64> for Refuses {
            fn write(&self, _: &u64) -> Result<(), PersistError> {
                Err(PersistError::Encode)
            }
        }
        let bad = writer.register(Refuses, Revision(0)).unwrap();
        let (file, _) = JsonFile::open(dir.0.join("good.json"), || 0).unwrap();
        let good = writer.register(file, Revision(0)).unwrap();
        good.submit(Revision(1), Arc::new(10)).unwrap();
        good.submit(Revision(2), Arc::new(20)).unwrap();
        assert_eq!(good.saved().get().durable, Revision(0));
        assert!(!dir.0.join("good.json").exists());
        bad.submit(Revision(1), Arc::new(1)).unwrap();
        drop(good); // registrations survive owner drop until drain
        assert_eq!(writer.shutdown(), Err(PersistError::Encode));
        assert_eq!(bad.saved().get().error, Some(PersistError::Encode));
        let (_, loaded) = JsonFile::<u64>::open(dir.0.join("good.json"), || 0).unwrap();
        assert_eq!(loaded.value, 20);
        assert_eq!(writer.shutdown(), Err(PersistError::Encode));
    }

    #[test]
    fn errors_and_their_sources_discard_parser_serializer_and_io_contents() {
        use std::error::Error;
        let dir = Folder::new();
        #[derive(serde::Deserialize)]
        struct Sensitive;
        impl Serialize for Sensitive {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom("PRIVATE-CONTENT"))
            }
        }
        let (file, _) = JsonFile::open(dir.0.join("sensitive.json"), || Sensitive).unwrap();
        let errors = [
            file.write(&Sensitive).unwrap_err(),
            PersistError::from(std::io::Error::other("PRIVATE-CONTENT")),
            PersistError::Decode,
        ];
        for error in errors {
            let mut current: Option<&dyn Error> = Some(&error);
            while let Some(cause) = current {
                assert!(!format!("{cause} {cause:?}").contains("PRIVATE-CONTENT"));
                current = cause.source();
            }
        }
        assert!(!dir.0.join("sensitive.json").exists());
    }

    #[test]
    fn newer_format_is_preserved_without_quarantine_or_defaults() {
        let dir = Folder::new();
        let path = dir.0.join("library.json");
        fs::write(&path, b"future-format").unwrap();
        let result = JsonFile::<u64>::open_with(
            path.clone(),
            || panic!("defaults must not run"),
            |_| Err(PersistError::UnsupportedVersion),
            |value| serde_json::to_vec(value).map_err(|_| PersistError::Encode),
        );
        assert!(matches!(result, Err(PersistError::UnsupportedVersion)));
        assert_eq!(fs::read(path).unwrap(), b"future-format");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("kudzu-persist-{}", uuid::Uuid::new_v4()));
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
    fn atomic_replacement_and_corrupt_quarantine() {
        let dir = Folder::new();
        let path = dir.0.join("library.json");
        let (file, loaded) = JsonFile::<Vec<String>>::open(path.clone(), Vec::new).unwrap();
        assert_eq!(loaded.source, LoadSource::Missing);
        file.write(&vec!["old".into()]).unwrap();
        // A reader holding the old inode must see its entire old value after replacement.
        let mut old = fs::File::open(&path).unwrap();
        file.write(&vec!["new".into()]).unwrap();
        let mut bytes = String::new();
        use std::io::Read;
        old.read_to_string(&mut bytes).unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&bytes).unwrap(),
            ["old"]
        );
        let (_, loaded) = JsonFile::<Vec<String>>::open(path.clone(), Vec::new).unwrap();
        assert_eq!(loaded.value, ["new"]);
        fs::write(&path, b"PRIVATE-CONTENT invalid json").unwrap();
        let (recovered, loaded) = JsonFile::<Vec<String>>::open(path.clone(), Vec::new).unwrap();
        let LoadSource::Recovered(aside) = loaded.source else {
            panic!("not quarantined")
        };
        assert_eq!(fs::read(&aside).unwrap(), b"PRIVATE-CONTENT invalid json");
        assert!(!path.exists());
        recovered.write(&vec!["default".into()]).unwrap();
        assert_eq!(fs::read(&aside).unwrap(), b"PRIVATE-CONTENT invalid json");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 2);
    }

    #[test]
    fn concurrent_atomic_writes_use_unique_temporaries() {
        let dir = Folder::new();
        let (file, _) = JsonFile::<Vec<u8>>::open(dir.0.join("settings.json"), Vec::new).unwrap();
        let file = Arc::new(file);
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let threads: Vec<_> = (0..8)
            .map(|n| {
                let file = file.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    file.write(&vec![n; 100_000]).unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let (_, loaded) = JsonFile::<Vec<u8>>::open(dir.0.join("settings.json"), Vec::new).unwrap();
        assert_eq!(loaded.value.len(), 100_000);
        assert!(loaded.value.iter().all(|n| *n == loaded.value[0]));
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn failed_quarantine_never_overwrites_the_original() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Folder::new();
        let path = dir.0.join("form-memory.json");
        fs::write(&path, b"PRIVATE-CONTENT").unwrap();
        fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o500)).unwrap();
        let result = JsonFile::<Vec<String>>::open(path.clone(), Vec::new);
        fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(
            result.is_err(),
            "quarantine failure must refuse a writable file handle"
        );
        assert_eq!(fs::read(path).unwrap(), b"PRIVATE-CONTENT");
    }
}
