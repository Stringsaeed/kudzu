//! An OS-held flock, not a sentinel. Never unlink the lock file: waiters share its inode.
use kudzu_app::error::PortError;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{File, OpenOptions},
    path::Path,
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
            return Ok(Self::Primary(InstanceGuard { _lock: lock }));
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err(PortError::Io(error.into()));
        }
        if let Some(bundle) = bundle {
            let _ = reopen(bundle);
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
}
