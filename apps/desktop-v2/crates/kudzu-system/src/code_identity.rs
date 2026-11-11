//! One Developer ID requirement for static bundles and loaded process identities.
use kudzu_app::error::PortError;
use std::{
    path::Path,
    process::{Command, Stdio},
};

fn requirement(bundle_id: &str) -> String {
    format!("anchor apple generic and identifier \"{bundle_id}\" and certificate leaf[subject.OU] = \"V3HN8HXZYK\" and certificate 1[field.1.2.840.113635.100.6.2.6] exists and certificate leaf[field.1.2.840.113635.100.6.1.13] exists")
}

fn constrain_version(base: &str, version: &semver::Version) -> String {
    format!("{base} and info[CFBundleShortVersionString] = \"{version}\"")
}
fn command_with_requirement(bundle: &Path, requirement: &str) -> Command {
    let mut command = Command::new("/usr/bin/codesign");
    command
        .args(["--verify", "--deep", "--strict", "--test-requirement"])
        .arg(format!("={requirement}"))
        .arg(bundle);
    command
}
pub(crate) fn command(bundle: &Path, bundle_id: &str) -> Command {
    command_with_requirement(bundle, &requirement(bundle_id))
}
pub(crate) fn verify_versioned_bundle(
    bundle: &Path,
    bundle_id: &str,
    version: &semver::Version,
) -> Result<(), PortError> {
    let status =
        command_with_requirement(bundle, &constrain_version(&requirement(bundle_id), version))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| PortError::Io(e.into()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PortError::InvalidData)
    }
}

pub(crate) fn verify_bundle(bundle: &Path, bundle_id: &str) -> Result<(), PortError> {
    let status = command(bundle, bundle_id)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| PortError::Io(e.into()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PortError::InvalidData)
    }
}

pub(crate) fn verify_process(pid: u32, bundle_id: &str) -> Result<(), PortError> {
    check_process(pid, &requirement(bundle_id))
}

#[cfg(target_os = "macos")]
fn check_process(pid: u32, requirement: &str) -> Result<(), PortError> {
    use core_foundation::{
        base::{CFRelease, TCFType},
        dictionary::{CFDictionary, CFDictionaryRef},
        number::CFNumber,
        string::{CFString, CFStringRef},
    };
    use std::{ffi::c_void, ptr};
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecGuestAttributePid: CFStringRef;
        fn SecCodeCopyGuestWithAttributes(
            host: *const c_void,
            attributes: CFDictionaryRef,
            flags: u32,
            guest: *mut *const c_void,
        ) -> i32;
        fn SecRequirementCreateWithString(
            text: CFStringRef,
            flags: u32,
            requirement: *mut *const c_void,
        ) -> i32;
        fn SecCodeCheckValidity(code: *const c_void, flags: u32, requirement: *const c_void)
            -> i32;
    }
    struct Owned(*const c_void);
    impl Owned {
        fn checked(status: i32, raw: *const c_void) -> Result<Self, PortError> {
            let object = Self(raw);
            if status == 0 && !raw.is_null() {
                Ok(object)
            } else {
                Err(PortError::InvalidData)
            }
        }
    }
    impl Drop for Owned {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    CFRelease(self.0);
                }
            }
        }
    }
    let pid = i32::try_from(pid)
        .ok()
        .filter(|p| *p > 0)
        .ok_or(PortError::InvalidData)?;
    let attributes = CFDictionary::from_CFType_pairs(&[(
        unsafe { CFString::wrap_under_get_rule(kSecGuestAttributePid) },
        CFNumber::from(pid),
    )]);
    let mut code = ptr::null();
    let status = unsafe {
        SecCodeCopyGuestWithAttributes(ptr::null(), attributes.as_concrete_TypeRef(), 0, &mut code)
    };
    let code = Owned::checked(status, code)?;
    let mut parsed = ptr::null();
    let status = unsafe {
        SecRequirementCreateWithString(
            CFString::new(requirement).as_concrete_TypeRef(),
            0,
            &mut parsed,
        )
    };
    let parsed = Owned::checked(status, parsed)?;
    // Dynamic validation establishes the loaded code's identity; the bundle's full
    // resource seal is checked independently by verify_bundle before opening it.
    if unsafe { SecCodeCheckValidity(code.0, 0, parsed.0) } == 0 {
        Ok(())
    } else {
        Err(PortError::InvalidData)
    }
}
#[cfg(not(target_os = "macos"))]
fn check_process(_: u32, _: &str) -> Result<(), PortError> {
    Err(PortError::Unavailable)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn loaded_process_identity_is_checked_without_a_keychain_or_real_signing_identity() {
        let root = crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
        let executable = root.0.join("process-fixture");
        std::fs::copy("/bin/sleep", &executable).unwrap();
        assert!(Command::new("/usr/bin/codesign")
            .args([
                "--force",
                "--sign",
                "-",
                "--identifier",
                "sh.saeed.kudzu.process-fixture"
            ])
            .arg(&executable)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let child = Child(Command::new(executable).arg("10").spawn().unwrap());
        assert_eq!(
            check_process(
                child.0.id(),
                "identifier \"sh.saeed.kudzu.process-fixture\""
            ),
            Ok(())
        );
        assert_eq!(
            check_process(child.0.id(), "identifier \"wrong.identity\""),
            Err(PortError::InvalidData)
        );
        assert_eq!(
            verify_process(child.0.id(), "sh.saeed.kudzu.process-fixture"),
            Err(PortError::InvalidData)
        );
    }

    #[test]
    fn version_requirement_accepts_only_the_version_in_the_valid_resource_seal() {
        let root = crate::update_install::location::Scratch::beside(&std::env::temp_dir()).unwrap();
        let bundle = root.0.join("Fixture.app");
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        std::fs::copy("/usr/bin/true", bundle.join("Contents/MacOS/Kudzu")).unwrap();
        let mut info = plist::Dictionary::new();
        info.insert(
            "CFBundleIdentifier".into(),
            "sh.saeed.kudzu.version-fixture".into(),
        );
        info.insert("CFBundleExecutable".into(), "Kudzu".into());
        info.insert("CFBundleShortVersionString".into(), "1.0.0".into());
        plist::Value::Dictionary(info.clone())
            .to_file_xml(bundle.join("Contents/Info.plist"))
            .unwrap();
        assert!(Command::new("/usr/bin/codesign")
            .args(["--force", "--sign", "-", "--timestamp=none"])
            .arg(&bundle)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
        let base = "identifier \"sh.saeed.kudzu.version-fixture\"";
        let accepts = |version: &str| {
            command_with_requirement(
                &bundle,
                &constrain_version(base, &semver::Version::parse(version).unwrap()),
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
        };
        assert!(accepts("1.0.0"));
        assert!(!accepts("2.0.0"));
        info.insert("CFBundleShortVersionString".into(), "2.0.0".into());
        plist::Value::Dictionary(info)
            .to_file_xml(bundle.join("Contents/Info.plist"))
            .unwrap();
        assert!(
            !accepts("2.0.0"),
            "an unsigned plist version change must not satisfy the check"
        );
    }
}
