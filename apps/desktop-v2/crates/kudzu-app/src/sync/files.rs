//! `sync/index.json` (compact, the golden shape) and `sync/engine-state.data`.
//! The sync thread is the only writer. A failed checkpoint leaves the previous bytes.

use super::SyncError;
use crate::{
    error::IoFailure,
    persist::{JsonFile, PersistError, WriteTarget},
};
use kudzu_core::sync::SyncIndex;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub(super) struct SyncFiles {
    index: JsonFile<SyncIndex>,
    pub(super) checkpoint: PathBuf,
}

impl SyncFiles {
    pub(super) fn open(directory: &Path) -> Result<(Self, SyncIndex), SyncError> {
        fs::create_dir_all(directory).map_err(|error| SyncError::Storage(error.into()))?;
        let (index, loaded) = JsonFile::open_with(
            directory.join("index.json"),
            SyncIndex::default,
            |bytes| serde_json::from_slice(bytes).map_err(|_| PersistError::Decode),
            |value| serde_json::to_vec(value).map_err(|_| PersistError::Encode),
        )
        .map_err(SyncError::Index)?;
        Ok((
            Self {
                index,
                checkpoint: directory.join("engine-state.data"),
            },
            loaded.value,
        ))
    }

    pub(super) fn write_index(&self, index: &SyncIndex) -> Result<(), PersistError> {
        self.index.write(index)
    }
}

pub(super) fn read_checkpoint(path: &Path) -> Result<Option<Vec<u8>>, IoFailure> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn remove_checkpoint(path: &Path) -> Result<(), IoFailure> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Writes only when the bytes differ. Empty bytes with no file is a no-op.
pub(super) fn write_checkpoint(path: &Path, bytes: &[u8]) -> Result<(), IoFailure> {
    match fs::read(path) {
        Ok(existing) if existing == bytes => return Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound && bytes.is_empty() => return Ok(()),
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".engine-{}.tmp", uuid::Uuid::new_v4()));
    let write = write_temp(&temporary, parent, path, bytes);
    if write.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write
}

fn write_temp(temporary: &Path, parent: &Path, path: &Path, bytes: &[u8]) -> Result<(), IoFailure> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
