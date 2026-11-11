//! Launch Services reopen and explicit restart, with no shell interpolation of paths.
use kudzu_app::{error::PortError, ports::Relauncher};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

/// The quit callback must run the ordered application shutdown and exit (S40).
/// The helper waits for this process to exit before launching, so it cannot lose the flock race.
pub struct BundleRelauncher {
    bundle: PathBuf,
    quit: Arc<dyn Fn() + Send + Sync>,
}

impl BundleRelauncher {
    pub fn new(
        bundle: PathBuf,
        quit: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, PortError> {
        validate_bundle(&bundle)?;
        Ok(Self {
            bundle,
            quit: Arc::new(quit),
        })
    }
}

const RESTART: &str = r#"pid=$1; bundle=$2
while /bin/kill -0 "$pid" 2>/dev/null; do /bin/sleep 0.1; done
exec /usr/bin/open -n "$bundle"
"#;

impl Relauncher for BundleRelauncher {
    fn relaunch(&self) -> Result<(), PortError> {
        self.restart_with(|cmd| {
            // The helper outlives this process; it has no inherited stdin/stdout or UI.
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|e| PortError::Io(e.into()))
        })
    }
}

impl BundleRelauncher {
    fn restart_with(
        &self,
        spawn: impl FnOnce(&mut Command) -> Result<(), PortError>,
    ) -> Result<(), PortError> {
        validate_bundle(&self.bundle)?;
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                RESTART,
                "kudzu-restart",
                &std::process::id().to_string(),
            ])
            .arg(&self.bundle);
        spawn(&mut command)?;
        (self.quit)();
        Ok(())
    }
}

pub(crate) fn validate_bundle(bundle: &Path) -> Result<(), PortError> {
    if !bundle.is_absolute() || bundle.extension().is_none_or(|e| e != "app") || !bundle.is_dir() {
        return Err(PortError::Unavailable);
    }
    Ok(())
}

pub(crate) fn reopen(bundle: &Path) -> Result<(), PortError> {
    validate_bundle(bundle)?;
    let status = Command::new("/usr/bin/open")
        .arg(bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| PortError::Io(e.into()))?;
    if status.success() {
        Ok(())
    } else {
        Err(PortError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    #[test]
    fn bundled_restart_passes_path_as_one_argument_then_requests_shutdown() {
        let bundle = std::env::temp_dir().join(format!("Kudzu ' $ {}.app", uuid::Uuid::new_v4()));
        std::fs::create_dir(&bundle).unwrap();
        let quit = Arc::new(AtomicBool::new(false));
        let marker = quit.clone();
        let relaunch = BundleRelauncher::new(bundle.clone(), move || {
            marker.store(true, Ordering::SeqCst);
        })
        .unwrap();
        relaunch
            .restart_with(|cmd| {
                assert_eq!(cmd.get_program(), "/bin/sh");
                let args: Vec<_> = cmd.get_args().collect();
                assert_eq!(args[1], RESTART);
                assert_eq!(args.last().unwrap(), &bundle.as_os_str());
                assert!(!quit.load(Ordering::SeqCst));
                Ok(())
            })
            .unwrap();
        assert!(quit.load(Ordering::SeqCst));
        quit.store(false, Ordering::SeqCst);
        assert!(relaunch
            .restart_with(|_| Err(PortError::Unavailable))
            .is_err());
        assert!(!quit.load(Ordering::SeqCst));
        std::fs::remove_dir(bundle).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "explicit temporary-bundle Launch Services smoke; no product app is launched"]
    fn launch_services_restarts_a_temporary_bundle_after_the_old_process_exits() {
        use std::{
            fs,
            os::unix::fs::PermissionsExt,
            time::{Duration, Instant},
        };
        let bundle = std::env::temp_dir().join(format!(
            "Kudzu restart fixture {}.app",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(bundle.join("Contents/MacOS")).unwrap();
        let executable = bundle.join("Contents/MacOS/Kudzu");
        fs::write(
            &executable,
            b"#!/bin/sh\nbundle=${0%/Contents/MacOS/Kudzu}\n: > \"$bundle/relaunched\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        let mut dict = plist::Dictionary::new();
        dict.insert(
            "CFBundleIdentifier".into(),
            format!("sh.saeed.kudzu.fixture.{}", uuid::Uuid::new_v4()).into(),
        );
        dict.insert("CFBundleExecutable".into(), "Kudzu".into());
        dict.insert("CFBundlePackageType".into(), "APPL".into());
        dict.insert("CFBundleVersion".into(), "1.0.0".into());
        dict.insert("LSUIElement".into(), true.into());
        plist::Value::Dictionary(dict)
            .to_file_xml(bundle.join("Contents/Info.plist"))
            .unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "relaunch::tests::restart_child", "--ignored"])
            .env("KUDZU_RESTART_TEST_BUNDLE", &bundle)
            .status()
            .unwrap();
        assert!(status.success());
        let start = Instant::now();
        while !bundle.join("relaunched").exists() && start.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let relaunched = bundle.join("relaunched").exists();
        fs::remove_dir_all(bundle).unwrap();
        assert!(
            relaunched,
            "Launch Services did not launch the temporary fixture"
        );
    }

    #[test]
    #[ignore = "only invoked by the isolated temporary-bundle restart test"]
    fn restart_child() {
        let bundle = std::env::var_os("KUDZU_RESTART_TEST_BUNDLE").expect("isolated child only");
        BundleRelauncher::new(PathBuf::from(bundle), || std::process::exit(0))
            .unwrap()
            .relaunch()
            .unwrap();
        panic!("quit callback must exit");
    }
}
