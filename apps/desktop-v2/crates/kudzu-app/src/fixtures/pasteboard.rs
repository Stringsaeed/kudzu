//! In-memory pasteboard; external copies are distinguishable from Kudzu writes.
use super::apps::Latch;
use crate::{
    error::PortError,
    ports::{PasteSnapshot, Pasteboard},
};
use kudzu_core::{clipboard::PasteItem, system::AppInfo};
use std::sync::{Arc, Mutex};
#[derive(Default)]
pub struct FakePasteboard {
    pub snapshot: Mutex<PasteSnapshot>,
    pub reads: Mutex<usize>,
    pub paste_allowed: Mutex<bool>,
    pub pasted_into: Mutex<Vec<Option<AppInfo>>>,
    /// The plain text on the pasteboard at each paste.
    pub pasted_text: Mutex<Vec<Option<String>>>,
    /// While set, `paste` waits for this latch after setting `paste_entered`, so a test can
    /// look at the world at the moment the keystroke is about to be posted.
    pub paste_hold: Mutex<Option<Arc<Latch>>>,
    pub paste_entered: Latch,
    pub failure: Mutex<Option<PortError>>,
    pub text_reads: Mutex<usize>,
    #[cfg(test)]
    pub before_read: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
    #[cfg(test)]
    pub after_write: Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>>,
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
        #[cfg(test)]
        {
            let hook = self.before_read.lock().unwrap().clone();
            if let Some(hook) = hook {
                hook();
            }
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
        let count = state.change_count;
        drop(state);
        #[cfg(test)]
        {
            let hook = self.after_write.lock().unwrap().clone();
            if let Some(hook) = hook {
                hook();
            }
        }
        Ok(count)
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
        self.paste_entered.set();
        let hold = self.paste_hold.lock().unwrap().clone();
        if let Some(hold) = hold {
            hold.wait();
        }
        let text = kudzu_core::clipboard::text_of(&self.snapshot.lock().unwrap().items);
        self.pasted_text.lock().unwrap().push(text);
        self.pasted_into.lock().unwrap().push(target.cloned());
        Ok(())
    }
}
