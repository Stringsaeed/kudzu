//! Records restarts without starting a process.
use crate::{error::PortError, ports::Relauncher};
use std::sync::Mutex;
#[derive(Default)]
pub struct FakeRelauncher {
    pub calls: Mutex<usize>,
    pub failure: Mutex<Option<PortError>>,
}
impl Relauncher for FakeRelauncher {
    fn relaunch(&self) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        *self.calls.lock().unwrap() += 1;
        Ok(())
    }
}
