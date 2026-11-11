//! Startup cleanup must not remove another store's in-flight write in the same directory.
//! Composition must hold the single-instance guard before opening any product stores.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

static ACTIVE: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub(super) struct Temporary(PathBuf);
impl Temporary {
    pub(super) fn reserve(parent: &Path) -> std::io::Result<Self> {
        let path = fs::canonicalize(parent)?.join(format!(".kudzu-{}.tmp", uuid::Uuid::new_v4()));
        ACTIVE
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(path.clone());
        Ok(Self(path))
    }
    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let mut active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
        let _ = fs::remove_file(&self.0);
        active.retain(|path| path != &self.0);
    }
}

pub(super) fn sweep(path: &Path) -> std::io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = match fs::canonicalize(parent) {
        Ok(parent) => parent,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let active = ACTIVE.lock().unwrap_or_else(PoisonError::into_inner);
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".kudzu-")
            && name.ends_with(".tmp")
            && entry.file_type()?.is_file()
            && !active.contains(&entry.path())
        {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
