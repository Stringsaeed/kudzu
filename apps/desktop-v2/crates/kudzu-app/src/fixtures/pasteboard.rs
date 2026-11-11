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
    pub reads: Mutex<usize>,
    pub paste_allowed: Mutex<bool>,
    pub pasted_into: Mutex<Vec<Option<AppInfo>>>,
    pub failure: Mutex<Option<PortError>>,
    pub text_reads: Mutex<usize>,
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
    fn is_own(&self, change_count: i64) -> bool {
        let state = self.snapshot.lock().unwrap();
        state.change_count == change_count && state.own
    }
    fn has_private_marker(&self, change_count: i64) -> Result<bool, PortError> {
        let state = self.snapshot.lock().unwrap();
        if state.change_count != change_count {
            return Err(PortError::Cancelled);
        }
        Ok(kudzu_core::clipboard::is_private(&state.items))
    }
    fn read(&self) -> Result<PasteSnapshot, PortError> {
        *self.reads.lock().unwrap() += 1;
        if let Some(e) = *self.failure.lock().unwrap() {
            return Err(e);
        }
        Ok(self.snapshot.lock().unwrap().clone())
    }
    fn read_text(&self) -> Result<Option<String>, PortError> {
        *self.text_reads.lock().unwrap() += 1;
        if let Some(error) = *self.failure.lock().unwrap() {
            return Err(error);
        }
        Ok(self
            .snapshot
            .lock()
            .unwrap()
            .items
            .iter()
            .flat_map(|item| item.iter())
            .find(|(kind, _)| kind == "public.utf8-plain-text")
            .and_then(|(_, bytes)| String::from_utf8(bytes.clone()).ok()))
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
