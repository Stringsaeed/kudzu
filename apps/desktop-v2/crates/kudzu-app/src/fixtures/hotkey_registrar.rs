//! In-memory registrations with per-chord failure injection for rollback tests.
use crate::{
    error::PortError,
    ports::{HotkeyRegistrar, ShortcutSlot},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyOp {
    Register(ShortcutSlot, String),
    Unregister(ShortcutSlot),
}

#[derive(Default)]
pub struct FakeHotkeys {
    pub registered: Mutex<BTreeMap<ShortcutSlot, String>>,
    pub rejected: Mutex<BTreeSet<String>>,
    pub unregister_failure: Mutex<Option<PortError>>,
    pub log: Mutex<Vec<HotkeyOp>>,
}
impl HotkeyRegistrar for FakeHotkeys {
    fn register(&self, slot: ShortcutSlot, chord: &str) -> Result<(), PortError> {
        self.log
            .lock()
            .unwrap()
            .push(HotkeyOp::Register(slot, chord.into()));
        if self.rejected.lock().unwrap().contains(chord) {
            return Err(PortError::Unavailable);
        }
        self.registered.lock().unwrap().insert(slot, chord.into());
        Ok(())
    }
    fn unregister(&self, slot: ShortcutSlot) -> Result<(), PortError> {
        self.log.lock().unwrap().push(HotkeyOp::Unregister(slot));
        if let Some(e) = *self.unregister_failure.lock().unwrap() {
            return Err(e);
        }
        self.registered.lock().unwrap().remove(&slot);
        Ok(())
    }
}
