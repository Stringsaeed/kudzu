//! Running and installed applications, activation and icons.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use objc2::rc::{autoreleasepool, Retained};
use objc2::AnyThread;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSBitmapImageFileType, NSBitmapImageRep,
    NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
    NSImageInterpolation, NSRunningApplication, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString, NSURL};

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

/// Opens a URL with its default handler.
pub(crate) fn open_url(url: &str) -> bool {
    autoreleasepool(|_| {
        NSURL::URLWithString(&NSString::from_str(url))
            .is_some_and(|url| NSWorkspace::sharedWorkspace().openURL(&url))
    })
}

fn application_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = [
        "/Applications",
        "/Applications/Utilities",
        "/System/Applications",
        "/System/Applications/Utilities",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join("Applications"));
    }
    roots
}

fn is_app(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("app"))
}

/// `.app` bundles directly in `root` and in its plain sub-folders (one level deep).
fn app_bundles(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if is_app(&path) {
            found.push(path);
        } else if path.is_dir() {
            if let Ok(nested) = fs::read_dir(&path) {
                found.extend(nested.flatten().map(|e| e.path()).filter(|p| is_app(p)));
            }
        }
    }
    found
}

fn read_bundle(path: &Path) -> Option<AppInfo> {
    let plist = plist::Value::from_file(path.join("Contents/Info.plist")).ok()?;
    let dict = plist.as_dictionary()?;
    let string = |key: &str| {
        dict.get(key)
            .and_then(|v| v.as_string())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    let bundle_id = string("CFBundleIdentifier")?.to_string();
    let name = string("CFBundleDisplayName")
        .or_else(|| string("CFBundleName"))
        .map(String::from)
        .unwrap_or_else(|| display_name_from_path(&path.to_string_lossy()));
    Some(AppInfo {
        bundle_id,
        name,
        pid: None,
        path: Some(path.to_string_lossy().into_owned()),
    })
}

/// Installed apps, deduped by bundle id (first root wins), sorted by name.
pub(crate) fn installed_apps() -> Vec<AppInfo> {
    let mut by_id: HashMap<String, AppInfo> = HashMap::new();
    let mut seen_paths = std::collections::HashSet::new();
    for root in application_roots() {
        for bundle in app_bundles(&root) {
            let key = fs::canonicalize(&bundle).unwrap_or_else(|_| bundle.clone());
            if !seen_paths.insert(key) {
                continue;
            }
            if let Some(app) = read_bundle(&bundle) {
                by_id.entry(app.bundle_id.clone()).or_insert(app);
            }
        }
    }
    let mut apps: Vec<AppInfo> = by_id.into_values().collect();
    apps.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.bundle_id.cmp(&b.bundle_id))
    });
    apps
}

/// PNG of the icon for `path`, drawn at exactly `size`×`size` pixels.
pub(crate) fn icon_png(path: &str, size: u32) -> Option<Vec<u8>> {
    if size == 0 || size > 1024 || !Path::new(path).exists() {
        return None;
    }
    autoreleasepool(|_| {
        let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path));
        draw_png(&image, size, size)
    })
}

/// Draws `image` into a `width`×`height` RGBA bitmap and encodes it as PNG.
/// Call inside an autorelease pool; safe off the main thread (it uses its own bitmap context).
pub(crate) fn draw_png(image: &NSImage, width: u32, height: u32) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return None;
    }
    // SAFETY: null planes ask AppKit to allocate the buffer; the arguments describe 8-bit RGBA.
    let rep = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            width as isize,
            height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            32,
        )
    }?;
    let points = NSSize::new(f64::from(width), f64::from(height));
    rep.setSize(points);
    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
    NSGraphicsContext::saveGraphicsState_class();
    NSGraphicsContext::setCurrentContext(Some(&context));
    context.setImageInterpolation(NSImageInterpolation::High);
    let target = NSRect::new(NSPoint::new(0.0, 0.0), points);
    image.drawInRect_fromRect_operation_fraction(
        target,
        NSRect::ZERO,
        NSCompositingOperation::Copy,
        1.0,
    );
    context.flushGraphics();
    NSGraphicsContext::restoreGraphicsState_class();
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_system_apps_sorted_and_unique() {
        let apps = installed_apps();
        if apps.is_empty() {
            return; // Not a regular macOS install (CI sandbox).
        }
        let mut ids: Vec<&str> = apps.iter().map(|a| a.bundle_id.as_str()).collect();
        let total = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), total);
        let names: Vec<String> = apps.iter().map(|a| a.name.to_lowercase()).collect();
        assert!(names.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn icon_is_png() {
        let Some(app) = installed_apps().into_iter().find(|a| a.path.is_some()) else {
            return;
        };
        let png = icon_png(app.path.as_deref().unwrap(), 32).expect("icon");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(icon_png("/definitely/not/here.app", 32).is_none());
    }
}
