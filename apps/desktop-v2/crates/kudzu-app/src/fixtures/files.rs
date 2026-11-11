//! Records requests without opening a real file or application.
use crate::{error::PortError, ports::FileOpener};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
#[derive(Default)]
pub struct FakeFiles {
    pub opened: Mutex<Vec<PathBuf>>,
    pub revealed: Mutex<Vec<PathBuf>>,
    pub failure: Mutex<Option<PortError>>,
}
impl FileOpener for FakeFiles {
    fn open(&self, path: &Path) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        self.opened.lock().unwrap().push(path.into());
        Ok(())
    }
    fn reveal(&self, path: &Path) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        self.revealed.lock().unwrap().push(path.into());
        Ok(())
    }
}
