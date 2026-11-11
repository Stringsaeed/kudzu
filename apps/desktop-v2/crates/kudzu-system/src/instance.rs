//! An OS-held flock, not a sentinel. Never unlink the lock file: waiters share its inode.
use kudzu_app::error::PortError;
use std::os::fd::AsRawFd;
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    fs::OpenOptionsExt,
};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub enum Instance {
    Primary(InstanceGuard),
    /// The caller must exit, whether Launch Services accepted the reopen or not.
    Secondary,
}

pub struct InstanceGuard {
    _lock: File,
}

unsafe extern "C" {
    fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        // Closing alone can leave a fork-inherited open-file description holding the lock.
        // Explicit LOCK_UN releases ownership immediately, before the child's CLOEXEC.
        while unsafe { flock(self._lock.as_raw_fd(), 8) } != 0 {
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                break;
            }
        }
    }
}

impl Instance {
    /// `directory` is the resolved identity's private data directory. Hold Primary until exit.
    /// S40 must acquire this lock before starting NSApplication, so a losing copy never
    /// registers itself with Launch Services or accepts reopen events intended for the owner.
    pub fn acquire(
        directory: &Path,
        bundle_id: &str,
        bundle: Option<&Path>,
    ) -> Result<Self, PortError> {
        Self::acquire_with(directory, bundle_id, bundle, super::relaunch::reopen)
    }

    fn acquire_with(
        directory: &Path,
        bundle_id: &str,
        bundle: Option<&Path>,
        reopen: impl FnOnce(&Path) -> Result<(), PortError>,
    ) -> Result<Self, PortError> {
        if !super::login_item::valid_id(bundle_id) {
            return Err(PortError::InvalidData);
        }
        std::fs::create_dir_all(directory).map_err(|e| PortError::Io(e.into()))?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(directory.join(format!("{bundle_id}.lock")))
            .map_err(|e| PortError::Io(e.into()))?;
        // LOCK_EX | LOCK_NB, on both macOS and the Unix test hosts.
        if unsafe { flock(lock.as_raw_fd(), 2 | 4) } == 0 {
            let mut owner = InstanceGuard { _lock: lock };
            owner
                ._lock
                .set_len(0)
                .map_err(|e| PortError::Io(e.into()))?;
            if let Some(bundle) = bundle {
                owner
                    ._lock
                    .write_all(bundle.as_os_str().as_bytes())
                    .map_err(|e| PortError::Io(e.into()))?;
            }
            return Ok(Self::Primary(owner));
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err(PortError::Io(error.into()));
        }
        if let Some(own_bundle) = bundle {
            let mut record = Vec::new();
            let running = lock
                .take(4097)
                .read_to_end(&mut record)
                .ok()
                .filter(|_| record.len() <= 4096)
                .map(|_| PathBuf::from(std::ffi::OsString::from_vec(record)))
                .filter(|p| super::relaunch::validate_bundle(p).is_ok());
            let _ = reopen(running.as_deref().unwrap_or(own_bundle));
        }
        Ok(Self::Secondary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };

    #[test]
    fn a_second_launch_reopens_only_when_bundled_and_identities_are_independent() {
        let dir = std::env::temp_dir().join(format!("kudzu-lock-{}", uuid::Uuid::new_v4()));
        let first = Instance::acquire_with(&dir, "sh.saeed.kudzu.next", None, |_| {
            panic!("primary must not reopen")
        })
        .unwrap();
        assert!(matches!(first, Instance::Primary(_)));
        let bundle = Path::new("/fixture/Kudzu Next.app");
        let second = Instance::acquire_with(&dir, "sh.saeed.kudzu.next", Some(bundle), |seen| {
            assert_eq!(seen, bundle);
            Err(PortError::Unavailable)
        })
        .unwrap();
        assert!(matches!(second, Instance::Secondary));
        assert!(matches!(
            Instance::acquire_with(&dir, "sh.saeed.kudzu.next", None, |_| panic!("unbundled"))
                .unwrap(),
            Instance::Secondary
        ));
        assert!(matches!(
            Instance::acquire_with(&dir, "sh.saeed.kudzu", None, |_| panic!("other identity"))
                .unwrap(),
            Instance::Primary(_)
        ));
        drop(first);
        assert!(matches!(
            Instance::acquire(&dir, "sh.saeed.kudzu.next", None).unwrap(),
            Instance::Primary(_)
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn dropping_the_owner_unlocks_even_while_an_inherited_descriptor_is_open() {
        let dir =
            std::env::temp_dir().join(format!("kudzu-inherited-lock-{}", uuid::Uuid::new_v4()));
        let Instance::Primary(owner) =
            Instance::acquire(&dir, "sh.saeed.kudzu.next", None).unwrap()
        else {
            panic!("first owner");
        };
        // fork temporarily retains the same open-file description until exec's CLOEXEC.
        let inherited = owner._lock.try_clone().unwrap();
        drop(owner);
        assert!(matches!(
            Instance::acquire(&dir, "sh.saeed.kudzu.next", None).unwrap(),
            Instance::Primary(_)
        ));
        drop(inherited);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn crash_releases_the_os_lock_without_removing_the_file() {
        let dir = std::env::temp_dir().join(format!("kudzu-crash-{}", uuid::Uuid::new_v4()));
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "instance::tests::crash_child",
                "--ignored",
                "--nocapture",
            ])
            .env("KUDZU_LOCK_TEST_DIR", &dir)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if line.contains("LOCK_READY") {
                break;
            }
            line.clear();
        }
        assert!(matches!(
            Instance::acquire(&dir, "sh.saeed.kudzu.next", None).unwrap(),
            Instance::Secondary
        ));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(dir.join("sh.saeed.kudzu.next.lock").exists());
        assert!(matches!(
            Instance::acquire(&dir, "sh.saeed.kudzu.next", None).unwrap(),
            Instance::Primary(_)
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "only invoked in an isolated crash-test process"]
    fn crash_child() {
        let dir = std::env::var_os("KUDZU_LOCK_TEST_DIR").expect("isolated child only");
        let _guard = Instance::acquire(Path::new(&dir), "sh.saeed.kudzu.next", None).unwrap();
        println!("LOCK_READY");
        std::io::stdout().flush().unwrap();
        loop {
            std::thread::park();
        }
    }

    #[test]
    fn review_second_copy_reopens_the_running_bundle_elsewhere() {
        let dir = std::env::temp_dir().join(format!("kudzu-two-copies-{}", uuid::Uuid::new_v4()));
        let running = dir.join("installed/Kudzu.app");
        let second = dir.join("download/Kudzu.app");
        std::fs::create_dir_all(&running).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let owner = Instance::acquire_with(&dir, "sh.saeed.kudzu.next", Some(&running), |_| {
            panic!("primary")
        })
        .unwrap();
        assert!(matches!(
            Instance::acquire_with(&dir, "sh.saeed.kudzu.next", Some(&second), |seen| {
                assert_eq!(seen, running);
                Ok(())
            })
            .unwrap(),
            Instance::Secondary
        ));
        for record in [b"/missing/Kudzu.app".to_vec(), vec![b'x'; 4097], Vec::new()] {
            std::fs::write(dir.join("sh.saeed.kudzu.next.lock"), record).unwrap();
            assert!(matches!(
                Instance::acquire_with(&dir, "sh.saeed.kudzu.next", Some(&second), |seen| {
                    assert_eq!(seen, second);
                    Ok(())
                })
                .unwrap(),
                Instance::Secondary
            ));
        }
        assert!(matches!(
            Instance::acquire_with(&dir, "sh.saeed.kudzu.next", None, |_| panic!(
                "unbundled launch must exit"
            ))
            .unwrap(),
            Instance::Secondary
        ));
        drop(owner);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
