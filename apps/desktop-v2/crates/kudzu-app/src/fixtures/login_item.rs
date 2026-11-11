//! In-memory login-item state.
use crate::{error::PortError, ports::LoginItem};
use std::sync::Mutex;
#[derive(Default)]
pub struct FakeLoginItem {
    pub enabled: Mutex<bool>,
    pub failure: Mutex<Option<PortError>>,
}
impl LoginItem for FakeLoginItem {
    fn enabled(&self) -> Result<bool, PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        Ok(*self.enabled.lock().unwrap())
    }
    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        *self.enabled.lock().unwrap() = enabled;
        Ok(())
    }
}
