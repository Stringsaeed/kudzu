use super::*;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

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
            fs::write(&tmp, bytes)?;
            fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(tmp);
        }
        result.map_err(IoFailure::from)?;
        Ok(())
    }
    pub fn trash(&self, entries: &[ClipEntry], restore: bool) {
        for e in entries {
            for path in [self.blob(e.id), self.thumb(e.id)] {
                let target = self.dir.join("trash").join(path.file_name().unwrap());
                let (from, to) = if restore {
                    (&target, &path)
                } else {
                    (&path, &target)
                };
                if from.exists() {
                    let _ = fs::rename(from, to);
                }
            }
        }
    }
    pub fn purge(&self, entries: &[ClipEntry]) {
        for e in entries {
            for p in [self.blob(e.id), self.thumb(e.id)] {
                let _ = fs::remove_file(p);
            }
        }
    }
    pub fn clean(&self, history: &History) {
        if let Ok(entries) = fs::read_dir(self.dir.join("trash")) {
            for e in entries.flatten() {
                let _ = fs::remove_file(e.path());
            }
        }
        let live: HashSet<_> = history.entries.iter().map(|e| e.id.to_string()).collect();
        for sub in ["items", "thumbs"] {
            if let Ok(entries) = fs::read_dir(self.dir.join(sub)) {
                for e in entries.flatten() {
                    if !e
                        .path()
                        .file_stem()
                        .is_some_and(|s| live.contains(&s.to_string_lossy().into_owned()))
                    {
                        let _ = fs::remove_file(e.path());
                    }
                }
            }
        }
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
