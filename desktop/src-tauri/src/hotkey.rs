//! Global shortcuts: quick search and clipboard history.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::state::core;

/// Which window a global shortcut opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Palette,
    Clipboard,
}

/// Normalizes the UI's accelerator ("Control+Alt+Space", "Command+Shift+Semicolon") for the plugin.
fn normalize(accel: &str) -> String {
    accel
        .split('+')
        .map(|k| match k {
            "Command" | "Cmd" | "Meta" => "Super",
            other => other,
        })
        .collect::<Vec<_>>()
        .join("+")
}

fn parse(accel: &str) -> Option<Shortcut> {
    normalize(accel).parse().ok()
}

/// The slot a pressed shortcut belongs to.
pub fn slot_of(app: &AppHandle, pressed: &Shortcut) -> Slot {
    let core = core(app);
    let clip = core.registered_clip_hotkey.lock().clone();
    if clip.as_deref().and_then(parse).is_some_and(|s| s.id() == pressed.id()) {
        Slot::Clipboard
    } else {
        Slot::Palette
    }
}

/// Registers `accel` for `slot`, replacing that slot's previous shortcut. On failure the old one stays.
pub fn register(app: &AppHandle, slot: Slot, accel: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let shortcut: Shortcut = normalize(accel).parse().map_err(|e| format!("Invalid shortcut: {e}"))?;
    let core = core(app);
    let (mine, other) = match slot {
        Slot::Palette => (&core.registered_hotkey, &core.registered_clip_hotkey),
        Slot::Clipboard => (&core.registered_clip_hotkey, &core.registered_hotkey),
    };
    if other.lock().as_deref().and_then(parse).is_some_and(|s| s.id() == shortcut.id()) {
        return Err("Kudzu already uses that shortcut for something else.".into());
    }
    let previous = mine.lock().clone();
    if previous.as_deref() == Some(accel) && gs.is_registered(shortcut) {
        return Ok(());
    }
    if let Some(p) = previous.as_deref().and_then(parse) {
        let _ = gs.unregister(p);
    }
    match gs.register(shortcut) {
        Ok(()) => {
            *mine.lock() = Some(accel.to_string());
            Ok(())
        }
        Err(e) => {
            if let Some(p) = previous.as_deref().and_then(parse) {
                let _ = gs.register(p);
            }
            Err(format!("Couldn't use that shortcut — another app may own it ({e})."))
        }
    }
}

/// Removes `slot`'s shortcut (clipboard history turned off).
pub fn unregister(app: &AppHandle, slot: Slot) {
    let core = core(app);
    let mine = match slot {
        Slot::Palette => &core.registered_hotkey,
        Slot::Clipboard => &core.registered_clip_hotkey,
    };
    let previous = mine.lock().take();
    if let Some(p) = previous.as_deref().and_then(parse) {
        let _ = app.global_shortcut().unregister(p);
    }
}
