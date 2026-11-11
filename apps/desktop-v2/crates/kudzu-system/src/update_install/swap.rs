//! Atomic publication only. ENOTSUP and EXDEV fail closed; no two-rename emulation.
use super::location::Scratch;
use std::{ffi::CString, io, os::unix::ffi::OsStrExt, path::Path};

#[cfg(test)]
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
    // This includes ENOTSUP (45) and EXDEV (18): neither may remove bundle's name.
    swap(&extracted, bundle)?;
    // The new complete app is already published; retain the old copy without ever
    // unlinking the installed name. Rollback is another atomic exchange.
    if let Err(error) = rename(&extracted, &backup) {
        if swap(&extracted, bundle).is_err() {
            stage.preserve();
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exchange_errors_never_enter_a_non_atomic_fallback() {
        let mut stage = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = stage.0.clone();
        for code in [1, 2, 5, 13, 18, 22, ENOTSUP] {
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

    #[test]
    fn unsupported_exchange_never_removes_the_installed_name() {
        let root = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Kudzu.app");
        std::fs::create_dir(&bundle).unwrap();
        std::fs::write(bundle.join("old"), b"old").unwrap();
        let mut stage = Scratch::for_bundle(&bundle).unwrap();
        std::fs::create_dir(stage.0.join(super::super::ROOT)).unwrap();
        let error = publish(
            &mut stage,
            &bundle,
            |_, _| Err(io::Error::from_raw_os_error(ENOTSUP)),
            |a, b| std::fs::rename(a, b),
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(ENOTSUP));
        assert!(bundle.join("old").exists());
    }

    #[test]
    fn cross_device_exchange_never_moves_either_bundle() {
        let root = Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Kudzu.app");
        std::fs::create_dir(&bundle).unwrap();
        std::fs::write(bundle.join("old"), b"old").unwrap();
        let mut stage = Scratch::for_bundle(&bundle).unwrap();
        std::fs::create_dir(stage.0.join(super::super::ROOT)).unwrap();
        let error = publish(
            &mut stage,
            &bundle,
            |_, _| Err(io::Error::from_raw_os_error(18)),
            |_, _| panic!("EXDEV must never rename"),
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(18));
        assert!(bundle.join("old").exists());
        assert!(stage.0.join(super::super::ROOT).exists());
    }
}
