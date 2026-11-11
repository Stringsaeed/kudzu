use crate::error::IoFailure;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub struct Staged {
    pub path: PathBuf,
    pub digest: [u8; 32],
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Write `bytes` next to the running bundle (`dir`) with mode 0600 and a unique name.
pub fn archive(dir: &Path, bytes: &[u8]) -> Result<Staged, IoFailure> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("kudzu-update-{}.tar.gz", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        let _ = std::fs::remove_file(&path);
        return Err(error.into());
    }
    Ok(Staged {
        path,
        digest: *blake3::hash(bytes).as_bytes(),
    })
}
