//! Fake bundle probe and installation without touching a bundle.
use crate::{
    error::PortError,
    ports::{InstallOutcome, InstallProbe, UpdateInstaller, UpdatePackage},
};
use std::sync::Mutex;
#[derive(Default)]
pub struct FakeInstaller {
    pub probe: Mutex<InstallProbe>,
    pub installed: Mutex<Vec<UpdatePackage>>,
    pub failure: Mutex<Option<PortError>>,
    pub outcome: Mutex<InstallOutcome>,
}
impl UpdateInstaller for FakeInstaller {
    fn probe(&self) -> Result<InstallProbe, PortError> {
        Ok(*self.probe.lock().unwrap())
    }
    fn install(&self, package: &UpdatePackage) -> Result<InstallOutcome, PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        let outcome = self.outcome.lock().unwrap().clone();
        if outcome == InstallOutcome::Installed {
            self.installed.lock().unwrap().push(package.clone());
        }
        Ok(outcome)
    }
}
