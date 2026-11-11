use super::*;
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) struct Store {
    pub dir: PathBuf,
    #[cfg(test)]
    pub before_write: std::sync::Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
    #[cfg(test)]
    pub before_items: std::sync::Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
}
impl Store {
    pub fn open(dir: PathBuf) -> Result<Self, ClipboardError> {
        for p in [
            &dir,
            &dir.join("items"),
            &dir.join("thumbs"),
            &dir.join("trash"),
        ] {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(p).map_err(IoFailure::from)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(p, fs::Permissions::from_mode(0o700))
                    .map_err(IoFailure::from)?;
            }
        }
        Ok(Self {
            dir,
            #[cfg(test)]
            before_write: std::sync::Mutex::new(None),
            #[cfg(test)]
            before_items: std::sync::Mutex::new(None),
        })
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
        #[cfg(test)]
        {
            let hook = self.before_items.lock().unwrap().clone();
            if let Some(hook) = hook {
                hook();
            }
        }
        clipboard::decode_items(&fs::read(self.blob(id)).ok()?)
    }
    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), ClipboardError> {
        #[cfg(test)]
        if let Some(hook) = self.before_write.lock().unwrap().clone() {
            hook();
        }
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
            let parent = path
                .parent()
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
            fs::File::open(parent)?.sync_all()
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
                let name = path.file_name().ok_or(IoFailure {
                    kind: std::io::ErrorKind::InvalidInput,
                })?;
                let target = self.dir.join("trash").join(name);
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
    pub fn clean(&self, history: &History) -> Result<(), ClipboardError> {
        // Retire old journals (which could contain secret excerpts) and abandoned root temporaries.
        for item in fs::read_dir(&self.dir).map_err(IoFailure::from)? {
            let item = item.map_err(IoFailure::from)?;
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == "cleanup.json"
                || (name.starts_with(".kudzu-") && name.ends_with(".tmp"))
                || name.contains(".tmp-")
            {
                fs::remove_file(item.path()).map_err(IoFailure::from)?;
            }
        }
        // A live index always wins over leftover trash, including an interrupted undo.
        self.trash(&history.entries, true)?;
        let live: HashSet<_> = history.entries.iter().map(|e| e.id.to_string()).collect();
        for sub in ["items", "thumbs", "trash"] {
            for e in fs::read_dir(self.dir.join(sub)).map_err(IoFailure::from)? {
                let e = e.map_err(IoFailure::from)?;
                let valid_extension = e.path().extension().is_some_and(|ext| {
                    (sub == "items" && ext == "kzc") || (sub == "thumbs" && ext == "png")
                });
                let keep = sub != "trash"
                    && valid_extension
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
