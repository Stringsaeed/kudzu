//! Fake bundle probe and installation without touching a bundle.
use crate::{
    error::PortError,
    ports::{InstallProbe, UpdateInstaller, UpdatePackage},
};
use std::sync::Mutex;
#[derive(Default)]
pub struct FakeInstaller {
    pub probe: Mutex<InstallProbe>,
    pub installed: Mutex<Vec<UpdatePackage>>,
    pub failure: Mutex<Option<PortError>>,
}
impl UpdateInstaller for FakeInstaller {
    fn probe(&self) -> Result<InstallProbe, PortError> {
        Ok(*self.probe.lock().unwrap())
    }
    fn install(&self, package: &UpdatePackage) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        self.installed.lock().unwrap().push(package.clone());
        Ok(())
    }
}
