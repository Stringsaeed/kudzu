//! In-memory registrations with per-chord failure injection for rollback tests.
use crate::{
    error::PortError,
    ports::{HotkeyRegistrar, ShortcutSlot},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};
#[derive(Default)]
pub struct FakeHotkeys {
    pub registered: Mutex<BTreeMap<ShortcutSlot, String>>,
    pub rejected: Mutex<BTreeSet<String>>,
    pub unregister_failure: Mutex<Option<PortError>>,
}
impl HotkeyRegistrar for FakeHotkeys {
    fn register(&self, slot: ShortcutSlot, chord: &str) -> Result<(), PortError> {
        if self.rejected.lock().unwrap().contains(chord) {
            return Err(PortError::Unavailable);
        }
        self.registered.lock().unwrap().insert(slot, chord.into());
        Ok(())
    }
    fn unregister(&self, slot: ShortcutSlot) -> Result<(), PortError> {
        if let Some(e) = *self.unregister_failure.lock().unwrap() {
            return Err(e);
        }
        self.registered.lock().unwrap().remove(&slot);
        Ok(())
    }
}
