//! `HotkeyRegistrar` over Rune's global shortcuts. Quick Search's chord toggles the palette,
//! Clipboard History's the picker. Main thread only: `kudzu_app::hotkeys::HotkeySlots` calls it
//! from `main`'s poll, where `register_shortcut` answers at once (an off-thread caller would
//! block on the loop). Collision and rollback stay in `HotkeySlots` (SHL-6).
use super::windows::{PaletteState, PickerState};
use kudzu_app::error::PortError;
use kudzu_app::ports::{HotkeyRegistrar, ShortcutSlot};
use rune::{AppHandle, ShortcutError, ShortcutId};
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

pub(crate) struct Registrar {
    palette: AppHandle<PaletteState>,
    picker: AppHandle<PickerState>,
    ids: Mutex<BTreeMap<ShortcutSlot, ShortcutId>>,
}

impl Registrar {
    pub(crate) fn new(palette: AppHandle<PaletteState>, picker: AppHandle<PickerState>) -> Self {
        Self {
            palette,
            picker,
            ids: Mutex::new(BTreeMap::new()),
        }
    }

    fn release(&self, slot: ShortcutSlot, id: ShortcutId) -> Result<(), ShortcutError> {
        match slot {
            ShortcutSlot::QuickSearch => self.palette.unregister_shortcut(id),
            ShortcutSlot::Clipboard => self.picker.unregister_shortcut(id),
        }
    }
}

impl HotkeyRegistrar for Registrar {
    fn register(&self, slot: ShortcutSlot, chord: &str) -> Result<(), PortError> {
        let shortcut: rune::Shortcut = chord.parse().map_err(port_error)?;
        let id = match slot {
            ShortcutSlot::QuickSearch => self
                .palette
                .register_shortcut(shortcut, |s: &mut PaletteState, _| s.toggle()),
            ShortcutSlot::Clipboard => self
                .picker
                .register_shortcut(shortcut, |s: &mut PickerState, _| s.toggle()),
        }
        .map_err(port_error)?;
        let replaced = self
            .ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(slot, id);
        if let Some(old) = replaced {
            let _ = self.release(slot, old);
        }
        Ok(())
    }

    fn unregister(&self, slot: ShortcutSlot) -> Result<(), PortError> {
        let id = self
            .ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&slot);
        match id {
            Some(id) => self.release(slot, id).map_err(port_error),
            None => Ok(()),
        }
    }
}

/// A chord another app or the system owns is "taken"; the slot rolls back to the old one.
fn port_error(error: ShortcutError) -> PortError {
    match error {
        ShortcutError::InvalidAccelerator(_) => PortError::InvalidData,
        ShortcutError::PermissionDenied => PortError::PermissionDenied,
        _ => PortError::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::port_error;
    use kudzu_app::error::PortError;
    use rune::ShortcutError;

    #[test]
    fn rune_errors_map_to_port_errors_the_slots_roll_back_on() {
        assert_eq!(port_error(ShortcutError::Conflict), PortError::Unavailable);
        assert_eq!(
            port_error(ShortcutError::InvalidAccelerator("x".into())),
            PortError::InvalidData
        );
        assert_eq!(
            port_error(ShortcutError::PermissionDenied),
            PortError::PermissionDenied
        );
    }

    #[test]
    fn the_saved_chord_format_parses_as_a_rune_shortcut() {
        for chord in ["Control+Alt+Space", "Control+Alt+Shift+V", "Super+Shift+F5"] {
            assert!(chord.parse::<rune::Shortcut>().is_ok(), "{chord}");
        }
    }
}
