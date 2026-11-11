//! Atomic publication. Only ENOTSUP permits the legacy, non-atomic fallback.
use super::location::Scratch;
use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};

pub(super) const ENOTSUP: i32 = 45;

#[cfg(target_os = "macos")]
pub(super) fn exchange(from: &Path, to: &Path) -> io::Result<()> {
    unsafe extern "C" {
        fn renamex_np(
            from: *const std::ffi::c_char,
            to: *const std::ffi::c_char,
            flags: u32,
        ) -> i32;
    }
    let from =
        CString::new(from.as_os_str().as_bytes()).map_err(|_| io::ErrorKind::InvalidInput)?;
    let to = CString::new(to.as_os_str().as_bytes()).map_err(|_| io::ErrorKind::InvalidInput)?;
    // RENAME_SWAP, from the macOS SDK sys/stdio.h. Both names remain present atomically.
    if unsafe { renamex_np(from.as_ptr(), to.as_ptr(), 2) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) fn exchange(_: &Path, _: &Path) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

pub(super) fn publish(
    stage: &mut Scratch,
    bundle: &Path,
    swap: impl Fn(&Path, &Path) -> io::Result<()>,
    rename: impl Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let extracted = stage.0.join(super::ROOT);
    let backup = stage.0.join("previous.app");
    match swap(&extracted, bundle) {
        Ok(()) => {
            // The running path already contains the new whole bundle. A kill here leaves
            // the old whole bundle at extracted; backup retention never empties bundle.
            if let Err(error) = rename(&extracted, &backup) {
                if swap(&extracted, bundle).is_err() {
                    stage.preserve();
                }
                return Err(error);
            }
        }
        Err(error) if error.raw_os_error() == Some(ENOTSUP) => {
            rename(bundle, &backup)?;
            if let Err(error) = rename(&extracted, bundle) {
                if rename(&backup, bundle).is_err() {
                    stage.preserve();
                }
                return Err(error);
            }
        }
        Err(error) => return Err(error),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn errors_other_than_enotsup_never_enter_the_fallback() {
        let mut stage = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = stage.0.clone();
        for code in [1, 2, 5, 13, 18, 22] {
            assert_eq!(
                publish(
                    &mut stage,
                    &bundle,
                    |_, _| Err(io::Error::from_raw_os_error(code)),
                    |_, _| panic!("must not rename")
                )
                .unwrap_err()
                .raw_os_error(),
                Some(code)
            );
        }
    }
}
