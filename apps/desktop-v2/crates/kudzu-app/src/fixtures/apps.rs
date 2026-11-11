//! Catalogue and focus fixture, with no NSWorkspace or AX access.
use crate::{
    error::PortError,
    ports::{AppCatalog, TargetApp},
};
use kudzu_core::system::{AppInfo, Rect};
use std::{collections::BTreeMap, sync::Mutex};
#[derive(Default)]
pub struct FakeApps {
    pub installed: Mutex<Vec<AppInfo>>,
    pub icons: Mutex<BTreeMap<String, Vec<u8>>>,
    pub frontmost: Mutex<Option<AppInfo>>,
    pub selection: Mutex<Option<String>>,
    pub selection_reads: Mutex<usize>,
    pub activations: Mutex<Vec<AppInfo>>,
    pub caret: Mutex<Option<Rect>>,
    pub failure: Mutex<Option<PortError>>,
    pub installed_calls: Mutex<usize>,
    pub icon_calls: Mutex<Vec<(String, u32)>>,
}
impl FakeApps {
    fn check(&self) -> Result<(), PortError> {
        self.failure.lock().unwrap().map_or(Ok(()), Err)
    }
}
impl AppCatalog for FakeApps {
    fn installed(&self) -> Result<Vec<AppInfo>, PortError> {
        self.check()?;
        *self.installed_calls.lock().unwrap() += 1;
        Ok(self.installed.lock().unwrap().clone())
    }
    fn icon(&self, id: &str, size: u32) -> Result<Option<Vec<u8>>, PortError> {
        self.check()?;
        self.icon_calls.lock().unwrap().push((id.into(), size));
        Ok(self.icons.lock().unwrap().get(id).cloned())
    }
}
impl TargetApp for FakeApps {
    fn frontmost(&self) -> Result<Option<AppInfo>, PortError> {
        self.check()?;
        Ok(self.frontmost.lock().unwrap().clone())
    }
    fn activate(&self, app: &AppInfo) -> Result<(), PortError> {
        self.check()?;
        self.activations.lock().unwrap().push(app.clone());
        *self.frontmost.lock().unwrap() = Some(app.clone());
        Ok(())
    }
    fn selection(&self) -> Result<Option<String>, PortError> {
        *self.selection_reads.lock().unwrap() += 1;
        self.check()?;
        Ok(self.selection.lock().unwrap().clone())
    }
    fn caret(&self) -> Result<Option<Rect>, PortError> {
        self.check()?;
        Ok(*self.caret.lock().unwrap())
    }
}
