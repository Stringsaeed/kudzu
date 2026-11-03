//! The global quick-search shortcut.

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::state::core;

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

/// Registers `accel` as the palette shortcut, replacing the previous one. On failure the old one stays.
pub fn register(app: &AppHandle, accel: &str) -> Result<(), String> {
    let gs = app.global_shortcut();
    let shortcut: Shortcut = normalize(accel).parse().map_err(|e| format!("Invalid shortcut: {e}"))?;
    let core = core(app);
    let previous = core.registered_hotkey.lock().clone();
    if previous.as_deref() == Some(accel) && gs.is_registered(shortcut) {
        return Ok(());
    }
    if let Some(prev) = &previous {
        if let Ok(p) = normalize(prev).parse::<Shortcut>() {
            let _ = gs.unregister(p);
        }
    }
    match gs.register(shortcut) {
        Ok(()) => {
            *core.registered_hotkey.lock() = Some(accel.to_string());
            Ok(())
        }
        Err(e) => {
            if let Some(prev) = previous {
                if let Ok(p) = normalize(&prev).parse::<Shortcut>() {
                    let _ = gs.register(p);
                }
            }
            Err(format!("Couldn't use that shortcut — another app may own it ({e})."))
        }
    }
}
