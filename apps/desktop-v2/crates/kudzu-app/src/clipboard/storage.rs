use super::*;
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(serde::Serialize, serde::Deserialize)]
struct JournalJob {
    entries: Vec<ClipEntry>,
    trash: bool,
}
pub(super) struct Store {
    pub dir: PathBuf,
}
impl Store {
    pub fn open(dir: PathBuf) -> Result<Self, ClipboardError> {
        for p in [
            &dir,
            &dir.join("items"),
            &dir.join("thumbs"),
            &dir.join("trash"),
        ] {
            fs::create_dir_all(p).map_err(IoFailure::from)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(p, fs::Permissions::from_mode(0o700))
                    .map_err(IoFailure::from)?;
            }
        }
        Ok(Self { dir })
    }
    fn path(&self, sub: &str, id: Uuid, ext: &str) -> PathBuf {
        self.dir.join(sub).join(format!("{id}.{ext}"))
    }
    pub fn blob(&self, id: Uuid) -> PathBuf {
        self.path("items", id, "kzc")
    }
    pub fn thumb(&self, id: Uuid) -> PathBuf {
        self.path("thumbs", id, "png")
    }
    pub fn items(&self, id: Uuid) -> Option<Vec<PasteItem>> {
        clipboard::decode_items(&fs::read(self.blob(id)).ok()?)
    }
    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), ClipboardError> {
        let tmp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
        let result = (|| {
            let mut options = fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::rename(&tmp, path)?;
            fs::File::open(path.parent().unwrap())?.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(tmp);
        }
        result.map_err(IoFailure::from)?;
        Ok(())
    }
    pub fn trash(&self, entries: &[ClipEntry], restore: bool) -> Result<(), ClipboardError> {
        for e in entries {
            for path in [self.blob(e.id), self.thumb(e.id)] {
                let target = self.dir.join("trash").join(path.file_name().unwrap());
                let (from, to) = if restore {
                    (&target, &path)
                } else {
                    (&path, &target)
                };
                match fs::rename(from, to) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound && !from.exists() => {}
                    Err(e) => return Err(IoFailure::from(e).into()),
                }
            }
        }
        Ok(())
    }
    pub fn purge(&self, entries: &[ClipEntry]) -> Result<(), ClipboardError> {
        for e in entries {
            for p in [self.blob(e.id), self.thumb(e.id)] {
                match fs::remove_file(p) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(IoFailure::from(e).into()),
                }
            }
        }
        Ok(())
    }
    pub fn write_jobs(&self, jobs: &[Cleanup]) -> Result<(), ClipboardError> {
        let jobs: Vec<_> = jobs
            .iter()
            .map(|job| JournalJob {
                entries: job.entries.clone(),
                trash: job.trash,
            })
            .collect();
        let bytes = serde_json::to_vec(&jobs)
            .map_err(|_| ClipboardError::Persistence(PersistError::Encode))?;
        self.write(&self.dir.join("cleanup.json"), &bytes)
    }
    /// Replay unfinished cleanup against the durable index before removing orphans.
    pub fn recover_jobs(&self, history: &History) -> Result<(), ClipboardError> {
        let path = self.dir.join("cleanup.json");
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(IoFailure::from(e).into()),
        };
        let jobs: Vec<JournalJob> = serde_json::from_slice(&bytes)
            .map_err(|_| ClipboardError::Persistence(PersistError::Decode))?;
        for job in jobs {
            let (live, removed): (Vec<_>, Vec<_>) = job
                .entries
                .into_iter()
                .partition(|entry| history.get(entry.id).is_some());
            self.trash(&live, true)?;
            if job.trash {
                self.trash(&removed, false)?;
            } else {
                self.purge(&removed)?;
            }
        }
        self.write_jobs(&[])
    }
    pub fn clean(&self, history: &History) -> Result<(), ClipboardError> {
        // A live index always wins over leftover trash, including an interrupted undo.
        self.trash(&history.entries, true)?;
        let live: HashSet<_> = history.entries.iter().map(|e| e.id.to_string()).collect();
        for sub in ["items", "thumbs", "trash"] {
            for e in fs::read_dir(self.dir.join(sub)).map_err(IoFailure::from)? {
                let e = e.map_err(IoFailure::from)?;
                let keep = sub != "trash"
                    && e.path()
                        .file_stem()
                        .is_some_and(|s| live.contains(&s.to_string_lossy().into_owned()));
                if !keep {
                    fs::remove_file(e.path()).map_err(IoFailure::from)?;
                }
            }
        }
        Ok(())
    }
    pub fn usage(&self) -> u64 {
        ["items", "thumbs"]
            .iter()
            .filter_map(|sub| fs::read_dir(self.dir.join(sub)).ok())
            .flatten()
            .flatten()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }
}
