//! Global shortcuts: quick search and clipboard history. Ported from the Tauri shell's
//! `hotkey.rs`; the shell registers them with Rune's Carbon hot keys (no Accessibility needed).
//! Rune's parser reads the stored accelerators ("Control+Alt+Space") as they are.

use super::core::{core, Slot};

/// The same chord written two ways ("Cmd+K" and "Command+k") is the same shortcut.
pub fn same(a: &str, b: &str) -> bool {
    match (a.parse::<rune::Shortcut>(), b.parse::<rune::Shortcut>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// Registers `accel` for `slot`, replacing that slot's previous shortcut. On failure the old one stays.
pub fn register(slot: Slot, accel: &str) -> Result<(), String> {
    accel.parse::<rune::Shortcut>().map_err(|e| format!("Invalid shortcut: {e}"))?;
    let core = core();
    let (mine, other) = match slot {
        Slot::Palette => (&core.registered_hotkey, &core.registered_clip_hotkey),
        Slot::Clipboard => (&core.registered_clip_hotkey, &core.registered_hotkey),
    };
    if other.lock().as_deref().is_some_and(|o| same(o, accel)) {
        return Err("Kudzu already uses that shortcut for something else.".into());
    }
    let previous = mine.lock().clone();
    if previous.as_deref() == Some(accel) {
        return Ok(());
    }
    match core.shell().register_hotkey(slot, accel) {
        Ok(()) => {
            *mine.lock() = Some(accel.to_string());
            Ok(())
        }
        Err(e) => {
            if let Some(p) = previous.as_deref() {
                let _ = core.shell().register_hotkey(slot, p);
            }
            Err(format!("Couldn't use that shortcut — another app may own it ({e})."))
        }
    }
}

/// Removes `slot`'s shortcut (clipboard history turned off).
pub fn unregister(slot: Slot) {
    let core = core();
    let mine = match slot {
        Slot::Palette => &core.registered_hotkey,
        Slot::Clipboard => &core.registered_clip_hotkey,
    };
    if mine.lock().take().is_some() {
        core.shell().unregister_hotkey(slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_accelerators_parse_and_compare_by_keys() {
        for accel in ["Control+Alt+Space", "Control+Alt+V", "Control+Alt+Shift+Space", "Command+Shift+Semicolon"] {
            assert!(accel.parse::<rune::Shortcut>().is_ok(), "{accel}");
        }
        assert!(same("Control+Alt+V", "ctrl+option+v"));
        assert!(!same("Control+Alt+V", "Control+Alt+Space"));
    }
}
