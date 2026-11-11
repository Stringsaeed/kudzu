//! `AppCatalog`, `TargetApp` and `FileIcons` on macOS. Installed-app enumeration and icon
//! drawing moved here from the engine in S35; the frontmost app, activation, the caret and the
//! selection stay the engine's, because insertion shares them.
//!
//! Blocking: `installed` reads every bundle's Info.plist, icons rasterise, `selection` may
//! press ⌘C and wait up to ~300 ms behind the engine's pasteboard transactions. Call all of
//! them from workers, never the UI thread.
use kudzu_app::{
    error::PortError,
    ports::{AppCatalog, FileIcons, TargetApp},
};
use kudzu_core::system::{AppInfo, Rect};
use objc2::{rc::autoreleasepool, AnyThread};
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace,
    NSGraphicsContext, NSImage, NSImageInterpolation, NSWorkspace,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Icons are drawn at most this many pixels square.
const MAX_ICON_PX: u32 = 1024;

#[derive(Default)]
pub struct SystemApps;

impl AppCatalog for SystemApps {
    fn installed(&self) -> Result<Vec<AppInfo>, PortError> {
        Ok(installed_apps())
    }

    /// Looks the bundle up with Launch Services; `Ok(None)` when it isn't installed.
    fn icon(&self, bundle_id: &str, size: u32) -> Result<Option<Vec<u8>>, PortError> {
        Ok(app_path(bundle_id).and_then(|path| icon_png(&path, size)))
    }
}

impl TargetApp for SystemApps {
    fn frontmost(&self) -> Result<Option<AppInfo>, PortError> {
        Ok(kudzu_engine::system::frontmost_app())
    }

    /// `TargetGone` when the app isn't running or refused to activate.
    fn activate(&self, app: &AppInfo) -> Result<(), PortError> {
        if kudzu_engine::system::activate_app(app) {
            Ok(())
        } else {
            Err(PortError::TargetGone)
        }
    }

    /// Accessibility first; ⌘C only when the app reports no selection at all.
    fn selection(&self) -> Result<Option<String>, PortError> {
        Ok(kudzu_engine::system::selection_for_insert())
    }

    fn caret(&self) -> Result<Option<Rect>, PortError> {
        Ok(kudzu_engine::system::caret_rect())
    }
}

impl FileIcons for SystemApps {
    /// Finder's icon for `path`; `Ok(None)` when the path doesn't exist.
    fn file_icon(&self, path: &str, px: u32) -> Result<Option<Vec<u8>>, PortError> {
        Ok(icon_png(path, px))
    }
}

/// The installed bundle Launch Services picks for `bundle_id`.
fn app_path(bundle_id: &str) -> Option<String> {
    if bundle_id.is_empty() {
        return None;
    }
    autoreleasepool(|_| {
        NSWorkspace::sharedWorkspace()
            .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))
            .and_then(|url| url.path())
            .map(|path| path.to_string())
    })
}

fn display_name_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
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
        .unwrap_or_else(|| display_name_from_path(path));
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
    let mut seen_paths = HashSet::new();
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
    if size == 0 || size > MAX_ICON_PX || !Path::new(path).exists() {
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
    // SAFETY: an empty properties dictionary is valid for PNG encoding.
    let data = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }?;
    Some(data.to_vec())
}

#[cfg(test)]
mod tests;
