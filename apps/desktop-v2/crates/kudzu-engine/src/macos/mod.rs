//! macOS implementation.

mod apps;
mod ax;
pub(crate) mod clipboard;
mod engine;
mod events;
mod ffi;
mod input_source;
mod keys;
mod observe;
mod pasteboard;
mod tap;
#[cfg(test)]
mod tests;

use std::sync::Arc;

pub(crate) use engine::Imp;

/// The seams between the engine and macOS: what it posts and listens to, the pasteboard, and
/// what it observes. Everything else the engine calls directly.
pub(crate) struct Platform {
    pub events: Arc<dyn events::Events>,
    pub pasteboard: Arc<pasteboard::Pasteboard>,
    pub observe: Arc<dyn observe::Observe>,
}

impl Platform {
    /// The real system, with the process-wide pasteboard.
    pub fn system() -> Self {
        Self {
            events: Arc::new(events::SystemEvents),
            pasteboard: pasteboard::Pasteboard::shared().clone(),
            observe: Arc::new(observe::SystemObserve),
        }
    }
}

/// OS services behind [`crate::system`].
pub(crate) mod system {
    use std::time::Duration;

    use objc2::rc::autoreleasepool;
    use objc2_foundation::NSLocale;

    use super::events::SystemEvents;
    use super::pasteboard::Pasteboard;
    use super::{apps, ax};
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
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "inputMonitoring" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
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
        ax::selected_text().or_else(|| {
            Pasteboard::shared().copy_selection(&SystemEvents, Duration::from_millis(300))
        })
    }

    pub fn selection_for_insert() -> Option<String> {
        match ax::reported_selection() {
            Some(text) => Some(text).filter(|s| !s.is_empty()),
            None => Pasteboard::shared().copy_selection(&SystemEvents, Duration::from_millis(300)),
        }
    }

    pub fn read_clipboard() -> Option<String> {
        Pasteboard::shared().read_string()
    }

    pub fn write_clipboard(text: &str) -> bool {
        Pasteboard::shared().write_string(text)
    }

    pub fn secure_input() -> (bool, Option<String>) {
        let (on, pid) = ax::secure_input();
        (on, pid.and_then(apps::name_of))
    }

    pub fn locale() -> String {
        autoreleasepool(|_| {
            let identifier = NSLocale::currentLocale().localeIdentifier().to_string();
            // Drop keyword suffixes such as `@rg=gbzzzz`.
            let base = identifier
                .split('@')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if base.is_empty() {
                "en_US".to_string()
            } else {
                base
            }
        })
    }
}
