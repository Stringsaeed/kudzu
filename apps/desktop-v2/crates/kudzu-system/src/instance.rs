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
    /// An active advisory lock has no authenticated running Kudzu owner. The caller
    /// must report this condition and exit; never unlink/steal the lock or open a bundle.
    UnverifiedOwner,
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
        Self::acquire_with(directory, bundle_id, bundle, activate_owner)
    }

    fn acquire_with(
        directory: &Path,
        bundle_id: &str,
        bundle: Option<&Path>,
        activate: impl FnOnce(u32) -> Result<(), PortError>,
    ) -> Result<Self, PortError> {
        Self::acquire_using(directory, bundle_id, bundle, authorize_owner, activate)
    }

    fn acquire_using(
        directory: &Path,
        bundle_id: &str,
        bundle: Option<&Path>,
        authorize: impl Fn(&OwnerRecord, &str) -> Result<(), PortError>,
        activate: impl FnOnce(u32) -> Result<(), PortError>,
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
            let record = OwnerRecord {
                pid: std::process::id(),
                bundle: bundle.unwrap_or_else(|| Path::new("")).into(),
            };
            owner
                ._lock
                .write_all(&record.encode())
                .map_err(|e| PortError::Io(e.into()))?;
            return Ok(Self::Primary(owner));
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::WouldBlock {
            return Err(PortError::Io(error.into()));
        }
        let mut bytes = Vec::new();
        let record = lock
            .take(4105)
            .read_to_end(&mut bytes)
            .ok()
            .and_then(|_| OwnerRecord::decode(&bytes));
        let Some(record) = record.filter(|record| authorize(record, bundle_id).is_ok()) else {
            return Ok(Self::UnverifiedOwner);
        };
        if !bundle.is_some_and(|_| activate(record.pid).is_ok()) {
            return Ok(Self::UnverifiedOwner);
        }
        Ok(Self::Secondary)
    }
}

struct OwnerRecord {
    pid: u32,
    bundle: PathBuf,
}
impl OwnerRecord {
    fn encode(&self) -> Vec<u8> {
        let mut bytes = b"KZI1".to_vec();
        bytes.extend(self.pid.to_be_bytes());
        bytes.extend(self.bundle.as_os_str().as_bytes());
        bytes
    }
    fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 9 || bytes.len() > 4104 || &bytes[..4] != b"KZI1" {
            return None;
        }
        let pid = u32::from_be_bytes(bytes[4..8].try_into().ok()?);
        if pid == 0 || pid > i32::MAX as u32 {
            return None;
        }
        Some(Self {
            pid,
            bundle: PathBuf::from(std::ffi::OsString::from_vec(bytes[8..].to_vec())),
        })
    }
}

fn authorize_owner(record: &OwnerRecord, id: &str) -> Result<(), PortError> {
    authorize_with(
        record,
        id,
        super::update_install::verify_signature,
        running_bundle,
        crate::code_identity::verify_process,
    )
}

fn authorize_with(
    record: &OwnerRecord,
    id: &str,
    verify_bundle: impl Fn(&Path, &str) -> Result<(), PortError>,
    lookup: impl Fn(u32) -> Option<PathBuf>,
    verify_process: impl Fn(u32, &str) -> Result<(), PortError>,
) -> Result<(), PortError> {
    let canonical = std::fs::canonicalize(&record.bundle).map_err(|e| PortError::Io(e.into()))?;
    super::update_install::verified_bundle_version(&canonical, id, |path, id, _version| {
        verify_bundle(path, id)
    })?;
    let actual = lookup(record.pid).ok_or(PortError::Unavailable)?;
    if std::fs::canonicalize(actual).map_err(|e| PortError::Io(e.into()))? != canonical {
        return Err(PortError::InvalidData);
    }
    verify_process(record.pid, id)
}

#[cfg(target_os = "macos")]
fn running_bundle(pid: u32) -> Option<PathBuf> {
    objc2::rc::autoreleasepool(|_| {
        let app = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(
            pid.try_into().ok()?,
        )?;
        Some(PathBuf::from(app.bundleURL()?.path()?.to_string()))
    })
}
#[cfg(not(target_os = "macos"))]
fn running_bundle(_: u32) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn activate_owner(pid: u32) -> Result<(), PortError> {
    objc2::rc::autoreleasepool(|_| {
        let app = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(
            pid.try_into().map_err(|_| PortError::InvalidData)?,
        )
        .ok_or(PortError::Unavailable)?;
        if app
            .activateWithOptions(objc2_app_kit::NSApplicationActivationOptions::ActivateAllWindows)
        {
            Ok(())
        } else {
            Err(PortError::Unavailable)
        }
    })
}
#[cfg(not(target_os = "macos"))]
fn activate_owner(_: u32) -> Result<(), PortError> {
    Err(PortError::Unavailable)
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
        let second = Instance::acquire_with(&dir, "sh.saeed.kudzu.next", Some(bundle), |_| {
            panic!("unverified owner must not open")
        })
        .unwrap();
        assert!(matches!(second, Instance::UnverifiedOwner));
        assert!(matches!(
            Instance::acquire_with(&dir, "sh.saeed.kudzu.next", None, |_| panic!("unbundled"))
                .unwrap(),
            Instance::UnverifiedOwner
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
            Instance::UnverifiedOwner
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
            Instance::acquire_using(
                &dir,
                "sh.saeed.kudzu.next",
                Some(&second),
                |record, id| {
                    assert_eq!(record.pid, std::process::id());
                    assert_eq!(record.bundle, running);
                    assert_eq!(id, "sh.saeed.kudzu.next");
                    Ok(())
                },
                |pid| {
                    assert_eq!(pid, std::process::id());
                    Ok(())
                }
            )
            .unwrap(),
            Instance::Secondary
        ));
        for record in [b"/missing/Kudzu.app".to_vec(), vec![b'x'; 4097], Vec::new()] {
            std::fs::write(dir.join("sh.saeed.kudzu.next.lock"), record).unwrap();
            assert!(matches!(
                Instance::acquire_with(&dir, "sh.saeed.kudzu.next", Some(&second), |_| panic!(
                    "invalid record must not reopen any app"
                ))
                .unwrap(),
                Instance::UnverifiedOwner
            ));
        }
        assert!(matches!(
            Instance::acquire_with(&dir, "sh.saeed.kudzu.next", None, |_| panic!(
                "unbundled launch must exit"
            ))
            .unwrap(),
            Instance::UnverifiedOwner
        ));
        drop(owner);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn untrusted_owner_record_never_opens_any_bundle() {
        let root = crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
        let evil = root.0.join("Evil.app");
        let own = root.0.join("Kudzu.app");
        std::fs::create_dir(&evil).unwrap();
        std::fs::create_dir(&own).unwrap();
        let owner =
            Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", None, |_| panic!("primary"))
                .unwrap();
        std::fs::write(
            root.0.join("sh.saeed.kudzu.next.lock"),
            evil.as_os_str().as_bytes(),
        )
        .unwrap();
        let opened = std::cell::Cell::new(false);
        let _ = Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", Some(&own), |_| {
            opened.set(true);
            Ok(())
        })
        .unwrap();
        assert!(
            !opened.get(),
            "an advisory owner record must never authorize open"
        );
        drop(owner);
    }

    #[test]
    fn empty_owner_record_never_reopens_the_losing_bundle() {
        let root = crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
        let own = root.0.join("Kudzu.app");
        std::fs::create_dir(&own).unwrap();
        let owner =
            Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", None, |_| panic!("primary"))
                .unwrap();
        let opened = std::cell::Cell::new(false);
        let _ = Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", Some(&own), |_| {
            opened.set(true);
            Ok(())
        })
        .unwrap();
        assert!(
            !opened.get(),
            "an unbundled primary must not cause a relaunch loop"
        );
        drop(owner);
    }

    #[test]
    fn wrong_id_unsigned_bundle_and_forged_process_cannot_authorize_reopen() {
        let root = crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
        let target = root.0.join("Kudzu.app");
        std::fs::create_dir_all(target.join("Contents")).unwrap();
        let owner =
            Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", None, |_| panic!("primary"))
                .unwrap();
        let record = OwnerRecord {
            pid: std::process::id(),
            bundle: target.clone(),
        };
        std::fs::write(root.0.join("sh.saeed.kudzu.next.lock"), record.encode()).unwrap();
        for id in ["wrong.identity", "sh.saeed.kudzu.next"] {
            let mut info = plist::Dictionary::new();
            info.insert("CFBundleIdentifier".into(), id.into());
            info.insert("CFBundleShortVersionString".into(), "1.0.0".into());
            plist::Value::Dictionary(info)
                .to_file_xml(target.join("Contents/Info.plist"))
                .unwrap();
            assert!(matches!(
                Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", Some(&target), |_| panic!(
                    "untrusted bundle must not open"
                ))
                .unwrap(),
                Instance::UnverifiedOwner
            ));
        }
        let authorize = |record: &OwnerRecord, id: &str| {
            authorize_with(
                record,
                id,
                |_, _| Ok(()),
                |_| Some(target.clone()),
                |_, _| Err(PortError::InvalidData),
            )
        };
        assert!(matches!(
            Instance::acquire_using(
                &root.0,
                "sh.saeed.kudzu.next",
                Some(&target),
                authorize,
                |_| panic!("forged process must not open")
            )
            .unwrap(),
            Instance::UnverifiedOwner
        ));
        drop(owner);
        assert!(matches!(
            Instance::acquire(&root.0, "sh.saeed.kudzu.next", None).unwrap(),
            Instance::Primary(_)
        ));
    }

    #[test]
    fn second_launch_activates_verified_pid_without_opening_recorded_path() {
        let root = crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
        let running = root.0.join("Installed Kudzu.app");
        let evil = root.0.join("Evil.app");
        let second = root.0.join("Second Kudzu.app");
        let link = root.0.join("Link.app");
        std::fs::create_dir(&running).unwrap();
        std::fs::create_dir(&evil).unwrap();
        std::fs::create_dir(&second).unwrap();
        std::os::unix::fs::symlink(&running, &link).unwrap();

        let owner = Instance::acquire_with(&root.0, "sh.saeed.kudzu.next", Some(&link), |_| {
            panic!("primary")
        })
        .unwrap();
        let expected_pid = std::process::id();
        let activated = std::cell::Cell::new(None);
        let outcome = Instance::acquire_using(
            &root.0,
            "sh.saeed.kudzu.next",
            Some(&second),
            |record, id| {
                assert_eq!(record.pid, expected_pid);
                assert_eq!(id, "sh.saeed.kudzu.next");
                // Simulate an attacker flipping the recorded symlink after authentication passes.
                std::fs::remove_file(&link).unwrap();
                std::os::unix::fs::symlink(&evil, &link).unwrap();
                Ok(())
            },
            |pid| {
                activated.set(Some(pid));
                Ok(())
            },
        )
        .unwrap();
        assert!(matches!(outcome, Instance::Secondary));
        assert_eq!(activated.get(), Some(expected_pid));

        let unactivatable = Instance::acquire_using(
            &root.0,
            "sh.saeed.kudzu.next",
            Some(&second),
            |_, _| Ok(()),
            |_| Err(PortError::Unavailable),
        )
        .unwrap();
        assert!(matches!(unactivatable, Instance::UnverifiedOwner));
        assert_eq!(activate_owner(i32::MAX as u32), Err(PortError::Unavailable));
        assert_eq!(activate_owner(u32::MAX), Err(PortError::InvalidData));
        drop(owner);
    }
}
