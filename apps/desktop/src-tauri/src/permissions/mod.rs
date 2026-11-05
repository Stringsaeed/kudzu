//! Permission companion. OS approval remains in System Settings; this window guides the handoff.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    #[default]
    Accessibility,
    InputMonitoring,
}

#[derive(Default)]
pub struct Selection(Mutex<PermissionKind>);

const RESUME_FILE: &str = "permission-setup-pending";

pub fn resume_after_restart(app: &AppHandle) {
    let Ok(dir) = app.path().app_data_dir() else { return };
    if std::fs::remove_file(dir.join(RESUME_FILE)).is_err() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = show_permissions(app, PermissionKind::Accessibility).await {
            log::warn!("resume permission setup: {error}");
        }
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    kind: PermissionKind,
    app_name: String,
    accessibility_name: &'static str,
}

#[tauri::command]
pub fn permission_context(app: AppHandle) -> Context {
    let accessibility_name = "Accessibility";
    #[cfg(target_os = "macos")]
    // SAFETY: NSProcessInfo's immutable OS version can be read from any thread.
    let accessibility_name =
        if unsafe { kudzu_permission_uses_device_control_label() } { "Device Control and Data Access" } else { accessibility_name };
    Context {
        kind: *app.state::<Selection>().0.lock(),
        app_name: app.config().product_name.clone().unwrap_or_else(|| "Kudzu".into()),
        accessibility_name,
    }
}

async fn on_main<T: Send + 'static>(app: &AppHandle, action: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = send.send(action());
    })
    .map_err(|e| e.to_string())?;
    receive.await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn show_permissions(app: AppHandle, kind: PermissionKind) -> Result<(), String> {
    let handle = app.clone();
    on_main(&app, move || {
        *handle.state::<Selection>().0.lock() = kind;
        let window = match handle.get_webview_window("permissions") {
            Some(window) => window,
            None => {
                let path = if tauri::is_dev() { "permissions" } else { "permissions.html" };
                let window = WebviewWindowBuilder::new(&handle, "permissions", WebviewUrl::App(path.into()))
                    .title("Kudzu permissions")
                    .inner_size(420.0, 660.0)
                    .decorations(false)
                    .transparent(true)
                    .resizable(false)
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .visible(false)
                    .focused(false)
                    .visible_on_all_workspaces(true)
                    .build()
                    .map_err(|e| e.to_string())?;
                window.center().map_err(|e| e.to_string())?;
                crate::windows::round_corners(&window, 18.0);
                crate::windows::apply_theme(&handle, crate::state::core(&handle).store.lock().settings.theme);
                window
            }
        };
        stop_guide(&window);
        window.emit("kudzu://permission-show", kind).map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())
    })
    .await
}

fn companion(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "permissions" {
        Ok(())
    } else {
        Err("This action belongs to the permission companion.".into())
    }
}

#[cfg(target_os = "macos")]
extern "C" {
    fn kudzu_permission_drag_icon(window: *mut std::ffi::c_void, x: f64, y: f64, width: f64, height: f64) -> bool;
    fn kudzu_permission_open_settings(window: *mut std::ffi::c_void, accessibility: bool) -> bool;
    fn kudzu_permission_stop_guide(window: *mut std::ffi::c_void);
    fn kudzu_permission_reveal_app() -> bool;
    fn kudzu_permission_uses_device_control_label() -> bool;
}

#[tauri::command]
pub async fn install_permission_icon(window: WebviewWindow, x: f64, y: f64, width: f64, height: f64) -> Result<bool, String> {
    companion(&window)?;
    if ![x, y, width, height].iter().all(|n| n.is_finite()) || width < 1.0 || height < 1.0 || width > 128.0 || height > 128.0 {
        return Err("Invalid app icon bounds".into());
    }
    let app = window.app_handle().clone();
    on_main(&app, move || {
        #[cfg(target_os = "macos")]
        // SAFETY: Tauri owns this live NSWindow; AppKit runs on the main thread.
        unsafe {
            Ok(kudzu_permission_drag_icon(window.ns_window().map_err(|e| e.to_string())?, x, y, width, height))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Ok(false)
        }
    })
    .await
}

#[tauri::command]
pub async fn begin_permission(window: WebviewWindow, kind: PermissionKind) -> Result<(), String> {
    companion(&window)?;
    let app = window.app_handle().clone();
    on_main(&app, move || {
        match kind {
            PermissionKind::Accessibility => kudzu_engine::system::request_accessibility(),
            PermissionKind::InputMonitoring => kudzu_engine::system::request_input_monitoring(),
        }
        #[cfg(target_os = "macos")]
        // SAFETY: Tauri owns this live NSWindow; AppKit runs on the main thread.
        unsafe {
            if kudzu_permission_open_settings(window.ns_window().map_err(|e| e.to_string())?, matches!(kind, PermissionKind::Accessibility))
            {
                return Ok(());
            }
        }
        Err("Could not open System Settings. Open Privacy & Security from the Apple menu.".into())
    })
    .await
}

pub fn stop_guide(_window: &WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(pointer) = _window.ns_window() {
        // SAFETY: caller is on the main thread and Tauri owns the window.
        unsafe { kudzu_permission_stop_guide(pointer) };
    }
}

#[tauri::command]
pub async fn stop_permission_guide(window: WebviewWindow) -> Result<(), String> {
    companion(&window)?;
    let app = window.app_handle().clone();
    on_main(&app, move || {
        stop_guide(&window);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn close_permissions(window: WebviewWindow) -> Result<(), String> {
    companion(&window)?;
    let app = window.app_handle().clone();
    on_main(&app, move || {
        stop_guide(&window);
        window.hide().map_err(|e| e.to_string())?;
        crate::windows::show_main(window.app_handle());
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn restart_permission_app(window: WebviewWindow) -> Result<(), String> {
    companion(&window)?;
    let app = window.app_handle().clone();
    on_main(&app, move || {
        stop_guide(&window);
        let app = window.app_handle();
        let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
        std::fs::write(dir.join(RESUME_FILE), []).map_err(|e| e.to_string())?;
        app.request_restart();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn reveal_permission_app(window: WebviewWindow) -> Result<(), String> {
    companion(&window)?;
    let app = window.app_handle().clone();
    on_main(&app, move || {
        stop_guide(&window);
        #[cfg(target_os = "macos")]
        // SAFETY: AppKit runs on the main thread.
        if unsafe { kudzu_permission_reveal_app() } {
            return Ok(());
        }
        Err("Run the installed Kudzu application to reveal it in Finder.".into())
    })
    .await
}
