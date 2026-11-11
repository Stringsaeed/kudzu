//! Identity-scoped LaunchAgent. Construction never registers an item.
use kudzu_app::{
    error::PortError,
    ports::{LoginItem, UpdateInstaller},
};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct LaunchAgent {
    launch: auto_launch::AutoLaunch,
    operation: Mutex<()>,
    installer: Arc<dyn UpdateInstaller>,
}

impl LaunchAgent {
    /// Pass the resolved identity, bundle executable and shared install-location probe.
    /// S40 treats a constructor error as login-item unavailable, never a startup failure.
    pub fn new(
        bundle_id: &str,
        executable: &Path,
        installer: Arc<dyn UpdateInstaller>,
    ) -> Result<Self, PortError> {
        let path = executable.to_str().ok_or(PortError::InvalidData)?;
        // auto-launch 0.6 interpolates XML without escaping. Refuse values it cannot encode.
        if !valid_id(bundle_id)
            || !executable.is_absolute()
            || path.chars().any(|c| c.is_control() || "<>&".contains(c))
        {
            return Err(PortError::InvalidData);
        }
        let launch = auto_launch::AutoLaunchBuilder::new()
            .set_app_name(bundle_id)
            .set_app_path(path)
            .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
            .set_bundle_identifiers(&[bundle_id])
            .set_args(&["--hidden"])
            .build()
            .map_err(|_| PortError::Unavailable)?;
        Ok(Self {
            launch,
            operation: Mutex::new(()),
            installer,
        })
    }
}

impl LaunchAgent {
    fn enable_with(&self, write: impl FnOnce() -> Result<(), PortError>) -> Result<(), PortError> {
        if self.installer.probe()?.needs_move {
            return Err(PortError::PermissionDenied);
        }
        let home = std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(PortError::Unavailable)?;
        let path = home
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", self.launch.get_app_name()));
        let result = (|| {
            write()?;
            let plist = plist::Value::from_file(&path).map_err(|_| PortError::InvalidData)?;
            let dict = plist.as_dictionary().ok_or(PortError::InvalidData)?;
            let expected = [self.launch.get_app_path(), "--hidden"];
            let arguments = dict
                .get("ProgramArguments")
                .and_then(plist::Value::as_array);
            if dict.contains_key("Program")
                || dict.get("Label").and_then(plist::Value::as_string)
                    != Some(self.launch.get_app_name())
                || !arguments.is_some_and(|args| {
                    args.len() == 2
                        && args
                            .iter()
                            .zip(expected)
                            .all(|(value, expected)| value.as_string() == Some(expected))
                })
            {
                return Err(PortError::InvalidData);
            }
            Ok(())
        })();
        if result.is_err() {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(PortError::Io(error.into())),
            }
        }
        result
    }
}

pub(crate) fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c))
        && id != "."
        && id != ".."
}

impl LoginItem for LaunchAgent {
    fn enabled(&self) -> Result<bool, PortError> {
        let _guard = self.operation.lock().unwrap();
        self.launch.is_enabled().map_err(map_error)
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        let _guard = self.operation.lock().unwrap();
        // Rewriting on enable also refreshes a moved executable's path.
        if enabled {
            self.enable_with(|| self.launch.enable().map_err(map_error))
        } else {
            self.launch.disable().map_err(map_error)
        }
    }
}

fn map_error(error: auto_launch::Error) -> PortError {
    match error {
        auto_launch::Error::Io(error) => PortError::Io(error.into()),
        _ => PortError::Unavailable,
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    #[derive(Default)]
    struct Probe(std::sync::atomic::AtomicBool);
    impl UpdateInstaller for Probe {
        fn probe(&self) -> Result<kudzu_app::ports::InstallProbe, PortError> {
            Ok(kudzu_app::ports::InstallProbe {
                needs_move: self.0.load(std::sync::atomic::Ordering::Relaxed),
            })
        }
        fn install(
            &self,
            _: &kudzu_app::ports::UpdatePackage,
        ) -> Result<kudzu_app::ports::InstallOutcome, PortError> {
            panic!("not an installer test");
        }
    }
    #[test]
    fn launch_agent_round_trip_uses_only_child_temporary_home() {
        let home = std::env::temp_dir().join(format!("kudzu-login-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(home.join("Library")).unwrap();
        fs::write(home.join("Kudzu"), b"fixture executable").unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "login_item::tests::isolated_home_child",
                "--ignored",
            ])
            .env("HOME", &home)
            .env("KUDZU_LOGIN_TEST_HOME", &home)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(!home
            .join("Library/LaunchAgents/sh.saeed.kudzu.next.plist")
            .exists());
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    #[ignore = "only invoked by parent with a fresh HOME"]
    fn isolated_home_child() {
        let home = std::env::var_os("KUDZU_LOGIN_TEST_HOME").expect("isolated child only");
        assert_eq!(std::env::var_os("HOME"), Some(home.clone()));
        let home = std::path::PathBuf::from(home);
        let agent = LaunchAgent::new(
            "sh.saeed.kudzu.next",
            &home.join("Kudzu"),
            Arc::new(Probe::default()),
        )
        .unwrap();
        assert!(!agent.enabled().unwrap());
        agent.set_enabled(false).unwrap();
        agent.set_enabled(true).unwrap();
        agent.set_enabled(true).unwrap();
        let file = home.join("Library/LaunchAgents/sh.saeed.kudzu.next.plist");
        let plist = plist::Value::from_file(&file).unwrap();
        assert_eq!(
            plist.as_dictionary().unwrap()["Label"].as_string(),
            Some("sh.saeed.kudzu.next")
        );
        let args = plist.as_dictionary().unwrap()["ProgramArguments"]
            .as_array()
            .unwrap();
        assert_eq!(
            args.iter()
                .map(|v| v.as_string().unwrap())
                .collect::<Vec<_>>(),
            vec![home.join("Kudzu").to_str().unwrap(), "--hidden"]
        );
        assert_eq!(
            plist.as_dictionary().unwrap()["RunAtLoad"].as_boolean(),
            Some(true)
        );
        assert_eq!(
            plist.as_dictionary().unwrap()["AssociatedBundleIdentifiers"]
                .as_array()
                .unwrap()[0]
                .as_string(),
            Some("sh.saeed.kudzu.next")
        );
        assert!(agent.enabled().unwrap());
        let prod = LaunchAgent::new(
            "sh.saeed.kudzu",
            &home.join("Kudzu"),
            Arc::new(Probe::default()),
        )
        .unwrap();
        assert!(!prod.enabled().unwrap());
        agent.set_enabled(false).unwrap();
        agent.set_enabled(false).unwrap();
        let blocked = Arc::new(Probe(std::sync::atomic::AtomicBool::new(true)));
        let blocked_agent =
            LaunchAgent::new("sh.saeed.kudzu.next", &home.join("Kudzu"), blocked).unwrap();
        assert_eq!(
            blocked_agent.set_enabled(true),
            Err(PortError::PermissionDenied)
        );
        assert!(!file.exists());
        for (label, args) in [
            (
                "wrong.id",
                vec![home.join("Kudzu").to_str().unwrap(), "--hidden"],
            ),
            ("sh.saeed.kudzu.next", vec!["/wrong/executable", "--hidden"]),
            (
                "sh.saeed.kudzu.next",
                vec![home.join("Kudzu").to_str().unwrap()],
            ),
        ] {
            let result = agent.enable_with(|| {
                let mut dict = plist::Dictionary::new();
                dict.insert("Label".into(), label.into());
                dict.insert(
                    "ProgramArguments".into(),
                    plist::Value::Array(args.iter().map(|v| (*v).into()).collect()),
                );
                plist::Value::Dictionary(dict).to_file_xml(&file).unwrap();
                Ok(())
            });
            assert_eq!(result, Err(PortError::InvalidData));
        }
        agent.set_enabled(false).unwrap();
        assert!(!agent.enabled().unwrap());
    }

    #[test]
    fn unsafe_xml_and_identity_are_refused_before_io() {
        assert!(LaunchAgent::new(
            "../elsewhere",
            Path::new("/tmp/Kudzu"),
            Arc::new(Probe::default())
        )
        .is_err());
        assert!(LaunchAgent::new(
            "sh.saeed.kudzu.next",
            Path::new("/tmp/A&B/Kudzu"),
            Arc::new(Probe::default())
        )
        .is_err());
        assert!(LaunchAgent::new(
            "sh.saeed.kudzu.next",
            Path::new("relative"),
            Arc::new(Probe::default())
        )
        .is_err());
    }

    #[test]
    fn login_readback_rejects_program_and_removes_failed_registration() {
        let mut passed = Vec::new();
        for case in ["cleanup", "program"] {
            let home =
                std::env::temp_dir().join(format!("kudzu-login-review-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(home.join("Library/LaunchAgents")).unwrap();
            fs::write(home.join("Kudzu"), b"fixture").unwrap();
            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "login_item::tests::login_readback_failure_child",
                    "--ignored",
                ])
                .env("HOME", &home)
                .env("KUDZU_LOGIN_TEST_HOME", &home)
                .env("KUDZU_LOGIN_REVIEW_CASE", case)
                .status()
                .unwrap();
            passed.push(status.success());
            fs::remove_dir_all(home).unwrap();
        }
        assert!(passed.into_iter().all(|passed| passed));
    }

    #[test]
    #[ignore = "invoked only by the isolated temporary-HOME readback parent"]
    fn login_readback_failure_child() {
        let home = std::path::PathBuf::from(
            std::env::var_os("KUDZU_LOGIN_TEST_HOME").expect("isolated child only"),
        );
        assert_eq!(std::env::var_os("HOME").unwrap(), home);
        let case = std::env::var("KUDZU_LOGIN_REVIEW_CASE").unwrap();
        let agent = LaunchAgent::new(
            "sh.saeed.kudzu.next",
            &home.join("Kudzu"),
            Arc::new(Probe::default()),
        )
        .unwrap();
        let path = home.join("Library/LaunchAgents/sh.saeed.kudzu.next.plist");
        let result = agent.enable_with(|| {
            let mut dict = plist::Dictionary::new();
            dict.insert(
                "Label".into(),
                if case == "cleanup" {
                    "wrong.id"
                } else {
                    "sh.saeed.kudzu.next"
                }
                .into(),
            );
            dict.insert(
                "ProgramArguments".into(),
                plist::Value::Array(vec![
                    home.join("Kudzu").to_str().unwrap().into(),
                    "--hidden".into(),
                ]),
            );
            if case == "program" {
                dict.insert("Program".into(), home.join("Evil").to_str().unwrap().into());
            }
            plist::Value::Dictionary(dict).to_file_xml(&path).unwrap();
            Ok(())
        });
        assert_eq!(result, Err(PortError::InvalidData), "case {case}");
        assert!(!path.exists(), "failed registration survived: {case}");
        assert!(!agent.enabled().unwrap());
    }
}
