//! Identity-scoped LaunchAgent. Construction never registers an item.
use kudzu_app::{error::PortError, ports::LoginItem};
use std::path::Path;
use std::sync::Mutex;

pub struct LaunchAgent {
    launch: auto_launch::AutoLaunch,
    operation: Mutex<()>,
}

impl LaunchAgent {
    /// Pass the resolved identity and the absolute bundle executable, not the `.app`.
    pub fn new(bundle_id: &str, executable: &Path) -> Result<Self, PortError> {
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
        })
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
            self.launch.enable()
        } else {
            self.launch.disable()
        }
        .map_err(map_error)
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
        let agent = LaunchAgent::new("sh.saeed.kudzu.next", &home.join("Kudzu")).unwrap();
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
        let prod = LaunchAgent::new("sh.saeed.kudzu", &home.join("Kudzu")).unwrap();
        assert!(!prod.enabled().unwrap());
        agent.set_enabled(false).unwrap();
        agent.set_enabled(false).unwrap();
        assert!(!agent.enabled().unwrap());
    }

    #[test]
    fn unsafe_xml_and_identity_are_refused_before_io() {
        assert!(LaunchAgent::new("../elsewhere", Path::new("/tmp/Kudzu")).is_err());
        assert!(LaunchAgent::new("sh.saeed.kudzu.next", Path::new("/tmp/A&B/Kudzu")).is_err());
        assert!(LaunchAgent::new("sh.saeed.kudzu.next", Path::new("relative")).is_err());
    }
}
