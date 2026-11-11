//! Bundle permissions and crash debris, with an OS-held lease for live staging folders.
use kudzu_app::{error::PortError, ports::InstallProbe};
use std::{
    ffi::{CString, OsString},
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

unsafe extern "C" {
    fn getuid() -> u32;
    fn access(path: *const std::ffi::c_char, mode: i32) -> i32;
}

pub(crate) struct Scratch(pub(crate) PathBuf, File, bool);
impl Scratch {
    pub(crate) fn beside(parent: &Path) -> Result<Self, PortError> {
        let path = parent.join(format!(".kudzu-update-{}", uuid::Uuid::new_v4()));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .map_err(super::port_io)?;
        let result = (|| {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path.join(".active"))?;
            lock.try_lock().map_err(io::Error::from)?;
            Ok::<_, io::Error>(lock)
        })();
        match result {
            Ok(lock) => Ok(Self(path, lock, false)),
            Err(error) => {
                let _ = fs::remove_dir_all(&path);
                Err(super::port_io(error))
            }
        }
    }
    pub(super) fn for_bundle(bundle: &Path) -> Result<Self, PortError> {
        let stage = Self::beside(bundle.parent().ok_or(PortError::Unavailable)?)?;
        let mut record = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(stage.0.join("bundle-path"))
            .map_err(super::port_io)?;
        record
            .write_all(bundle.as_os_str().as_bytes())
            .map_err(super::port_io)?;
        Ok(stage)
    }
    pub(super) fn preserve(&mut self) {
        self.2 = true;
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.2 {
            let _ = fs::remove_dir_all(&self.0);
        }
        // Keep the lease alive until removal finishes (including during concurrent probes).
        let _ = &self.1;
    }
}

pub(crate) fn probe(bundle: &Path) -> InstallProbe {
    probe_as(bundle, unsafe { getuid() })
}
fn probe_as(bundle: &Path, uid: u32) -> InstallProbe {
    sweep_as(bundle, uid);
    let canonical = fs::canonicalize(bundle).unwrap_or_else(|_| bundle.into());
    let unsafe_location = [bundle, canonical.as_path()].iter().any(|p| {
        p.starts_with("/Volumes") || p.components().any(|c| c.as_os_str() == "AppTranslocation")
    });
    let owned = fs::metadata(bundle).is_ok_and(|m| m.uid() == uid);
    let writable = CString::new(bundle.as_os_str().as_bytes())
        .is_ok_and(|p| unsafe { access(p.as_ptr(), 2) } == 0);
    InstallProbe {
        needs_move: unsafe_location
            || super::super::relaunch::validate_bundle(bundle).is_err()
            || !owned
            || !writable
            || bundle.parent().is_none_or(|p| {
                fs::metadata(p).map_or(true, |m| m.permissions().readonly())
                    || Scratch::beside(p).is_err()
            }),
    }
}

fn sweep_as(bundle: &Path, uid: u32) {
    let Some(parent) = bundle.parent() else {
        return;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".kudzu-update-")
        {
            continue;
        }
        let path = entry.path();
        if !fs::symlink_metadata(&path)
            .is_ok_and(|m| m.is_dir() && m.uid() == uid && m.mode() & 0o022 == 0)
        {
            continue;
        }
        let installed = (|| {
            let mut bytes = Vec::new();
            File::open(path.join("bundle-path"))
                .ok()?
                .take(4097)
                .read_to_end(&mut bytes)
                .ok()?;
            (bytes.len() <= 4096)
                .then(|| PathBuf::from(OsString::from_vec(bytes)))
                .filter(|p| p.is_absolute())
        })()
        .unwrap_or_else(|| bundle.into());
        if path.join("previous.app").exists() && !installed.exists() {
            continue;
        }
        let lease = path.join(".active");
        let lock = match fs::symlink_metadata(&lease) {
            Ok(m) if m.is_file() && m.uid() == uid && m.nlink() == 1 => {
                OpenOptions::new().read(true).write(true).open(&lease)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&lease),
            _ => continue,
        };
        let Ok(lock) = lock else {
            continue;
        };
        if lock.try_lock().is_ok() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    #[test]
    fn probe_requires_bundle_ownership_and_access_not_only_parent_writability() {
        let root = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Kudzu.app");
        fs::create_dir(&bundle).unwrap();
        let uid = unsafe { getuid() };
        assert!(!probe_as(&bundle, uid).needs_move);
        assert!(probe_as(&bundle, uid.wrapping_add(1)).needs_move);
        fs::set_permissions(&bundle, fs::Permissions::from_mode(0o500)).unwrap();
        assert!(probe(&bundle).needs_move);
        fs::set_permissions(&bundle, fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[test]
    fn sweeps_only_stale_owned_directories_and_preserves_the_only_backup_and_live_leases() {
        let root = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Kudzu.app");
        fs::create_dir(&bundle).unwrap();
        let stale = root.0.join(".kudzu-update-stale");
        fs::create_dir(&stale).unwrap();
        let active = Scratch::for_bundle(&bundle).unwrap();
        let outside = root.0.join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, root.0.join(".kudzu-update-symlink")).unwrap();
        let uid = unsafe { getuid() };
        sweep_as(&bundle, uid.wrapping_add(1));
        assert!(stale.exists());
        probe(&bundle);
        assert!(!stale.exists());
        assert!(active.0.exists());
        assert!(outside.exists());
        let recovery = root.0.join(".kudzu-update-recovery");
        fs::create_dir_all(recovery.join("previous.app")).unwrap();
        fs::remove_dir(&bundle).unwrap();
        probe(&bundle);
        assert!(recovery.join("previous.app").exists());
        fs::create_dir(&bundle).unwrap();
        probe(&bundle);
        assert!(!recovery.exists());
    }
}
