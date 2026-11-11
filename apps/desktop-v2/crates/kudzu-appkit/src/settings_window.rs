//! RUNE-GAP(#16): metadata-only adapter; delete when permission guidance no longer needs it.
//! Gap 16 has no upstream issue. Only pid, layer and bounds are decoded; never log dictionaries.
use crate::geometry::Rect;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSRunningApplication, NSScreen, NSWorkspace};
use objc2_foundation::{MainThreadMarker, NSArray, NSDictionary, NSNumber, NSString};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pane {
    Accessibility,
    InputMonitoring,
}

impl Pane {
    pub fn url(self) -> &'static str {
        match self {
            Self::Accessibility => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            Self::InputMonitoring => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRecord {
    pub pid: i32,
    pub layer: i32,
    pub bounds: Rect,
}

/// Input order is front to back. Bounds enter in CG top-down points and leave in AppKit points.
pub fn pick(records: &[WindowRecord], pid: i32, desktop_top: f64) -> Option<Rect> {
    records
        .iter()
        .find(|r| {
            r.pid == pid
                && r.layer == 0
                && r.bounds.valid()
                && r.bounds.width >= 400.
                && r.bounds.height >= 300.
        })
        .map(|r| Rect {
            y: desktop_top - r.bounds.max_y(),
            ..r.bounds
        })
}

/// Obj-C's MAX(lower, MIN(preferred, upper)), including a companion taller than the screen.
pub fn companion_frame(settings: Rect, visible: Rect, size: (f64, f64)) -> Rect {
    let mut x = settings.x - size.0 - 18.;
    if x < visible.x + 12. {
        x = settings.max_x() + 18.;
    }
    Rect {
        x: x.min(visible.max_x() - size.0 - 12.).max(visible.x + 12.),
        y: (settings.max_y() - size.1)
            .min(visible.max_y() - size.1 - 12.)
            .max(visible.y + 12.),
        width: size.0,
        height: size.1,
    }
}

pub fn find(mtm: MainThreadMarker) -> Option<Rect> {
    let app = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        "com.apple.systempreferences",
    ))
    .firstObject()?;
    let top = NSScreen::screens(mtm).firstObject()?.frame().size.height;
    pick(&records(mtm), app.processIdentifier(), top)
}

/// On-screen metadata, in window-server order. This neither captures pixels nor queries AX.
pub fn records(_: MainThreadMarker) -> Vec<WindowRecord> {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> *mut AnyObject;
    }
    // SAFETY: Copy returns a +1 CFArray, toll-free bridged to NSArray and released by Retained.
    let Some(array): Option<Retained<NSArray>> =
        (unsafe { Retained::from_raw(CGWindowListCopyWindowInfo(1 | 16, 0).cast()) })
    else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|value| {
            let dict: Retained<NSDictionary> = value.downcast().ok()?;
            let pid = number(&dict, "kCGWindowOwnerPID")? as i32;
            let layer = number(&dict, "kCGWindowLayer")? as i32;
            let bounds: Retained<NSDictionary> = dict
                .objectForKey(&NSString::from_str("kCGWindowBounds"))?
                .downcast()
                .ok()?;
            Some(WindowRecord {
                pid,
                layer,
                bounds: Rect {
                    x: number(&bounds, "X")?,
                    y: number(&bounds, "Y")?,
                    width: number(&bounds, "Width")?,
                    height: number(&bounds, "Height")?,
                },
            })
        })
        .collect()
}

fn number(dict: &NSDictionary, key: &str) -> Option<f64> {
    let number: Retained<NSNumber> = dict
        .objectForKey(&NSString::from_str(key))?
        .downcast()
        .ok()?;
    Some(number.as_f64())
}

pub fn reduce_motion(_: MainThreadMarker) -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

/// Read-only evidence for live QA. Never requests Screen Recording access.
pub fn screen_recording_access(_: MainThreadMarker) -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }
    // SAFETY: this preflight is a read-only query with no parameters.
    unsafe { CGPreflightScreenCaptureAccess() }
}
