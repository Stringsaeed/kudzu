//! The updater archive. std-only on purpose: S36's installer tests compile this exact file
//! (`#[path]`) and run the installer's own validator on what it produces, so the validator is
//! never duplicated and `cargo xtask check-deps` keeps xtask free of `kudzu-*` dependencies.
use std::{error::Error, path::Path, process::Command};

/// Arguments for `/usr/bin/tar` (bsdtar). macOS tar stores extended attributes, such as
/// `com.apple.provenance`, as AppleDouble `._` entries and PAX `xattr` headers. A top-level
/// `._Kudzu.app` is rejected by the installer, and a nested `Kudzu.app/Contents/._Info.plist`
/// breaks `codesign --strict` after extraction. ustar is the one representation the installer's
/// validator reads.
pub fn tar_args(parent: &Path, root: &str, output: &Path) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = [
        "--format",
        "ustar",
        "--no-mac-metadata",
        "--no-xattrs",
        "--no-acls",
        "-czf",
    ]
    .iter()
    .map(Into::into)
    .collect();
    args.push(output.into());
    args.push("-C".into());
    args.push(parent.into());
    args.push(root.into());
    args
}

/// Archives `parent/root` into `output` (a `.tar.gz`) without any macOS metadata entries.
pub fn create(parent: &Path, root: &str, output: &Path) -> Result<(), Box<dyn Error>> {
    let status = Command::new("/usr/bin/tar")
        .args(tar_args(parent, root, output))
        .env("COPYFILE_DISABLE", "1")
        .status()?;
    if !status.success() {
        return Err("release stage failed: tar".into());
    }
    Ok(())
}
