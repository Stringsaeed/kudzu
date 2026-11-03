//! Menu bar icon and menu.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use kudzu_engine::{EngineStatus, TapMode};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::state::{core, EV_NAVIGATE, EV_NEW_SNIPPET};
use crate::updater::UpdateState;
use crate::windows;

const TRAY_ID: &str = "kudzu";

struct Items {
    status: MenuItem<Wry>,
    pause: MenuItem<Wry>,
    search: MenuItem<Wry>,
    updates: MenuItem<Wry>,
}

/// Which menu-bar glyph to show (the same set as the native app's).
#[derive(Clone, Copy)]
enum Glyph {
    Normal = 0,
    Paused = 1,
    SecureInput = 2,
    NeedsPermission = 3,
}

/// The glyph last shown, so the update dot can be added or removed without engine status.
static GLYPH: AtomicU8 = AtomicU8::new(Glyph::Normal as u8);
/// Whether a downloaded update is waiting (draws a dot on the glyph).
pub(crate) static UPDATE_DOT: AtomicBool = AtomicBool::new(false);

fn icon(glyph: Glyph) -> Image<'static> {
    let bytes: &'static [u8] = match glyph {
        Glyph::Normal => include_bytes!("../icons/tray.png"),
        Glyph::Paused => include_bytes!("../icons/tray-paused.png"),
        Glyph::SecureInput => include_bytes!("../icons/tray-secure.png"),
        Glyph::NeedsPermission => include_bytes!("../icons/tray-permissions.png"),
    };
    let image = Image::from_bytes(bytes).expect("tray icon");
    if UPDATE_DOT.load(Ordering::Relaxed) {
        with_dot(&image)
    } else {
        image
    }
}

/// Adds a small filled dot in the top-right corner. Template images use alpha only, so the
/// dot takes the menu bar's text colour, like the system's own badges. A clear ring separates
/// it from the glyph underneath.
fn with_dot(image: &Image<'_>) -> Image<'static> {
    let (w, h) = (image.width() as i64, image.height() as i64);
    let mut rgba = image.rgba().to_vec();
    let r = (w.min(h) as f64 * 0.17).max(2.0);
    let (cx, cy) = (w as f64 - r - 0.5, r + 0.5);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
            let i = ((y * w + x) * 4) as usize;
            if d <= r {
                rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 255]);
            } else if d <= r + r * 0.45 {
                rgba[i + 3] = 0;
            }
        }
    }
    Image::new_owned(rgba, w as u32, h as u32)
}

fn glyph_from(n: u8) -> Glyph {
    match n {
        1 => Glyph::Paused,
        2 => Glyph::SecureInput,
        3 => Glyph::NeedsPermission,
        _ => Glyph::Normal,
    }
}

fn refresh_icon(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon(glyph_from(GLYPH.load(Ordering::Relaxed)))));
        let _ = tray.set_icon_as_template(true);
    }
}

fn glyph_for(status: &EngineStatus) -> Glyph {
    if status.paused {
        Glyph::Paused
    } else if status.secure_input {
        Glyph::SecureInput
    } else if status.tap == TapMode::Off || !status.permissions.accessibility {
        Glyph::NeedsPermission
    } else {
        Glyph::Normal
    }
}

fn hotkey_label(accel: &str) -> String {
    accel
        .split('+')
        .map(|k| match k.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => "⌃".to_string(),
            "alt" | "option" => "⌥".to_string(),
            "shift" => "⇧".to_string(),
            "command" | "cmd" | "super" | "meta" | "cmdorctrl" | "commandorcontrol" => "⌘".to_string(),
            other => other.trim_start_matches("key").to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("")
}

pub fn summary(status: &EngineStatus) -> String {
    if status.paused {
        "Paused".into()
    } else if status.secure_input {
        match &status.secure_input_owner {
            Some(app) => format!("Blocked — Secure Input in {app}"),
            None => "Blocked — Secure Input is on".into(),
        }
    } else if status.tap == TapMode::Off {
        "Needs permission to work".into()
    } else {
        "Expanding".into()
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let (paused, hotkey) = {
        let c = core(app);
        let s = c.store.lock();
        (s.settings.paused, s.settings.palette_hotkey.clone())
    };
    let status = MenuItem::with_id(app, "status", "Starting…", false, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", if paused { "Resume Expanding" } else { "Pause Expanding" }, true, Some("CmdOrCtrl+Shift+P"))?;
    let search = MenuItem::with_id(app, "search", format!("Quick Search      {}", hotkey_label(&hotkey)), true, None::<&str>)?;
    let from_sel = MenuItem::with_id(app, "from-selection", "New Snippet from Selection", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Kudzu…", true, Some("CmdOrCtrl+O"))?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let updates = MenuItem::with_id(app, "updates", "Check for Updates…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Kudzu", true, Some("CmdOrCtrl+Q"))?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &search,
            &from_sel,
            &pause,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &settings,
            &updates,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    app.manage(parking_lot::Mutex::new(Items { status, pause, search, updates }));

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon(if paused { Glyph::Paused } else { Glyph::Normal }))
        .icon_as_template(true)
        .tooltip("Kudzu")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "pause" => {
                let core = core(app);
                let paused = {
                    let mut s = core.store.lock();
                    s.settings.paused = !s.settings.paused;
                    s.settings.paused
                };
                core.settings_changed();
                if let Some(engine) = core.engine() {
                    engine.set_paused(paused);
                    update(app, &engine.status());
                }
            }
            "search" => windows::toggle_palette(app),
            "from-selection" => {
                let app = app.clone();
                std::thread::spawn(move || {
                    let text = kudzu_engine::system::copy_selection().unwrap_or_default();
                    let a = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        windows::show_main(&a);
                        let _ = a.emit(EV_NEW_SNIPPET, serde_json::json!({ "content": text }));
                    });
                });
            }
            "open" => windows::show_main(app),
            "settings" => {
                windows::show_main(app);
                let _ = app.emit(EV_NAVIGATE, serde_json::json!({ "view": { "kind": "settings" } }));
            }
            "updates" => match crate::updater::state(app) {
                // Downloaded and verified: this item reads "Restart to Update to x.y.z".
                UpdateState::Ready { .. } => {
                    tauri::async_runtime::spawn(crate::updater::restart_to_update(app.clone()));
                }
                state => {
                    windows::show_main(app);
                    let _ = app.emit(EV_NAVIGATE, serde_json::json!({ "view": { "kind": "settings", "section": "about" } }));
                    if !matches!(state, UpdateState::Downloading { .. } | UpdateState::Checking) {
                        tauri::async_runtime::spawn(crate::updater::check(app.clone(), false));
                    }
                }
            },
            "quit" => {
                if let Some(engine) = core(app).engine() {
                    engine.stop();
                }
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Reflects engine status in the icon and menu. Must run on the main thread.
pub fn update(app: &AppHandle, status: &EngineStatus) {
    let glyph = glyph_for(status);
    GLYPH.store(glyph as u8, Ordering::Relaxed);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(icon(glyph)));
        let _ = tray.set_icon_as_template(true);
        let _ = tray.set_tooltip(Some(format!("Kudzu — {}", summary(status))));
    }
    if let Some(items) = app.try_state::<parking_lot::Mutex<Items>>() {
        let items = items.lock();
        let _ = items.status.set_text(summary(status));
        let _ = items.pause.set_text(if status.paused { "Resume Expanding" } else { "Pause Expanding" });
    }
}

/// Updates the hotkey hint next to "Quick Search".
pub fn set_hotkey_hint(app: &AppHandle, accel: &str) {
    if let Some(items) = app.try_state::<parking_lot::Mutex<Items>>() {
        let _ = items.lock().search.set_text(format!("Quick Search      {}", hotkey_label(accel)));
    }
}

/// Mirrors the update state in the menu ("Update to 0.3.0…").
pub fn set_update(app: &AppHandle, state: &UpdateState) {
    let label = match state {
        UpdateState::Ready { version, .. } => format!("Restart to Update to {version}"),
        UpdateState::Downloading { version, .. } => format!("Downloading {version}…"),
        UpdateState::Restarting { .. } => "Restarting…".into(),
        UpdateState::Checking => "Checking for Updates…".into(),
        _ => "Check for Updates…".into(),
    };
    if let Some(items) = app.try_state::<parking_lot::Mutex<Items>>() {
        let _ = items.lock().updates.set_text(label);
    }
    // A dot on the menu-bar glyph while an update is waiting.
    let ready = matches!(state, UpdateState::Ready { .. });
    UPDATE_DOT.store(ready, Ordering::Relaxed);
    refresh_icon(app);
}
