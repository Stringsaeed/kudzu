//! Permission companion. OS approval remains in System Settings; this window guides the handoff.
//! Ported from the Tauri shell's `permissions/mod.rs`. The native half (`macos.m`: the
//! draggable app icon, the Settings guide) is the same file, built by this crate's build.rs.
//! Every function here touches AppKit, so call them on the main thread (Rune handlers run there)
//! with the companion window's `NSWindow`.

use std::ffi::c_void;

use parking_lot::Mutex;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PermissionKind {
    #[default]
    Accessibility,
    InputMonitoring,
}

/// Which permission the companion is guiding.
pub static SELECTION: Mutex<PermissionKind> = Mutex::new(PermissionKind::Accessibility);

const RESUME_FILE: &str = "permission-setup-pending";

/// A restart asked for mid-setup: true once, so the companion reopens after relaunch.
pub fn take_resume_after_restart() -> bool {
    std::fs::remove_file(super::identity::data_dir().join(RESUME_FILE)).is_ok()
}

#[derive(Debug, Clone)]
pub struct Context {
    pub kind: PermissionKind,
    pub app_name: String,
    pub accessibility_name: &'static str,
}

pub fn permission_context() -> Context {
    let accessibility_name = "Accessibility";
    #[cfg(target_os = "macos")]
    // SAFETY: NSProcessInfo's immutable OS version can be read from any thread.
    let accessibility_name =
        if unsafe { kudzu_permission_uses_device_control_label() } { "Device Control and Data Access" } else { accessibility_name };
    Context { kind: *SELECTION.lock(), app_name: super::identity::get().name.to_string(), accessibility_name }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn kudzu_permission_drag_icon(window: *mut c_void, x: f64, y: f64, width: f64, height: f64) -> bool;
    fn kudzu_permission_open_settings(window: *mut c_void, accessibility: bool) -> bool;
    fn kudzu_permission_stop_guide(window: *mut c_void);
    fn kudzu_permission_reveal_app() -> bool;
    fn kudzu_permission_uses_device_control_label() -> bool;
}

/// Puts the draggable app icon over the companion's icon slot (window points).
pub fn install_permission_icon(window: *mut c_void, x: f64, y: f64, width: f64, height: f64) -> Result<bool, String> {
    if window.is_null() {
        return Err("The companion window isn't open.".into());
    }
    if ![x, y, width, height].iter().all(|n| n.is_finite()) || width < 1.0 || height < 1.0 || width > 128.0 || height > 128.0 {
        return Err("Invalid app icon bounds".into());
    }
    #[cfg(target_os = "macos")]
    // SAFETY: Rune owns this live NSWindow; we're on the main thread.
    unsafe {
        Ok(kudzu_permission_drag_icon(window, x, y, width, height))
    }
    #[cfg(not(target_os = "macos"))]
    Ok(false)
}

pub fn begin_permission(window: *mut c_void, kind: PermissionKind) -> Result<(), String> {
    match kind {
        PermissionKind::Accessibility => kudzu_engine::system::request_accessibility(),
        PermissionKind::InputMonitoring => kudzu_engine::system::request_input_monitoring(),
    }
    #[cfg(target_os = "macos")]
    // SAFETY: Rune owns this live NSWindow; we're on the main thread.
    unsafe {
        if !window.is_null() && kudzu_permission_open_settings(window, matches!(kind, PermissionKind::Accessibility)) {
            return Ok(());
        }
    }
    let _ = window;
    Err("Could not open System Settings. Open Privacy & Security from the Apple menu.".into())
}

pub fn stop_guide(window: *mut c_void) {
    #[cfg(target_os = "macos")]
    if !window.is_null() {
        // SAFETY: caller is on the main thread and Rune owns the window.
        unsafe { kudzu_permission_stop_guide(window) };
    }
    let _ = window;
}

/// Leaves a marker so the companion reopens, then relaunches the app.
pub fn restart_permission_app(window: *mut c_void) -> Result<(), String> {
    stop_guide(window);
    std::fs::write(super::identity::data_dir().join(RESUME_FILE), []).map_err(|e| e.to_string())?;
    relaunch()
}

pub fn reveal_permission_app(window: *mut c_void) -> Result<(), String> {
    stop_guide(window);
    #[cfg(target_os = "macos")]
    // SAFETY: AppKit runs on the main thread.
    if unsafe { kudzu_permission_reveal_app() } {
        return Ok(());
    }
    Err("Run the installed Kudzu application to reveal it in Finder.".into())
}

/// Starts a new copy once this one has exited, then quits (Tauri's `request_restart`).
pub fn relaunch() -> Result<(), String> {
    let target = super::identity::bundle_dir().unwrap_or_else(|| std::env::current_exe().unwrap_or_default());
    let pid = std::process::id();
    let open = if target.extension().is_some_and(|e| e == "app") { "/usr/bin/open \"$0\"" } else { "\"$0\" &" };
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("while kill -0 {pid} 2>/dev/null; do sleep 0.1; done; {open}"))
        .arg(&target)
        .spawn()
        .map_err(|e| e.to_string())?;
    super::core::core().shell().quit();
    Ok(())
}
