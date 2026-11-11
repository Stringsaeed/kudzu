//! AppKit and CoreGraphics reads. Every call runs on the UI thread. Window list is metadata
//! only: no screen capture, no Accessibility.

use std::path::Path;
use std::ptr;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSPasteboard,
    NSPasteboardTypeFileURL, NSPasteboardTypeString, NSPasteboardTypeURL, NSRunningApplication,
    NSScreen, NSWindow, NSWorkspace,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSDictionary, NSNumber, NSPoint, NSRect, NSString, NSURL,
};

use crate::place::{self, Rect};
use crate::settings::{self, WindowRecord};

pub const SETTINGS_ID: &str = "com.apple.systempreferences";
pub const SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const COMPANION_TITLE: &str = "kudzu-spike-permissions";
pub const GUIDE_TITLE: &str = "kudzu-spike-guide";
pub const COLLECTION_BITS: u64 = 1 | (1 << 8);

#[derive(Clone, Debug)]
pub struct Fact {
    pub found: bool,
    pub number: i64,
    pub visible: bool,
    pub on_space: bool,
    pub level: i64,
    pub behavior: u64,
    pub style: u64,
    pub alpha: f64,
    pub ignores_mouse: bool,
    pub shadow: bool,
    pub class_name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub radius: f64,
}

impl Fact {
    pub fn appkit(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Front {
    pub bundle: String,
    pub pid: i32,
    pub app_active: bool,
}

fn marker() -> MainThreadMarker {
    MainThreadMarker::new().expect("AppKit calls run on the UI thread")
}

fn class_name(win: &NSWindow) -> String {
    win.class().name().to_string_lossy().into_owned()
}

fn named(title: &str) -> Vec<Retained<NSWindow>> {
    NSApplication::sharedApplication(marker())
        .windows()
        .iter()
        .filter(|win| win.title().to_string() == title)
        .collect()
}

fn by_title(title: &str) -> Option<Retained<NSWindow>> {
    pick_named(&named(title))
}

fn pick_named(named: &[Retained<NSWindow>]) -> Option<Retained<NSWindow>> {
    let with_view: Vec<_> = named
        .iter()
        .filter(|win| win.contentView().is_some())
        .cloned()
        .collect();
    let pool = if with_view.is_empty() {
        named.to_vec()
    } else {
        with_view
    };
    pool.iter()
        .find(|win| class_name(win).contains("Panel"))
        .cloned()
        .or_else(|| pool.into_iter().next())
}

pub fn inventory() -> Vec<serde_json::Value> {
    NSApplication::sharedApplication(marker())
        .windows()
        .iter()
        .map(|win| {
            let frame = ns_to_rect(win.frame());
            serde_json::json!({
                "title": win.title().to_string(),
                "class": class_name(&win),
                "level": win.level() as i64,
                "visible": win.isVisible(),
                "on_space": win.isOnActiveSpace(),
                "style": win.styleMask().0 as u64,
                "behavior": win.collectionBehavior().0 as u64,
                "alpha": win.alphaValue(),
                "frame": [frame.x, frame.y, frame.w, frame.h],
                "has_view": win.contentView().is_some(),
            })
        })
        .collect()
}

fn by_size(w: f64, h: f64) -> Option<Retained<NSWindow>> {
    NSApplication::sharedApplication(marker())
        .windows()
        .iter()
        .find(|win| {
            let frame = win.frame();
            (frame.size.width - w).abs() < 4.0 && (frame.size.height - h).abs() < 4.0
        })
}

pub fn companion_window() -> Option<Retained<NSWindow>> {
    by_title(COMPANION_TITLE)
        .or_else(|| by_title("permissions"))
        .or_else(|| by_size(place::COMPANION_W, place::COMPANION_H))
}

pub fn desktop_top() -> f64 {
    NSScreen::screens(marker())
        .iter()
        .next()
        .map(|s| s.frame().size.height)
        .unwrap_or(0.0)
}

pub fn visible_frame_containing(point: (f64, f64)) -> Rect {
    let mtm = marker();
    let screens = NSScreen::screens(mtm);
    let hit = screens
        .iter()
        .find(|s| {
            let f = s.frame();
            point.0 >= f.origin.x
                && point.0 < f.origin.x + f.size.width
                && point.1 >= f.origin.y
                && point.1 < f.origin.y + f.size.height
        })
        .or_else(|| screens.iter().next());
    match hit {
        Some(s) => ns_to_rect(s.visibleFrame()),
        None => Rect {
            x: 0.0,
            y: 0.0,
            w: 1440.0,
            h: 900.0,
        },
    }
}

fn ns_to_rect(rect: NSRect) -> Rect {
    Rect {
        x: rect.origin.x,
        y: rect.origin.y,
        w: rect.size.width,
        h: rect.size.height,
    }
}

pub fn fact(title: &str) -> Fact {
    let win = if title == COMPANION_TITLE {
        companion_window()
    } else {
        by_title(title)
    };
    let Some(win) = win else {
        return missing();
    };
    fact_of(&win)
}

fn fact_of(win: &NSWindow) -> Fact {
    let frame = ns_to_rect(win.frame());
    Fact {
        found: true,
        number: win.windowNumber() as i64,
        visible: win.isVisible(),
        on_space: win.isOnActiveSpace(),
        level: win.level() as i64,
        behavior: win.collectionBehavior().0 as u64,
        style: win.styleMask().0 as u64,
        alpha: win.alphaValue(),
        ignores_mouse: win.ignoresMouseEvents(),
        shadow: win.hasShadow(),
        class_name: class_name(win),
        x: frame.x,
        y: frame.y,
        w: frame.w,
        h: frame.h,
        radius: content_radius(win),
    }
}

pub fn missing() -> Fact {
    Fact {
        found: false,
        number: -1,
        visible: false,
        on_space: false,
        level: -1,
        behavior: 0,
        style: 0,
        alpha: -1.0,
        ignores_mouse: false,
        shadow: false,
        class_name: String::new(),
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        radius: -1.0,
    }
}

fn content_radius(win: &NSWindow) -> f64 {
    let Some(view) = win.contentView() else {
        return -1.0;
    };
    let layer: *mut AnyObject = unsafe { msg_send![&*view, layer] };
    if layer.is_null() {
        return -1.0;
    }
    unsafe { msg_send![layer, cornerRadius] }
}

pub fn front() -> Front {
    let app = NSApplication::sharedApplication(marker());
    let front = NSWorkspace::sharedWorkspace().frontmostApplication();
    Front {
        bundle: front
            .as_ref()
            .and_then(|a| a.bundleIdentifier())
            .map(|s| s.to_string())
            .unwrap_or_default(),
        pid: front.as_ref().map(|a| a.processIdentifier()).unwrap_or(-1),
        app_active: app.isActive(),
    }
}

pub fn settings_running() -> bool {
    settings_app().is_some()
}

fn settings_app() -> Option<Retained<NSRunningApplication>> {
    let id = NSString::from_str(SETTINGS_ID);
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&id).firstObject()
}

pub fn settings_pid() -> Option<i32> {
    settings_app().map(|a| a.processIdentifier())
}

/// CGWindowListCopyWindowInfo, on-screen, no desktop elements. No capture, no AX.
pub fn window_records() -> Vec<WindowRecord> {
    const ON_SCREEN: u32 = 1;
    const EXCLUDE_DESKTOP: u32 = 16;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(option: u32, relative: u32) -> *mut AnyObject;
    }
    let ptr = unsafe { CGWindowListCopyWindowInfo(ON_SCREEN | EXCLUDE_DESKTOP, 0) };
    if ptr.is_null() {
        return Vec::new();
    }
    let array: Retained<NSArray> = unsafe { Retained::from_raw(ptr.cast()).expect("window list") };
    array
        .iter()
        .filter_map(|item| record_from(item.downcast::<NSDictionary>().ok()?))
        .collect()
}

fn record_from(info: Retained<NSDictionary>) -> Option<WindowRecord> {
    let pid = number(&info, "kCGWindowOwnerPID")? as i32;
    let layer = number(&info, "kCGWindowLayer").unwrap_or(0.0) as i32;
    let bounds = bounds_of(&info)?;
    Some(WindowRecord { pid, layer, bounds })
}

fn number(info: &NSDictionary, key: &str) -> Option<f64> {
    let obj = info.objectForKey(&NSString::from_str(key))?;
    let num: Option<Retained<NSNumber>> = obj.downcast().ok();
    num.map(|n| n.as_f64())
}

fn bounds_of(info: &NSDictionary) -> Option<Rect> {
    let obj = info.objectForKey(&NSString::from_str("kCGWindowBounds"))?;
    let dict: Retained<NSDictionary> = obj.downcast().ok()?;
    Some(Rect {
        x: number(&dict, "X").unwrap_or(0.0),
        y: number(&dict, "Y").unwrap_or(0.0),
        w: number(&dict, "Width").unwrap_or(0.0),
        h: number(&dict, "Height").unwrap_or(0.0),
    })
}

pub fn settings_frame() -> Option<Rect> {
    let pid = settings_pid()?;
    let rec = settings::pick(&window_records(), pid)?;
    Some(place::cg_to_appkit(rec.bounds, desktop_top()))
}

pub fn open_settings() -> bool {
    let url = NSURL::URLWithString(&NSString::from_str(SETTINGS_URL)).expect("settings url");
    NSWorkspace::sharedWorkspace().openURL(&url)
}

pub fn quit_settings() {
    if let Some(app) = settings_app() {
        app.terminate();
    }
}

fn window_named(title: &str) -> Option<Retained<NSWindow>> {
    if title == COMPANION_TITLE {
        companion_window()
    } else {
        by_title(title)
    }
}

pub fn set_origin(title: &str, x: f64, y: f64) -> bool {
    let Some(win) = window_named(title) else {
        return false;
    };
    win.setFrameOrigin(NSPoint::new(x, y));
    true
}

pub fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

pub fn can_post_events() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightPostEventAccess() -> bool;
    }
    unsafe { CGPreflightPostEventAccess() }
}

pub fn warp(x: f64, y: f64) {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWarpMouseCursorPosition(point: NSPoint) -> i32;
    }
    unsafe {
        CGWarpMouseCursorPosition(NSPoint::new(x, y));
    }
}

pub fn post_mouse(title: &str, events: &[(usize, f64, f64)]) -> bool {
    let Some(win) = window_named(title) else {
        return false;
    };
    let mtm = marker();
    let content = win.contentLayoutRect();
    for &(kind, x, y) in events {
        let ty = match kind {
            1 => NSEventType::LeftMouseDown,
            2 => NSEventType::LeftMouseUp,
            6 => NSEventType::LeftMouseDragged,
            _ => continue,
        };
        let at = NSPoint::new(
            content.origin.x + x,
            content.origin.y + content.size.height - y,
        );
        let Some(event) = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            ty,
            at,
            NSEventModifierFlags::empty(),
            0.0,
            win.windowNumber(),
            None,
            1,
            1,
            1.0,
        ) else {
            return false;
        };
        NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
    }
    true
}

pub fn release_real_button() {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreate(source: *const std::ffi::c_void) -> *mut std::ffi::c_void;
        fn CGEventGetLocation(event: *mut std::ffi::c_void) -> NSPoint;
        fn CGEventCreateMouseEvent(
            source: *const std::ffi::c_void,
            kind: u32,
            at: NSPoint,
            button: u32,
        ) -> *mut std::ffi::c_void;
        fn CGEventPost(tap: u32, event: *mut std::ffi::c_void);
        fn CFRelease(object: *mut std::ffi::c_void);
    }
    unsafe {
        let here = CGEventCreate(ptr::null());
        let at = CGEventGetLocation(here);
        CFRelease(here);
        let up = CGEventCreateMouseEvent(ptr::null(), 2, at, 0);
        CGEventPost(0, up);
        CFRelease(up);
    }
}

pub fn drag_pasteboard() -> (Option<String>, Option<String>) {
    let name = NSString::from_str("Apple CFPasteboard drag");
    let board = NSPasteboard::pasteboardWithName(&name);
    let url = unsafe { board.stringForType(NSPasteboardTypeFileURL) }.map(|s| s.to_string());
    let path = unsafe { board.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string());
    (url, path)
}

/// Write the same types Rune's drag-out uses onto a private pasteboard, then read them back.
pub fn prove_pasteboard(bundle: &Path) -> serde_json::Value {
    let path = bundle.display().to_string();
    let file = NSURL::fileURLWithPath(&NSString::from_str(&path));
    let url = file
        .absoluteString()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("file://{path}"));
    let name = NSString::from_str("kudzu.permissions-spike.drag");
    let board = NSPasteboard::pasteboardWithName(&name);
    let _ = board.clearContents();
    let wrote_file =
        unsafe { board.setString_forType(&NSString::from_str(&url), NSPasteboardTypeFileURL) };
    let wrote_url =
        unsafe { board.setString_forType(&NSString::from_str(&url), NSPasteboardTypeURL) };
    let wrote_path =
        unsafe { board.setString_forType(&NSString::from_str(&path), NSPasteboardTypeString) };
    let got_url = unsafe { board.stringForType(NSPasteboardTypeFileURL) }.map(|s| s.to_string());
    let got_path = unsafe { board.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string());
    serde_json::json!({
        "wrote_file_url": url,
        "wrote_ok": wrote_file && wrote_url && wrote_path,
        "read_file_url": got_url,
        "read_path": got_path,
        "matches": got_url.as_deref() == Some(url.as_str()) && got_path.as_deref() == Some(path.as_str()),
    })
}

pub fn reveal_app(bundle: &Path) -> bool {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&bundle.display().to_string()));
    let urls = NSArray::from_retained_slice(&[url]);
    NSWorkspace::sharedWorkspace().activateFileViewerSelectingURLs(&urls);
    true
}

pub fn finder_running() -> bool {
    let id = NSString::from_str("com.apple.finder");
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&id)
        .iter()
        .next()
        .is_some()
}

pub fn quit_finder() {
    let id = NSString::from_str("com.apple.finder");
    if let Some(app) = NSRunningApplication::runningApplicationsWithBundleIdentifier(&id)
        .iter()
        .next()
    {
        app.terminate();
    }
}

pub fn close_finder_front_window() {
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            r#"tell application "Finder" to if (count of windows) > 0 then close front window"#,
        ])
        .status();
}

pub fn show_native_guide(canvas: Rect) -> Fact {
    unsafe {
        let mtm = marker();
        let rect = NSRect::new(
            NSPoint::new(canvas.x, canvas.y),
            objc2_foundation::NSSize::new(canvas.w, canvas.h),
        );
        let style: usize = 1 << 11 | 1 << 7;
        let panel: *mut AnyObject = msg_send![class!(NSPanel), alloc];
        let panel: *mut AnyObject = msg_send![
            panel,
            initWithContentRect: rect,
            styleMask: style,
            backing: 2usize,
            defer: false
        ];
        let _: () = msg_send![panel, setReleasedWhenClosed: false];
        let _: () = msg_send![panel, setOpaque: false];
        let _: () = msg_send![panel, setHasShadow: false];
        let _: () = msg_send![panel, setIgnoresMouseEvents: true];
        let _: () = msg_send![panel, setLevel: 3isize];
        let _: () = msg_send![panel, setCollectionBehavior: COLLECTION_BITS];
        let _: () = msg_send![panel, setTitle: &*NSString::from_str("kudzu-spike-guide-native")];
        let clear: *mut AnyObject = msg_send![class!(NSColor), clearColor];
        let _: () = msg_send![panel, setBackgroundColor: clear];
        let _: () = msg_send![panel, orderFrontRegardless];
        let _ = mtm;
        fact("kudzu-spike-guide-native")
    }
}

pub fn hide_native_guide() {
    if let Some(win) = by_title("kudzu-spike-guide-native") {
        win.orderOut(None);
        win.close();
    }
}

pub fn close_titled(title: &str) {
    for win in named(title) {
        win.orderOut(None);
        win.close();
    }
}

pub fn behavior_names(bits: u64) -> Vec<&'static str> {
    [
        (1_u64, "CanJoinAllSpaces"),
        (1 << 8, "FullScreenAuxiliary"),
        (1 << 1, "MoveToActiveSpace"),
        (1 << 4, "Stationary"),
        (1 << 6, "IgnoresCycle"),
        (1 << 18, "CanJoinAllApplications"),
    ]
    .into_iter()
    .filter(|(bit, _)| bits & bit != 0)
    .map(|(_, name)| name)
    .collect()
}
