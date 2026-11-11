//! Opening a pane or requesting access does not change the configured grants.
use crate::{
    error::PortError,
    ports::{PermissionKind, SettingsPane, SystemSettings},
};
use kudzu_core::system::PermissionStatus;
use std::sync::Mutex;
#[derive(Default)]
pub struct FakeSystemSettings {
    pub permissions: Mutex<PermissionStatus>,
    pub opened: Mutex<Vec<SettingsPane>>,
    pub requested: Mutex<Vec<PermissionKind>>,
    pub failure: Mutex<Option<PortError>>,
}
impl SystemSettings for FakeSystemSettings {
    fn permissions(&self) -> PermissionStatus {
        *self.permissions.lock().unwrap()
    }
    fn request(&self, kind: PermissionKind) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        self.requested.lock().unwrap().push(kind);
        Ok(())
    }
    fn open(&self, pane: SettingsPane) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        self.opened.lock().unwrap().push(pane);
        Ok(())
    }
}
