//! Stand-ins for adapters that cannot exist in this process (an unbundled `cargo run`, or a
//! constructor that failed). They report `Unavailable`; none of them pretends to succeed.
use kudzu_app::{
    error::PortError,
    ports::{
        HotkeyRegistrar, InstallOutcome, InstallProbe, LoginItem, Relauncher, ShortcutSlot,
        UpdateInstaller, UpdatePackage,
    },
};

/// Login item unavailable: reading says off; enabling fails, so Settings can say so.
pub(crate) struct Login;
impl LoginItem for Login {
    fn enabled(&self) -> Result<bool, PortError> {
        Ok(false)
    }
    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        if enabled {
            Err(PortError::Unavailable)
        } else {
            Ok(())
        }
    }
}

pub(crate) struct Installer;
impl UpdateInstaller for Installer {
    fn probe(&self) -> Result<InstallProbe, PortError> {
        Err(PortError::Unavailable)
    }
    fn install(&self, _: &UpdatePackage) -> Result<InstallOutcome, PortError> {
        Err(PortError::Unavailable)
    }
}

pub(crate) struct Relaunch;
impl Relauncher for Relaunch {
    fn relaunch(&self) -> Result<(), PortError> {
        Err(PortError::Unavailable)
    }
}

/// Never registered: the UI shell is the process's registrar.
pub(crate) struct Hotkeys;
impl HotkeyRegistrar for Hotkeys {
    fn register(&self, _: ShortcutSlot, _: &str) -> Result<(), PortError> {
        Err(PortError::Unavailable)
    }
    fn unregister(&self, _: ShortcutSlot) -> Result<(), PortError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_adapters_never_report_success_for_an_action() {
        assert!(!Login.enabled().unwrap());
        assert!(matches!(
            Login.set_enabled(true),
            Err(PortError::Unavailable)
        ));
        assert!(Login.set_enabled(false).is_ok());
        assert!(matches!(Installer.probe(), Err(PortError::Unavailable)));
        assert!(matches!(Relaunch.relaunch(), Err(PortError::Unavailable)));
        assert!(Hotkeys
            .register(ShortcutSlot::QuickSearch, "Control+Space")
            .is_err());
    }
}
