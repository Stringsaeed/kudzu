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
        // Release explicitly even if an in-flight fork inherited the open-file description.
        let _ = self.1.unlock();
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
        if lock.try_lock().is_err() {
            continue;
        }
        let previous = path.join("previous.app");
        if previous.exists() && !installed.exists() {
            // Recover only an app in this parent, never an arbitrary target from the
            // advisory record. Preserve the sole backup if any recovery check fails.
            if installed.parent() != Some(parent)
                || installed.extension().is_none_or(|e| e != "app")
                || !fs::symlink_metadata(&previous).is_ok_and(|m| m.is_dir() && m.uid() == uid)
                || fs::rename(&previous, &installed).is_err()
            {
                continue;
            }
        }
        let _ = fs::remove_dir_all(path);
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
        assert!(bundle.is_dir());
        assert!(!recovery.exists());
    }

    #[test]
    fn probe_recovers_a_missing_bundle_from_the_interrupted_legacy_backup() {
        let root = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Kudzu.app");
        fs::create_dir(&bundle).unwrap();
        fs::write(bundle.join("old"), b"complete previous app").unwrap();
        let mut stage = Scratch::for_bundle(&bundle).unwrap();
        let path = stage.0.clone();
        fs::rename(&bundle, stage.0.join("previous.app")).unwrap();
        let inherited = stage.1.try_clone().unwrap();
        stage.preserve();
        drop(stage);
        assert!(!probe(&bundle).needs_move);
        assert_eq!(
            fs::read(bundle.join("old")).unwrap(),
            b"complete previous app"
        );
        assert!(!path.exists());
        drop(inherited);
    }

    #[test]
    fn probe_recovers_the_legacy_backup_after_sigkill_skips_drop() {
        use std::{
            io::{BufRead, BufReader},
            process::{Command, Stdio},
        };
        let root = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Kudzu.app");
        fs::create_dir(&bundle).unwrap();
        fs::write(bundle.join("old"), b"old complete app").unwrap();
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "update_install::location::tests::legacy_backup_crash_child",
                    "--ignored",
                    "--nocapture",
                ])
                .env("KUDZU_LEGACY_RECOVERY_BUNDLE", &bundle)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let mut output = BufReader::new(child.0.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if line.contains("LEGACY_GAP") {
                break;
            }
            line.clear();
        }
        assert!(
            probe(&bundle).needs_move,
            "live stage must not be recovered"
        );
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        assert!(!probe(&bundle).needs_move);
        assert_eq!(fs::read(bundle.join("old")).unwrap(), b"old complete app");
        assert_eq!(
            fs::read_dir(&root.0).unwrap().count(),
            2,
            "only root lease and restored app remain"
        );
    }

    #[test]
    #[ignore = "invoked only by the temporary legacy-recovery crash parent"]
    fn legacy_backup_crash_child() {
        let bundle = PathBuf::from(
            std::env::var_os("KUDZU_LEGACY_RECOVERY_BUNDLE").expect("isolated child only"),
        );
        assert!(bundle.starts_with(std::env::temp_dir()));
        let stage = Scratch::for_bundle(&bundle).unwrap();
        // Replay the previous implementation's unsafe first rename; new installs never do it.
        fs::rename(&bundle, stage.0.join("previous.app")).unwrap();
        println!("LEGACY_GAP");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }
}
