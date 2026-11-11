//! Atomic JSON storage and a shared writer. Memory acceptance and disk durability are separate.

use crate::error::IoFailure;
use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub use crate::watch::Revision;
mod temporary;
mod writer;
pub use writer::{Persisted, Writer};

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
    #[error("The save operation stopped unexpectedly. Kudzu will retry.")]
    Panicked,
    #[error("Saving took too long. Some changes may not be on disk yet.")]
    Timeout,
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
        temporary::sweep(&path)?;
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
        let temporary = temporary::Temporary::reserve(parent)?;
        let temp = temporary.path();
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(temp, &self.path)?;
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
#[cfg(test)]
mod review_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::clock::FakeClock;
    use std::sync::mpsc;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
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
