//! Bind privately, then publish the already-restricted socket without following symlinks.
use std::{
    fs, io,
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) struct Endpoint {
    pub wake: PathBuf,
    path: PathBuf,
    private: PathBuf,
    identity: (u64, u64),
}
impl Endpoint {
    pub fn bind(path: PathBuf) -> io::Result<(UnixListener, Self)> {
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.file_type().is_socket() {
                return Err(io::ErrorKind::AddrInUse.into());
            }
            for attempt in 0..2 {
                match UnixStream::connect(&path) {
                    Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {}
                    _ => return Err(io::ErrorKind::AddrInUse.into()),
                }
                if attempt == 0 {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
            // Preserve an endpoint replaced while we were probing the stale one.
            let current = fs::symlink_metadata(&path)?;
            if (current.dev(), current.ino()) != (meta.dev(), meta.ino()) {
                return Err(io::ErrorKind::AddrInUse.into());
            }
            fs::remove_file(&path)?;
        }
        let parent = path.parent().unwrap_or(Path::new("."));
        let private = parent.join(format!(
            ".a{}",
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        ));
        fs::DirBuilder::new().mode(0o700).create(&private)?;
        let wake = private.join("s");
        let result = (|| {
            let listener = UnixListener::bind(&wake)?;
            // lchmod is the no-follow operation. The path is inside our newly created 0700 directory.
            nofollow_mode(&wake)?;
            let meta = fs::symlink_metadata(&wake)?;
            fs::hard_link(&wake, &path)?;
            Ok((
                listener,
                Self {
                    wake: wake.clone(),
                    path,
                    private: private.clone(),
                    identity: (meta.dev(), meta.ino()),
                },
            ))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&wake);
            let _ = fs::remove_dir(&private);
        }
        result
    }
    pub fn unpublish(&self) {
        if fs::symlink_metadata(&self.path).is_ok_and(|m| (m.dev(), m.ino()) == self.identity) {
            let _ = fs::remove_file(&self.path);
        }
    }
    pub fn wake(&self) {
        let _ = UnixStream::connect(&self.wake);
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        self.unpublish();
        let _ = fs::remove_file(&self.wake);
        let _ = fs::remove_dir(&self.private);
    }
}
fn nofollow_mode(path: &Path) -> io::Result<()> {
    use std::{
        ffi::{c_char, c_int, CString},
        os::unix::ffi::OsStrExt,
    };
    #[cfg(target_os = "macos")]
    type Mode = u16;
    #[cfg(not(target_os = "macos"))]
    type Mode = u32;
    unsafe extern "C" {
        fn lchmod(path: *const c_char, mode: Mode) -> c_int;
    }
    let path =
        CString::new(path.as_os_str().as_bytes()).map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: the CString is NUL-terminated and lives throughout the syscall. lchmod never follows a symlink.
    if unsafe { lchmod(path.as_ptr(), 0o600) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
