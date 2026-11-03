//! Window management: the main library window, the quick-search palette and the caret form.

use kudzu_engine::{system, Rect};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::{core, EV_PALETTE_SHOW};

pub const PALETTE_W: f64 = 760.0;
pub const PALETTE_H: f64 = 480.0;
pub const FORM_W: f64 = 440.0;

#[cfg(target_os = "macos")]
fn floating_effects(radius: f64) -> tauri::utils::config::WindowEffectsConfig {
    use tauri::window::{Effect, EffectState};
    tauri::utils::config::WindowEffectsConfig {
        // Popover follows the window's appearance (set by `apply_theme`); HudWindow is always dark.
        effects: vec![Effect::Popover],
        state: Some(EffectState::Active),
        radius: Some(radius),
        color: None,
        ..Default::default()
    }
}

fn floating(app: &AppHandle, label: &str, url: &str, w: f64, h: f64) -> tauri::Result<WebviewWindow> {
    // The dev server routes `/palette`; the static export writes `palette.html`.
    let path = if cfg!(debug_assertions) { url.to_string() } else { format!("{url}.html") };
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App(path.into()))
        .title("Kudzu")
        .inner_size(w, h)
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .visible_on_all_workspaces(true);
    #[cfg(target_os = "macos")]
    let builder = builder.effects(floating_effects(if label == "palette" { PALETTE_RADIUS } else { FORM_RADIUS })).shadow(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.shadow(false);
    builder.build()
}

/// Clips a borderless window to a rounded rect and recomputes its shadow from that shape.
///
/// A transparent NSWindow's shadow is derived from its content's alpha. The vibrancy view is
/// rounded, but the content view underneath is a full rectangle, so macOS draws a square shadow
/// edge around the corners. Rounding and masking the content view's layer fixes the shape;
/// `invalidateShadow` makes AppKit recompute it. Must run on the main thread.
#[cfg(target_os = "macos")]
pub fn round_corners(win: &WebviewWindow, radius: f64) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    let Ok(ptr) = win.ns_window() else { return };
    if ptr.is_null() {
        return;
    }
    // SAFETY: `ptr` is this window's live NSWindow, and we're on the main thread.
    unsafe {
        let window = ptr as *mut AnyObject;
        let content: *mut AnyObject = msg_send![window, contentView];
        if content.is_null() {
            return;
        }
        let _: () = msg_send![content, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![content, layer];
        if !layer.is_null() {
            let _: () = msg_send![layer, setCornerRadius: radius];
            let _: () = msg_send![layer, setMasksToBounds: true];
        }
        let _: () = msg_send![window, invalidateShadow];
    }
}

#[cfg(not(target_os = "macos"))]
pub fn round_corners(_win: &WebviewWindow, _radius: f64) {}

const PALETTE_RADIUS: f64 = 18.0;
const FORM_RADIUS: f64 = 16.0;

/// Points every window's native appearance at the user's theme, so translucent materials
/// (sidebar, palette, form) and the webviews' `prefers-color-scheme` match the app.
pub fn apply_theme(app: &AppHandle, theme: crate::settings::Theme) {
    use crate::settings::Theme;
    let native = match theme {
        Theme::Dark => Some(tauri::Theme::Dark),
        Theme::Light => Some(tauri::Theme::Light),
        Theme::System => None,
    };
    for label in ["main", "palette", "form"] {
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.set_theme(native);
        }
    }
}

/// Creates the palette and form windows up front so they open instantly.
pub fn prewarm(app: &AppHandle) {
    if app.get_webview_window("palette").is_none() {
        match floating(app, "palette", "palette", PALETTE_W, PALETTE_H) {
            Ok(w) => round_corners(&w, PALETTE_RADIUS),
            Err(e) => log::error!("palette window: {e}"),
        }
    }
    if app.get_webview_window("form").is_none() {
        match floating(app, "form", "form", FORM_W, 360.0) {
            Ok(w) => round_corners(&w, FORM_RADIUS),
            Err(e) => log::error!("form window: {e}"),
        }
    }
}

/// The monitor under the mouse, as (x, y, w, h) in logical points.
fn active_monitor(app: &AppHandle, win: &WebviewWindow) -> Option<(f64, f64, f64, f64)> {
    let cursor = app.cursor_position().ok();
    let monitors = win.available_monitors().ok()?;
    let pick = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let p = m.position();
                let s = m.size();
                c.x >= p.x as f64 && c.x < (p.x + s.width as i32) as f64 && c.y >= p.y as f64 && c.y < (p.y + s.height as i32) as f64
            })
        })
        .or_else(|| monitors.first())?;
    let scale = pick.scale_factor();
    let p = pick.position().to_logical::<f64>(scale);
    let s = pick.size().to_logical::<f64>(scale);
    Some((p.x, p.y, s.width, s.height))
}

pub fn toggle_palette(app: &AppHandle) {
    let Some(win) = app.get_webview_window("palette") else {
        prewarm(app);
        return;
    };
    if win.is_visible().unwrap_or(false) {
        hide_palette(app, true);
        return;
    }
    let core = core(app);
    // Remember where the user was, unless it's Kudzu itself.
    let front = system::frontmost_app().filter(|a| a.pid != Some(std::process::id() as i32));
    *core.palette_target.lock() = front.clone();
    if let Some(engine) = core.engine() {
        engine.set_suppressed(true);
    }
    if let Some((x, y, w, h)) = active_monitor(app, &win) {
        let _ = win.set_position(LogicalPosition::new(x + (w - PALETTE_W) / 2.0, y + h * 0.2));
    }
    let _ = win.emit(EV_PALETTE_SHOW, serde_json::json!({ "target": front }));
    let _ = win.show();
    round_corners(&win, PALETTE_RADIUS);
    let _ = win.set_focus();
}

/// Hides the palette; with `restore`, hands focus back to the app the user came from.
pub fn hide_palette(app: &AppHandle, restore: bool) {
    let core = core(app);
    if let Some(win) = app.get_webview_window("palette") {
        if !win.is_visible().unwrap_or(false) {
            return;
        }
        let _ = win.hide();
    }
    if core.pending_form.lock().is_none() {
        if let Some(engine) = core.engine() {
            engine.set_suppressed(false);
        }
    }
    if restore {
        let main_visible = app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
        if !main_visible {
            if let Some(target) = core.palette_target.lock().clone() {
                system::activate_app(&target);
            }
        }
    }
}

pub fn show_form(app: &AppHandle, caret: Option<Rect>) {
    let Some(win) = app.get_webview_window("form") else {
        prewarm(app);
        return;
    };
    let monitor = active_monitor(app, &win);
    let size = win.inner_size().ok().map(|s| s.to_logical::<f64>(win.scale_factor().unwrap_or(1.0)));
    let h = size.map(|s| s.height).unwrap_or(360.0);
    let (x, y) = match (caret, monitor) {
        (Some(c), Some((mx, my, mw, mh))) => {
            let mut x = c.x - 24.0;
            let mut y = c.y + c.height + 6.0;
            if y + h > my + mh {
                y = (c.y - h - 6.0).max(my);
            }
            x = x.clamp(mx + 8.0, mx + mw - FORM_W - 8.0);
            (x, y)
        }
        (_, Some((mx, my, mw, mh))) => (mx + (mw - FORM_W) / 2.0, my + mh * 0.28),
        _ => (200.0, 200.0),
    };
    let _ = win.set_position(LogicalPosition::new(x, y));
    let _ = win.show();
    round_corners(&win, FORM_RADIUS);
    let _ = win.set_focus();
}

pub fn hide_form(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("form") {
        let _ = win.hide();
    }
}

pub fn resize_form(app: &AppHandle, height: f64) {
    if let Some(win) = app.get_webview_window("form") {
        let _ = win.set_size(LogicalSize::new(FORM_W, height.clamp(160.0, 680.0)));
        // The shadow is cached per shape; a new height needs a new one.
        let w = win.clone();
        let _ = app.run_on_main_thread(move || round_corners(&w, FORM_RADIUS));
    }
}

/// Shows (and focuses) the main window, adding the Dock icon if the user wants it.
pub fn show_main(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let dock = core(app).store.lock().settings.show_in_dock;
        let _ = app.set_activation_policy(if dock { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory });
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Hides the main window and drops back to a menu-bar-only app.
pub fn hide_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
}
