//! In-memory pasteboard; external copies are distinguishable from Kudzu writes.
use crate::{
    error::PortError,
    ports::{PasteSnapshot, Pasteboard},
};
use kudzu_core::{clipboard::PasteItem, system::AppInfo};
use std::sync::Mutex;
#[derive(Default)]
pub struct FakePasteboard {
    pub snapshot: Mutex<PasteSnapshot>,
    pub paste_allowed: Mutex<bool>,
    pub pasted_into: Mutex<Vec<Option<AppInfo>>>,
    pub failure: Mutex<Option<PortError>>,
}
impl FakePasteboard {
    pub fn external_copy(&self, items: Vec<PasteItem>) {
        let mut state = self.snapshot.lock().unwrap();
        state.change_count += 1;
        state.own = false;
        state.items = items;
    }
}
impl Pasteboard for FakePasteboard {
    fn change_count(&self) -> i64 {
        self.snapshot.lock().unwrap().change_count
    }
    fn read(&self) -> Result<PasteSnapshot, PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        Ok(self.snapshot.lock().unwrap().clone())
    }
    fn write(&self, items: &[PasteItem]) -> Result<i64, PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        let mut state = self.snapshot.lock().unwrap();
        state.change_count += 1;
        state.own = true;
        state.items = items.to_vec();
        Ok(state.change_count)
    }
    fn can_paste(&self) -> bool {
        *self.paste_allowed.lock().unwrap()
    }
    fn paste(&self, target: Option<&AppInfo>) -> Result<(), PortError> {
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        if !self.can_paste() {
            return Err(PortError::PermissionDenied);
        }
        self.pasted_into.lock().unwrap().push(target.cloned());
        Ok(())
    }
}
