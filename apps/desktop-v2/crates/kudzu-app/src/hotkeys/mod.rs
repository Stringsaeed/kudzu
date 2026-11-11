//! The two global shortcut slots: validation, collision, rollback, persist after success.

mod chord;

use crate::error::PortError;
use crate::ports::{HotkeyRegistrar, ShortcutSlot};
use crate::settings::{Settings, SettingsError, SettingsService};
use crate::watch::Revision;
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HotkeyError {
    #[error("Kudzu already uses that shortcut for something else.")]
    Collision,
    #[error("Invalid shortcut.")]
    Invalid,
    #[error("Couldn't use that shortcut — another app may own it.")]
    Taken,
    #[error("Couldn't restore the previous shortcut.")]
    Rollback,
    #[error(transparent)]
    Settings(#[from] SettingsError),
}

impl From<PortError> for HotkeyError {
    fn from(_: PortError) -> Self {
        Self::Taken
    }
}

#[derive(Default)]
struct Bound {
    palette: Option<String>,
    clipboard: Option<String>,
}

/// Owns collision and rollback. The registrar is passed in because it is main-thread-only.
pub struct HotkeySlots {
    settings: Arc<SettingsService>,
    bound: Mutex<Bound>,
}

impl HotkeySlots {
    pub fn new(settings: Arc<SettingsService>) -> Self {
        Self {
            settings,
            bound: Mutex::new(Bound::default()),
        }
    }

    pub fn settings(&self) -> Arc<Settings> {
        self.settings.watch().get()
    }

    /// Register the saved chords. Both slots run; the first error is returned afterwards.
    /// Clipboard is left unbound while recording is off.
    pub fn apply(&self, registrar: &dyn HotkeyRegistrar) -> Result<(), HotkeyError> {
        let settings = self.settings.watch().get();
        let palette = self.bind(
            ShortcutSlot::QuickSearch,
            &settings.palette_hotkey,
            registrar,
        );
        let clipboard = if settings.clipboard_enabled {
            self.bind(
                ShortcutSlot::Clipboard,
                &settings.clipboard_hotkey,
                registrar,
            )
        } else {
            self.clear(ShortcutSlot::Clipboard, registrar);
            Ok(())
        };
        match (palette, clipboard) {
            (Err(error), _) => Err(error),
            (Ok(()), result) => result,
        }
    }

    /// Persist only after the registrar accepts the chord. A failure restores the previous bind.
    pub fn set(
        &self,
        slot: ShortcutSlot,
        chord: &str,
        registrar: &dyn HotkeyRegistrar,
    ) -> Result<Revision, HotkeyError> {
        let identity = chord::identity(chord).ok_or(HotkeyError::Invalid)?;
        let settings = self.settings.watch().get();
        let other = match slot {
            ShortcutSlot::QuickSearch => settings.clipboard_hotkey.as_str(),
            ShortcutSlot::Clipboard => settings.palette_hotkey.as_str(),
        };
        if chord::identity(other).as_ref() == Some(&identity) {
            return Err(HotkeyError::Collision);
        }
        let previous = match slot {
            ShortcutSlot::QuickSearch => settings.palette_hotkey.clone(),
            ShortcutSlot::Clipboard => settings.clipboard_hotkey.clone(),
        };
        if slot == ShortcutSlot::Clipboard && !settings.clipboard_enabled {
            let revision = self.persist(slot, chord)?;
            self.clear(slot, registrar);
            return Ok(revision);
        }
        if let Err(error) = self.bind(slot, chord, registrar) {
            self.restore(slot, &previous, registrar)?;
            return Err(error);
        }
        match self.persist(slot, chord) {
            Ok(revision) => Ok(revision),
            Err(error) => {
                self.restore(slot, &previous, registrar)?;
                Err(error)
            }
        }
    }

    fn restore(
        &self,
        slot: ShortcutSlot,
        previous: &str,
        registrar: &dyn HotkeyRegistrar,
    ) -> Result<(), HotkeyError> {
        if self.bind(slot, previous, registrar).is_err() {
            log::error!("hotkey rollback failed");
            return Err(HotkeyError::Rollback);
        }
        Ok(())
    }

    fn persist(&self, slot: ShortcutSlot, chord: &str) -> Result<Revision, HotkeyError> {
        Ok(self.settings.update(|settings| match slot {
            ShortcutSlot::QuickSearch => settings.palette_hotkey = chord.into(),
            ShortcutSlot::Clipboard => settings.clipboard_hotkey = chord.into(),
        })?)
    }

    fn bind(
        &self,
        slot: ShortcutSlot,
        chord: &str,
        registrar: &dyn HotkeyRegistrar,
    ) -> Result<(), HotkeyError> {
        let identity = chord::identity(chord).ok_or(HotkeyError::Invalid)?;
        let current = {
            let mut bound = self.bound.lock().unwrap_or_else(PoisonError::into_inner);
            match slot {
                ShortcutSlot::QuickSearch => bound.palette.take(),
                ShortcutSlot::Clipboard => bound.clipboard.take(),
            }
        };
        if current.as_deref() == Some(chord) {
            let mut bound = self.bound.lock().unwrap_or_else(PoisonError::into_inner);
            match slot {
                ShortcutSlot::QuickSearch => bound.palette = current,
                ShortcutSlot::Clipboard => bound.clipboard = current,
            }
            return Ok(());
        }
        if current.is_some() {
            let _ = registrar.unregister(slot);
        }
        registrar.register(slot, &identity)?;
        let mut bound = self.bound.lock().unwrap_or_else(PoisonError::into_inner);
        match slot {
            ShortcutSlot::QuickSearch => bound.palette = Some(chord.into()),
            ShortcutSlot::Clipboard => bound.clipboard = Some(chord.into()),
        }
        Ok(())
    }

    fn clear(&self, slot: ShortcutSlot, registrar: &dyn HotkeyRegistrar) {
        let mut bound = self.bound.lock().unwrap_or_else(PoisonError::into_inner);
        let was = match slot {
            ShortcutSlot::QuickSearch => bound.palette.take(),
            ShortcutSlot::Clipboard => bound.clipboard.take(),
        };
        drop(bound);
        if was.is_some() {
            let _ = registrar.unregister(slot);
        }
    }
}

#[cfg(test)]
mod tests;
