//! Running applications and activation. Installed-app enumeration and icons live in
//! `kudzu-system`.

use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use objc2::rc::{autoreleasepool, Retained};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_foundation::NSString;

use crate::AppInfo;

fn info(app: &NSRunningApplication) -> AppInfo {
    let path = app
        .bundleURL()
        .and_then(|url| url.path())
        .map(|p| p.to_string());
    let bundle_id = app
        .bundleIdentifier()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let name = app
        .localizedName()
        .map(|s| s.to_string())
        .or_else(|| path.as_deref().map(display_name_from_path))
        .unwrap_or_else(|| bundle_id.clone());
    AppInfo {
        bundle_id,
        name,
        pid: Some(app.processIdentifier()),
        path,
    }
}

fn display_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The frontmost application.
pub(crate) fn frontmost_app() -> Option<AppInfo> {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| info(&app))
    })
}

/// Pid of the frontmost application (cheap; safe from the tap thread).
pub(crate) fn frontmost_pid() -> Option<i32> {
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier())
    })
}

/// The running process `pid`.
pub(crate) fn app_of(pid: i32) -> Option<AppInfo> {
    autoreleasepool(|_| {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid).map(|app| info(&app))
    })
}

/// Bundle id of the running process `pid`.
pub(crate) fn bundle_id_of(pid: i32) -> Option<String> {
    autoreleasepool(|_| {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .and_then(|app| app.bundleIdentifier())
            .map(|s| s.to_string())
    })
}

/// Localized name of the running process `pid`.
pub(crate) fn name_of(pid: i32) -> Option<String> {
    autoreleasepool(|_| {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .and_then(|app| app.localizedName())
            .map(|s| s.to_string())
    })
}

fn running(app: &AppInfo) -> Option<Retained<NSRunningApplication>> {
    if let Some(found) = app
        .pid
        .and_then(NSRunningApplication::runningApplicationWithProcessIdentifier)
    {
        return Some(found);
    }
    if app.bundle_id.is_empty() {
        return None;
    }
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        &app.bundle_id,
    ))
    .firstObject()
}

/// Brings `app` to the front (by pid, else bundle id). Returns whether the request was accepted.
pub(crate) fn activate(app: &AppInfo) -> bool {
    autoreleasepool(|_| {
        running(app)
            .is_some_and(|found| found.activateWithOptions(NSApplicationActivationOptions::empty()))
    })
}

/// Pid `app` resolves to when running.
pub(crate) fn resolve_pid(app: &AppInfo) -> Option<i32> {
    autoreleasepool(|_| running(app).map(|found| found.processIdentifier()))
}

/// Activates `app` and waits (≤ `timeout`) until it is frontmost.
pub(crate) fn activate_and_wait(app: &AppInfo, timeout: Duration) -> bool {
    let Some(pid) = resolve_pid(app) else {
        return false;
    };
    if frontmost_pid() == Some(pid) {
        return true;
    }
    activate(app);
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
        if frontmost_pid() == Some(pid) {
            return true;
        }
    }
    false
}
