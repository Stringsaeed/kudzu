//! AppKit reads and writes for the probe. Every call runs on the UI thread, from inside an
//! `AppHandle::update` closure. Selectors go through `msg_send!` so each read names the
//! property it measures.

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{class, msg_send};
use objc2_app_kit::{NSApplication, NSScreen, NSView, NSWindow};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSString};
use serde_json::{json, Value};

pub const TITLE: &str = "Kudzu chrome spike";

fn mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect("AppKit calls run on the UI thread")
}

fn string(obj: &AnyObject, selector_result: Option<Retained<NSString>>) -> String {
    let _ = obj;
    selector_result.map(|s| s.to_string()).unwrap_or_default()
}

fn rect(r: NSRect) -> Value {
    json!([r.origin.x, r.origin.y, r.size.width, r.size.height])
}

/// The spike's titled window (the one `Chrome` configured), not the hidden shells.
pub fn window() -> Option<Retained<NSWindow>> {
    let app = NSApplication::sharedApplication(mtm());
    app.windows()
        .iter()
        .find(|w| w.title().to_string() == TITLE && w.contentView().is_some())
}

pub fn global_height(w: &NSWindow) -> f64 {
    NSScreen::screens(mtm())
        .iter()
        .next()
        .map(|s| s.frame().size.height)
        .unwrap_or_else(|| w.frame().size.height)
}

/// The window frame as (x, y, w, h) from the primary display's top left, in points.
pub fn frame_top_left(w: &NSWindow) -> [f64; 4] {
    let f = w.frame();
    [
        f.origin.x,
        global_height(w) - (f.origin.y + f.size.height),
        f.size.width,
        f.size.height,
    ]
}

/// Centre the window on the main screen, high enough that the backdrop fits below it.
pub fn place(w: &NSWindow) {
    let screen = NSScreen::mainScreen(mtm()).expect("a main screen");
    let vf = screen.visibleFrame();
    let f = w.frame();
    let x = vf.origin.x + (vf.size.width - f.size.width) / 2.0;
    let top = vf.origin.y + vf.size.height - 80.0;
    w.setFrameOrigin(NSPoint::new(x, top - f.size.height));
    // Above ordinary windows of other apps, so an inactive window is still uncovered.
    w.setLevel(4);
}

pub fn system_is_dark() -> bool {
    // SAFETY: NSUserDefaults reads the global domain; the key is absent in light mode.
    unsafe {
        let defaults: Retained<AnyObject> = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let v: Option<Retained<NSString>> =
            msg_send![&*defaults, stringForKey: &*NSString::from_str("AppleInterfaceStyle")];
        v.is_some_and(|s| s.to_string() == "Dark")
    }
}

/// `NSApp.appearance`: `Some(true)` DarkAqua, `Some(false)` Aqua, `None` follows the system.
pub fn set_app_appearance(dark: Option<bool>) {
    let app = NSApplication::sharedApplication(mtm());
    // SAFETY: documented AppKit setters on the main thread.
    unsafe {
        let appearance: Option<Retained<AnyObject>> = dark.map(|d| {
            let name = NSString::from_str(if d {
                "NSAppearanceNameDarkAqua"
            } else {
                "NSAppearanceNameAqua"
            });
            msg_send![class!(NSAppearance), appearanceNamed: &*name]
        });
        let _: () = msg_send![&*app, setAppearance: appearance.as_deref()];
    }
}

fn appearance_name(obj: &AnyObject) -> String {
    // SAFETY: every NSApplication, NSWindow and NSView has `effectiveAppearance`.
    unsafe {
        let ap: Option<Retained<AnyObject>> = msg_send![obj, effectiveAppearance];
        ap.map(|a| {
            let name: Option<Retained<NSString>> = msg_send![&*a, name];
            string(&a, name)
        })
        .unwrap_or_default()
    }
}

pub fn activate(w: &NSWindow) {
    let app = NSApplication::sharedApplication(mtm());
    // SAFETY: ordinary activation of our own app and window.
    unsafe {
        let _: () = msg_send![&*app, activateIgnoringOtherApps: true];
        let _: () = msg_send![w, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
    }
}

/// A small titled window that takes key status from the spike window, so the spike window
/// is inactive (grey traffic lights, window-state-following materials at rest) while the
/// app stays frontmost. `NSApp.deactivate` was ignored on this macOS.
pub fn open_helper(w: &NSWindow) -> Retained<NSWindow> {
    use objc2_app_kit::{NSBackingStoreType, NSWindowStyleMask};
    let f = w.frame();
    let rect = NSRect::new(
        NSPoint::new(f.origin.x, (f.origin.y - 140.0).max(20.0)),
        objc2_foundation::NSSize::new(220.0, 70.0),
    );
    // SAFETY: window construction on the main thread.
    unsafe {
        let helper = NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm().alloc(),
            rect,
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        );
        let _: () = msg_send![&*helper, setReleasedWhenClosed: false];
        let _: () = msg_send![&*helper, setLevel: 4isize];
        let _: () = msg_send![&*helper, setTitle: &*NSString::from_str("spike helper")];
        let _: () = msg_send![&*helper, orderFront: std::ptr::null::<AnyObject>()];
        helper
    }
}

pub fn make_key(w: &NSWindow) {
    // SAFETY: ordering our own window.
    unsafe {
        let _: () = msg_send![w, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
    }
}

/// Level 0 for the full-screen transition; the capture level (above other apps) otherwise.
pub fn set_level(w: &NSWindow, level: isize) {
    w.setLevel(level);
}

pub fn is_fullscreen(w: &NSWindow) -> bool {
    // SAFETY: style mask read.
    let mask: usize = unsafe { msg_send![w, styleMask] };
    mask & (1 << 14) != 0
}

pub fn toggle_fullscreen(w: &NSWindow) {
    // SAFETY: documented AppKit call; the transition runs asynchronously.
    unsafe {
        let _: () = msg_send![w, toggleFullScreen: std::ptr::null::<AnyObject>()];
    }
}

fn effect_views(view: &NSView, out: &mut Vec<Value>) {
    // SAFETY: reads of NSView/NSVisualEffectView properties.
    unsafe {
        let class = AnyClass::get(c"NSVisualEffectView");
        let is: bool = class.is_some_and(|c| msg_send![view, isKindOfClass: c]);
        if is {
            let material: isize = msg_send![view, material];
            let blending: isize = msg_send![view, blendingMode];
            let state: isize = msg_send![view, state];
            let frame: NSRect = msg_send![view, frame];
            let alpha: f64 = msg_send![view, alphaValue];
            let hidden: bool = msg_send![view, isHidden];
            out.push(json!({
                "alpha": alpha, "hidden": hidden,
                "material": material, "blending": blending, "state": state,
                "frame": rect(frame), "appearance": appearance_name(view),
            }));
        }
    }
    for sub in view.subviews().iter() {
        effect_views(&sub, out);
    }
}

/// Insert (or remove) the fallback: an `NSVisualEffectView`, material `sidebar`, behind
/// window, following the window's active state, under everything in the content view.
pub fn set_sidebar_view(w: &NSWindow, on: bool) {
    let Some(content) = w.contentView() else {
        return;
    };
    let id = NSString::from_str("kudzu-spike-sidebar");
    // SAFETY: view hierarchy edits on the main thread; the view is ours, found by tag.
    unsafe {
        let existing = content.subviews().iter().find(|v| {
            let ident: Option<Retained<NSString>> = msg_send![&**v, identifier];
            ident.is_some_and(|i| i.to_string() == id.to_string())
        });
        if let Some(v) = existing {
            if !on {
                let _: () = msg_send![&*v, removeFromSuperview];
            }
            return;
        }
        if !on {
            return;
        }
        let bounds = content.bounds();
        let alloc: Allocated<AnyObject> = msg_send![class!(NSVisualEffectView), alloc];
        let view: Retained<AnyObject> = msg_send![alloc, initWithFrame: bounds];
        let _: () = msg_send![&*view, setMaterial: 7isize]; // sidebar
        let _: () = msg_send![&*view, setBlendingMode: 0isize]; // behind window
        let _: () = msg_send![&*view, setState: 0isize]; // follows window active state
        let _: () = msg_send![&*view, setAutoresizingMask: 18usize]; // width + height
        let _: () = msg_send![&*view, setIdentifier: &*id];
        let _: () = msg_send![&*content, addSubview: &*view, positioned: -1isize, relativeTo: std::ptr::null::<AnyObject>()];
    }
}

fn button(w: &NSWindow, kind: isize) -> Value {
    // SAFETY: standard window buttons; frames converted to window coordinates.
    unsafe {
        let b: Option<Retained<NSView>> = msg_send![w, standardWindowButton: kind];
        let Some(b) = b else { return Value::Null };
        let frame: NSRect = msg_send![&*b, frame];
        let in_window: NSRect =
            msg_send![&*b, convertRect: frame, toView: std::ptr::null::<NSView>()];
        let hidden: bool = msg_send![&*b, isHidden];
        let h = w.frame().size.height;
        let sup: Option<Retained<NSView>> = msg_send![&*b, superview];
        let sup_frame: Option<NSRect> = sup.as_ref().map(|v| msg_send![&**v, frame]);
        let sup_bounds: Option<NSRect> = sup.as_ref().map(|v| msg_send![&**v, bounds]);
        json!({
            "raw_frame": rect(frame),
            "superview_frame": sup_frame.map(rect),
            "superview_bounds": sup_bounds.map(rect),
            "convert_rect_left_unreliable": in_window.origin.x,
            "convert_rect_top_unreliable": h - (in_window.origin.y + in_window.size.height),
            "size": [in_window.size.width, in_window.size.height],
            "hidden": hidden,
        })
    }
}

/// Everything the note quotes about the window as it stands.
pub fn facts(w: &NSWindow) -> Value {
    let app = NSApplication::sharedApplication(mtm());
    let mut effects = Vec::new();
    if let Some(c) = w.contentView() {
        effects_in_tree(&c, &mut effects);
    }
    // SAFETY: property reads.
    unsafe {
        let mask: usize = msg_send![w, styleMask];
        let transparent_titlebar: bool = msg_send![w, titlebarAppearsTransparent];
        let title_visibility: isize = msg_send![w, titleVisibility];
        let opaque: bool = msg_send![w, isOpaque];
        let shadow: bool = msg_send![w, hasShadow];
        let level: isize = msg_send![w, level];
        let key: bool = msg_send![w, isKeyWindow];
        let main_window: bool = msg_send![w, isMainWindow];
        let layout: NSRect = msg_send![w, contentLayoutRect];
        let content = w.contentView();
        let class_name = content
            .as_ref()
            .map(|c| c.class().name().to_string_lossy().into_owned())
            .unwrap_or_default();
        let can_move = content
            .as_ref()
            .map(|c| msg_send![&**c, mouseDownCanMoveWindow])
            .unwrap_or(false);
        let active: bool = msg_send![&*app, isActive];
        json!({
            "style_mask": mask,
            "full_size_content_view": mask & (1 << 15) != 0,
            "titled": mask & 1 != 0,
            "resizable": mask & (1 << 3) != 0,
            "fullscreen": mask & (1 << 14) != 0,
            "titlebar_appears_transparent": transparent_titlebar,
            "title_visibility_hidden": title_visibility == 1,
            "opaque": opaque,
            "has_shadow": shadow,
            "level": level,
            "key": key,
            "main": main_window,
            "app_active": active,
            "frame_top_left": json!(frame_top_left(w)),
            "content_layout_rect": rect(layout),
            "close": button(w, 0),
            "miniaturize": button(w, 1),
            "zoom": button(w, 2),
            "content_view_class": class_name,
            "content_view_mouse_down_can_move_window": can_move,
            "app_effective_appearance": appearance_name(&app),
            "window_effective_appearance": appearance_name(w),
            "effect_views": effects,
        })
    }
}

fn effects_in_tree(content: &NSView, out: &mut Vec<Value>) {
    // The content view's superview (the frame view) can hold effect views too.
    // SAFETY: superview read.
    unsafe {
        let sup: Option<Retained<NSView>> = msg_send![content, superview];
        if let Some(s) = sup {
            effect_views(&s, out);
            return;
        }
    }
    effect_views(content, out);
}
