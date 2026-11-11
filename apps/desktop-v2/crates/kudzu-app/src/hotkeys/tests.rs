use super::*;
use crate::fixtures::clock::FakeClock;
use crate::fixtures::hotkey_registrar::{FakeHotkeys, HotkeyOp};
use crate::persist::{PersistError, Revision, WriteTarget, Writer};
use crate::ports::ShortcutSlot;
use crate::settings::{Settings, SettingsService, Variant};
use std::sync::Arc;
use std::time::Duration;

struct Memory;
impl WriteTarget<Settings> for Memory {
    fn write(&self, _: &Settings) -> Result<(), PersistError> {
        Ok(())
    }
}

struct Harness {
    slots: HotkeySlots,
    settings: Arc<SettingsService>,
    registrar: FakeHotkeys,
    writer: Writer,
}

impl Harness {
    fn new() -> Self {
        let clock = Arc::new(FakeClock::default());
        let writer = Writer::new(clock, Duration::from_secs(30), Duration::from_secs(1)).unwrap();
        let persisted = writer.register(Memory, Revision(0)).unwrap();
        let settings =
            Arc::new(SettingsService::new(Settings::for_variant(Variant::Dev), persisted).unwrap());
        let slots = HotkeySlots::new(settings.clone());
        Self {
            slots,
            settings,
            registrar: FakeHotkeys::default(),
            writer,
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = self.writer.shutdown();
    }
}

#[test]
fn apply_registers_normalised_chords_and_skips_clipboard_while_recording_is_off() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    let registered = h.registrar.registered.lock().unwrap().clone();
    assert_eq!(
        registered
            .get(&ShortcutSlot::QuickSearch)
            .map(String::as_str),
        Some("Control+Alt+Shift+Space")
    );
    assert_eq!(
        registered.get(&ShortcutSlot::Clipboard).map(String::as_str),
        Some("Control+Alt+Shift+V")
    );
    h.settings
        .update(|settings| settings.clipboard_enabled = false)
        .unwrap();
    h.slots.apply(&h.registrar).unwrap();
    let registered = h.registrar.registered.lock().unwrap().clone();
    assert!(!registered.contains_key(&ShortcutSlot::Clipboard));
    assert!(registered.contains_key(&ShortcutSlot::QuickSearch));
    assert_eq!(
        h.settings.watch().get().clipboard_hotkey,
        "Control+Alt+Shift+V"
    );
}

#[test]
fn colliding_slots_are_rejected_before_the_registrar_runs() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    h.registrar.log.lock().unwrap().clear();
    let before = h.settings.snapshot();
    let error = h
        .slots
        .set(
            ShortcutSlot::Clipboard,
            "Control+Alt+Shift+Space",
            &h.registrar,
        )
        .unwrap_err();
    assert_eq!(error, HotkeyError::Collision);
    assert_eq!(
        error.to_string(),
        "Kudzu already uses that shortcut for something else."
    );
    assert_eq!(h.settings.snapshot(), before);
    assert!(h.registrar.log.lock().unwrap().is_empty());
    let error = h
        .slots
        .set(ShortcutSlot::QuickSearch, "Ctrl+Alt+Shift+V", &h.registrar)
        .unwrap_err();
    assert_eq!(error, HotkeyError::Collision);
}

#[test]
fn a_failed_registration_restores_the_old_chord_and_does_not_persist() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    let before = h.settings.watch().get().palette_hotkey.clone();
    let revision = h.settings.watch().revision();
    h.registrar
        .rejected
        .lock()
        .unwrap()
        .insert("Control+Alt+K".into());
    h.registrar.log.lock().unwrap().clear();
    let error = h
        .slots
        .set(ShortcutSlot::QuickSearch, "Control+Alt+K", &h.registrar)
        .unwrap_err();
    assert_eq!(error, HotkeyError::Taken);
    assert_eq!(h.settings.watch().get().palette_hotkey, before);
    assert_eq!(h.settings.watch().revision(), revision);
    assert_eq!(
        h.registrar
            .registered
            .lock()
            .unwrap()
            .get(&ShortcutSlot::QuickSearch)
            .map(String::as_str),
        Some("Control+Alt+Shift+Space")
    );
    assert_eq!(
        *h.registrar.log.lock().unwrap(),
        [
            HotkeyOp::Unregister(ShortcutSlot::QuickSearch),
            HotkeyOp::Register(ShortcutSlot::QuickSearch, "Control+Alt+K".into()),
            HotkeyOp::Register(ShortcutSlot::QuickSearch, "Control+Alt+Shift+Space".into()),
        ]
    );
}

#[test]
fn a_successful_set_persists_only_after_the_registrar_accepts() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    let revision = h
        .slots
        .set(ShortcutSlot::QuickSearch, "Control+Alt+K", &h.registrar)
        .unwrap();
    assert!(revision > Revision(1));
    assert_eq!(h.settings.watch().get().palette_hotkey, "Control+Alt+K");
    assert_eq!(
        h.registrar
            .registered
            .lock()
            .unwrap()
            .get(&ShortcutSlot::QuickSearch)
            .unwrap(),
        "Control+Alt+K"
    );
}

#[test]
fn clipboard_set_while_recording_is_off_persists_and_stays_unregistered() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    h.settings
        .update(|settings| settings.clipboard_enabled = false)
        .unwrap();
    h.slots.apply(&h.registrar).unwrap();
    h.slots
        .set(ShortcutSlot::Clipboard, "Control+Alt+C", &h.registrar)
        .unwrap();
    assert_eq!(h.settings.watch().get().clipboard_hotkey, "Control+Alt+C");
    assert!(!h
        .registrar
        .registered
        .lock()
        .unwrap()
        .contains_key(&ShortcutSlot::Clipboard));
}

#[test]
fn invalid_chords_are_refused() {
    let h = Harness::new();
    assert_eq!(
        h.slots
            .set(ShortcutSlot::QuickSearch, "Shift+A", &h.registrar),
        Err(HotkeyError::Invalid)
    );
    assert_eq!(
        h.slots.set(ShortcutSlot::QuickSearch, "", &h.registrar),
        Err(HotkeyError::Invalid)
    );
}

#[test]
fn a_rejected_palette_chord_still_applies_the_clipboard_slot() {
    let h = Harness::new();
    h.registrar
        .rejected
        .lock()
        .unwrap()
        .insert("Control+Alt+Shift+Space".into());
    assert_eq!(h.slots.apply(&h.registrar), Err(HotkeyError::Taken));
    let registered = h.registrar.registered.lock().unwrap().clone();
    assert!(!registered.contains_key(&ShortcutSlot::QuickSearch));
    assert_eq!(
        registered.get(&ShortcutSlot::Clipboard).map(String::as_str),
        Some("Control+Alt+Shift+V")
    );
}

#[test]
fn a_rejected_palette_chord_still_clears_clipboard_when_recording_is_off() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    h.settings
        .update(|settings| {
            settings.clipboard_enabled = false;
            settings.palette_hotkey = "Control+Alt+K".into();
        })
        .unwrap();
    h.registrar
        .rejected
        .lock()
        .unwrap()
        .insert("Control+Alt+K".into());
    assert_eq!(h.slots.apply(&h.registrar), Err(HotkeyError::Taken));
    assert!(!h
        .registrar
        .registered
        .lock()
        .unwrap()
        .contains_key(&ShortcutSlot::Clipboard));
}

#[test]
fn a_failed_rollback_is_a_distinct_error() {
    let h = Harness::new();
    h.slots.apply(&h.registrar).unwrap();
    h.registrar
        .rejected
        .lock()
        .unwrap()
        .extend(["Control+Alt+K".into(), "Control+Alt+Shift+Space".into()]);
    assert_eq!(
        h.slots
            .set(ShortcutSlot::QuickSearch, "Control+Alt+K", &h.registrar),
        Err(HotkeyError::Rollback)
    );
}
