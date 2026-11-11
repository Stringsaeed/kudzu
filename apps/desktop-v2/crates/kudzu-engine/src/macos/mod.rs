//! macOS implementation.

mod apps;
mod ax;
pub(crate) mod clipboard;
mod engine;
mod ffi;
mod input_source;
mod keys;
mod pasteboard;
mod tap;

pub(crate) use engine::Imp;

/// OS services behind [`crate::system`].
pub(crate) mod system {
    use std::time::Duration;

    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSLocale;

    use super::{apps, ax, pasteboard};
    use crate::{AppInfo, PermissionStatus, Rect};

    pub fn permissions() -> PermissionStatus {
        ax::permissions()
    }

    pub fn request_accessibility() {
        ax::request_accessibility();
    }

    pub fn request_input_monitoring() {
        ax::request_input_monitoring();
    }

    pub fn open_settings(pane: &str) {
        let url = match pane {
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "inputMonitoring" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            "loginItems" => "x-apple.systempreferences:com.apple.LoginItems-Settings.extension",
            _ => "x-apple.systempreferences:com.apple.preference.security",
        };
        if !apps::open_url(url) {
            log::warn!("could not open System Settings pane {pane}");
        }
    }

    pub fn frontmost_app() -> Option<AppInfo> {
        apps::frontmost_app()
    }

    pub fn activate_app(app: &AppInfo) -> bool {
        apps::activate(app)
    }

    pub fn installed_apps() -> Vec<AppInfo> {
        apps::installed_apps()
    }

    pub fn app_icon_png(path: &str, size: u32) -> Option<Vec<u8>> {
        apps::icon_png(path, size)
    }

    pub fn caret_rect() -> Option<Rect> {
        ax::caret_rect()
    }

    pub fn copy_selection() -> Option<String> {
        ax::selected_text().or_else(|| pasteboard::copy_selection(Duration::from_millis(300)))
    }

    pub fn selection_for_insert() -> Option<String> {
        match ax::reported_selection() {
            Some(text) => Some(text).filter(|s| !s.is_empty()),
            None => pasteboard::copy_selection(Duration::from_millis(300)),
        }
    }

    pub fn read_clipboard() -> Option<String> {
        pasteboard::read_string()
    }

    pub fn write_clipboard(text: &str) -> bool {
        pasteboard::write_string(text)
    }

    pub fn secure_input() -> (bool, Option<String>) {
        let (on, pid) = ax::secure_input();
        (on, pid.and_then(apps::name_of))
    }

    pub fn locale() -> String {
        autoreleasepool(|_| {
            let identifier = NSLocale::currentLocale().localeIdentifier().to_string();
            // Drop keyword suffixes such as `@rg=gbzzzz`.
            let base = identifier.split('@').next().unwrap_or_default().trim().to_string();
            if base.is_empty() { "en_US".to_string() } else { base }
        })
    }
}
