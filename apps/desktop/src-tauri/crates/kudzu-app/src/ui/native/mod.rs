//! AppKit pieces Rune doesn't provide yet: the main window's `sidebar` vibrancy, the app's
//! appearance, the display under the pointer, and the menu-bar item (`tray`). All of it is
//! dynamic Objective-C messaging, so it needs no bindings beyond objc2. Call everything on the
//! main thread (Rune's handlers, `on_window` and `on_ready` run there).

pub mod tray;


use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSString};

use crate::settings::Theme as Appearance;

/// The `NSView` winit made for a Rune window.
pub fn content_view(handle: &rune::WindowHandle) -> Option<*mut AnyObject> {
    handle
        .with_raw_handle(|raw| match raw.as_raw() {
            raw_window_handle::RawWindowHandle::AppKit(h) => Some(h.ns_view.as_ptr() as *mut AnyObject),
            _ => None,
        })
        .ok()
        .flatten()
}

/// An `NSVisualEffectView` that never takes clicks, so winit's view receives every event.
fn effect_class() -> Option<&'static AnyClass> {
    const NAME: &std::ffi::CStr = c"KudzuVibrancyView";
    if let Some(class) = AnyClass::get(NAME) {
        return Some(class);
    }
    let mut builder = ClassBuilder::new(NAME, AnyClass::get(c"NSVisualEffectView")?)?;
    unsafe extern "C-unwind" fn hit_test(_: &AnyObject, _: Sel, _: NSPoint) -> *mut AnyObject {
        std::ptr::null_mut()
    }
    // SAFETY: the signature matches -[NSView hitTest:].
    unsafe {
        builder.add_method(sel!(hitTest:), hit_test as unsafe extern "C-unwind" fn(_, _, _) -> _);
    }
    Some(builder.register())
}

/// `windowEffects: { effects: ["sidebar"], state: "followsWindowActiveState" }`: the whole
/// window behind Rune's drawing is the sidebar material, as Tauri set it up. The window must be
/// transparent; the main pane draws its own opaque card over it.
pub fn install_sidebar_vibrancy(handle: &rune::WindowHandle) {
    let Some(view) = content_view(handle) else { return };
    let Some(class) = effect_class() else { return };
    // SAFETY: AppKit on the main thread; the effect view is retained by its superview.
    unsafe {
        let bounds: NSRect = msg_send![view, bounds];
        let alloc: *mut AnyObject = msg_send![class, alloc];
        let effect: *mut AnyObject = msg_send![alloc, initWithFrame: bounds];
        let Some(effect) = Retained::from_raw(effect) else { return };
        // NSVisualEffectMaterialSidebar, behind window, follows the window's active state.
        let _: () = msg_send![&*effect, setMaterial: 7isize];
        let _: () = msg_send![&*effect, setBlendingMode: 0isize];
        let _: () = msg_send![&*effect, setState: 0isize];
        // Width and height sizable.
        let _: () = msg_send![&*effect, setAutoresizingMask: 18usize];
        let _: () = msg_send![view, addSubview: &*effect, positioned: -1isize, relativeTo: std::ptr::null::<AnyObject>()];
        lift_metal_layer(view);
    }
}

/// Keeps the CAMetalLayer Rune draws into above native views added under it (AppKit re-sorts
/// sublayers to subview order; siblings composite by zPosition first).
unsafe fn lift_metal_layer(view: *mut AnyObject) {
    unsafe {
        let root: *mut AnyObject = msg_send![view, layer];
        let Some(root) = root.as_ref() else { return };
        let layers: *mut AnyObject = msg_send![root, sublayers];
        let Some(layers) = layers.as_ref() else { return };
        let n: usize = msg_send![layers, count];
        let metal = AnyClass::get(c"CAMetalLayer");
        for i in 0..n {
            let layer: *mut AnyObject = msg_send![layers, objectAtIndex: i];
            let is_metal: Bool = match metal {
                Some(m) => msg_send![layer, isKindOfClass: m],
                None => Bool::NO,
            };
            if is_metal.as_bool() {
                let _: () = msg_send![layer, setZPosition: 1.0f64];
            }
        }
    }
}

/// Points the app's appearance at the user's theme, so native materials, menus and the
/// windows' system-dark signal match (Tauri's `set_theme` on every window).
pub fn set_appearance(theme: Appearance) {
    // SAFETY: NSApplication and NSAppearance on the main thread.
    unsafe {
        let Some(app_class) = AnyClass::get(c"NSApplication") else { return };
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        let appearance: *mut AnyObject = match theme {
            Appearance::System => std::ptr::null_mut(),
            Appearance::Dark | Appearance::Light => {
                let Some(class) = AnyClass::get(c"NSAppearance") else { return };
                let name = NSString::from_str(if theme == Appearance::Dark { "NSAppearanceNameDarkAqua" } else { "NSAppearanceNameAqua" });
                msg_send![class, appearanceNamed: &*name]
            }
        };
        let _: () = msg_send![app, setAppearance: appearance];
    }
}

/// The display under the pointer's visible frame, as (x, y, w, h) in global top-left points.
pub fn pointer_display() -> Option<(f64, f64, f64, f64)> {
    // SAFETY: NSEvent and NSScreen class methods; called on the main thread or tolerated off it
    // (both are documented thread-safe reads).
    unsafe {
        let event = AnyClass::get(c"NSEvent")?;
        let mouse: NSPoint = msg_send![event, mouseLocation];
        let screen_class = AnyClass::get(c"NSScreen")?;
        let screens: *mut AnyObject = msg_send![screen_class, screens];
        let screens = screens.as_ref()?;
        let n: usize = msg_send![screens, count];
        if n == 0 {
            return None;
        }
        let first: *mut AnyObject = msg_send![screens, objectAtIndex: 0usize];
        let primary: NSRect = msg_send![first, frame];
        let mut pick = first;
        for i in 0..n {
            let s: *mut AnyObject = msg_send![screens, objectAtIndex: i];
            let f: NSRect = msg_send![s, frame];
            if mouse.x >= f.origin.x && mouse.x < f.origin.x + f.size.width && mouse.y >= f.origin.y && mouse.y < f.origin.y + f.size.height {
                pick = s;
                break;
            }
        }
        let f: NSRect = msg_send![pick, frame];
        // Cocoa's origin is bottom-left of the primary screen; flip to top-left.
        let top = primary.size.height - (f.origin.y + f.size.height);
        Some((f.origin.x, top, f.size.width, f.size.height))
    }
}

// MARK: App menu

/// The app menu's two actions: Close Windows (⌘Q) and Quit Kudzu.
type MenuHandlers = (Box<dyn Fn()>, Box<dyn Fn()>);

thread_local! {
    static MENU_HANDLERS: std::cell::RefCell<Option<MenuHandlers>> = const { std::cell::RefCell::new(None) };
    static MENU_TARGET: std::cell::RefCell<Option<Retained<AnyObject>>> = const { std::cell::RefCell::new(None) };
}

fn menu_target_class() -> Option<&'static AnyClass> {
    const NAME: &std::ffi::CStr = c"KudzuAppMenuTarget";
    if let Some(class) = AnyClass::get(NAME) {
        return Some(class);
    }
    let mut builder = ClassBuilder::new(NAME, AnyClass::get(c"NSObject")?)?;
    unsafe extern "C-unwind" fn close_windows(_: &AnyObject, _: Sel, _: *mut AnyObject) {
        MENU_HANDLERS.with(|h| {
            if let Some((close, _)) = h.borrow().as_ref() {
                close();
            }
        });
    }
    unsafe extern "C-unwind" fn quit(_: &AnyObject, _: Sel, _: *mut AnyObject) {
        MENU_HANDLERS.with(|h| {
            if let Some((_, quit)) = h.borrow().as_ref() {
                quit();
            }
        });
    }
    // SAFETY: both match an action method's signature.
    unsafe {
        builder.add_method(sel!(kudzuCloseWindows:), close_windows as unsafe extern "C-unwind" fn(_, _, _));
        builder.add_method(sel!(kudzuQuit:), quit as unsafe extern "C-unwind" fn(_, _, _));
    }
    Some(builder.register())
}

/// The app menu as the Tauri build had it: **Close Windows ⌘Q** (Kudzu keeps running in the
/// menu bar) and **Quit Kudzu** with no shortcut, both through the app's own paths so quitting
/// still saves clipboard history and installs a downloaded update. Call after the menu exists
/// (from `on_ready`).
pub fn adopt_app_menu(close_windows: impl Fn() + 'static, quit: impl Fn() + 'static) {
    MENU_HANDLERS.with(|h| *h.borrow_mut() = Some((Box::new(close_windows), Box::new(quit))));
    // SAFETY: AppKit on the main thread; the target is retained in MENU_TARGET.
    unsafe {
        let Some(app_class) = AnyClass::get(c"NSApplication") else { return };
        let Some(target_class) = menu_target_class() else { return };
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        let main: *mut AnyObject = msg_send![app, mainMenu];
        let Some(main) = main.as_ref() else { return };
        let first: *mut AnyObject = msg_send![main, itemAtIndex: 0isize];
        let Some(first) = first.as_ref() else { return };
        let menu: *mut AnyObject = msg_send![first, submenu];
        let Some(menu) = menu.as_ref() else { return };
        let target: *mut AnyObject = msg_send![target_class, new];
        let Some(target) = Retained::from_raw(target) else { return };
        let n: isize = msg_send![menu, numberOfItems];
        for i in 0..n {
            let item: *mut AnyObject = msg_send![menu, itemAtIndex: i];
            let action: Option<Sel> = msg_send![item, action];
            if action != Some(sel!(terminate:)) {
                continue;
            }
            let _: () = msg_send![item, setAction: sel!(kudzuQuit:)];
            let _: () = msg_send![item, setTarget: &*target];
            let _: () = msg_send![item, setKeyEquivalent: &*NSString::from_str("")];
            let class = AnyClass::get(c"NSMenuItem").expect("NSMenuItem");
            let alloc: *mut AnyObject = msg_send![class, alloc];
            let close: *mut AnyObject = msg_send![alloc, initWithTitle: &*NSString::from_str("Close Windows"), action: Some(sel!(kudzuCloseWindows:)), keyEquivalent: &*NSString::from_str("q")];
            let _: () = msg_send![close, setTarget: &*target];
            let _: () = msg_send![menu, insertItem: close, atIndex: i];
            let _: () = msg_send![close, release];
            break;
        }
        MENU_TARGET.with(|t| *t.borrow_mut() = Some(target));
    }
}
