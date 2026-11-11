//! AppKit reads and in-process events. Every call is made on the UI thread.

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSRunningApplication, NSScreen,
    NSView, NSWindow, NSWindowAnimationBehavior, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSString};

use crate::place::{self, Frame};

#[derive(Clone, Debug)]
pub struct Snap {
    pub alpha: f64,
    pub key: bool,
    pub visible: bool,
    pub on_space: bool,
    pub app_active: bool,
    pub front: String,
    pub pid: i32,
}

#[derive(Clone, Debug)]
pub struct Fact {
    pub found: bool,
    pub number: i64,
    pub alpha: f64,
    pub key: bool,
    pub visible: bool,
    pub on_space: bool,
    pub shadow: bool,
    pub level: i64,
    pub behavior: u64,
    pub animation: i64,
    pub style: u64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub radius: f64,
    pub effect_class: String,
    pub material: i64,
    pub blending: i64,
    pub effect_state: i64,
    pub can_key: bool,
    pub ignores_mouse: bool,
    pub accepts_first_mouse: bool,
    pub fullscreen: bool,
    pub class_name: String,
}

#[derive(Clone, Debug)]
pub struct Screen {
    pub frame: Frame,
    pub visible: Frame,
    pub scale: f64,
}

#[derive(Clone, Debug)]
pub struct Front {
    pub bundle: String,
    pub pid: i32,
    pub app_active: bool,
    pub policy: i64,
    pub ax: bool,
}

fn marker() -> MainThreadMarker {
    MainThreadMarker::new().expect("AppKit calls run on the UI thread")
}

fn primary_span(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .iter()
        .next()
        .map(|s| s.frame().origin.y + s.frame().size.height)
        .unwrap_or(0.0)
}

fn topdown(span: f64, rect: NSRect) -> Frame {
    Frame {
        x: rect.origin.x,
        y: span - (rect.origin.y + rect.size.height),
        w: rect.size.width,
        h: rect.size.height,
    }
}

fn contains(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y > rect.origin.y
        && point.y <= rect.origin.y + rect.size.height
}

fn class_name(win: &NSWindow) -> String {
    win.class().name().to_string_lossy().into_owned()
}

fn is_panel(win: &NSWindow) -> bool {
    class_name(win).contains("Panel")
}

/// Rune's launcher keeps the winit `NSWindow` and an adopted `NSPanel` under the same title.
/// The panel owns the content view. Prefer that window; a shell with no view is the wrong one.
fn by_title(title: &str) -> Option<Retained<NSWindow>> {
    let named = named(title);
    let with_view: Vec<_> = named
        .iter()
        .filter(|win| win.contentView().is_some())
        .cloned()
        .collect();
    let pool = if with_view.is_empty() {
        named
    } else {
        with_view
    };
    if let Some(panel) = pool.iter().find(|win| is_panel(win)).cloned() {
        return Some(panel);
    }
    pool.into_iter().next()
}

fn named(title: &str) -> Vec<Retained<NSWindow>> {
    let mtm = marker();
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .filter(|win| win.title().to_string() == title)
        .collect()
}

pub struct WindowSeen {
    pub title: String,
    pub class_name: String,
    pub level: i64,
    pub style: u64,
    pub behavior: u64,
    pub visible: bool,
    pub alpha: f64,
    pub number: i64,
    pub sharing: i64,
    pub has_content: bool,
    pub key: bool,
}

pub fn inventory() -> Vec<WindowSeen> {
    let mtm = marker();
    NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .map(|win| WindowSeen {
            title: win.title().to_string(),
            class_name: class_name(&win),
            level: win.level() as i64,
            style: win.styleMask().0 as u64,
            behavior: win.collectionBehavior().0 as u64,
            visible: win.isVisible(),
            alpha: win.alphaValue(),
            number: win.windowNumber() as i64,
            sharing: {
                let sharing: usize = unsafe { msg_send![&*win, sharingType] };
                sharing as i64
            },
            has_content: win.contentView().is_some(),
            key: win.isKeyWindow(),
        })
        .collect()
}

pub fn pointer_screen() -> Option<Screen> {
    let mtm = marker();
    let span = primary_span(mtm);
    let point = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    let screen = screens
        .iter()
        .find(|s| contains(s.frame(), point))
        .or_else(|| screens.iter().next())?;
    Some(Screen {
        frame: topdown(span, screen.frame()),
        visible: topdown(span, screen.visibleFrame()),
        scale: screen.backingScaleFactor(),
    })
}

pub fn front() -> Front {
    let mtm = marker();
    let app = NSApplication::sharedApplication(mtm);
    let front = NSWorkspace::sharedWorkspace().frontmostApplication();
    Front {
        bundle: front
            .as_ref()
            .and_then(|a| a.bundleIdentifier())
            .map(|s| s.to_string())
            .unwrap_or_default(),
        pid: front.as_ref().map(|a| a.processIdentifier()).unwrap_or(-1),
        app_active: app.isActive(),
        policy: app.activationPolicy().0 as i64,
        ax: ax_trusted(),
    }
}

pub fn ax_trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> u8;
    }
    // SAFETY: AXIsProcessTrusted does not prompt and has no pointer arguments.
    unsafe { AXIsProcessTrusted() != 0 }
}

pub fn fact(title: &str) -> Fact {
    let Some(win) = by_title(title) else {
        return missing();
    };
    let mtm = marker();
    let span = primary_span(mtm);
    let frame = topdown(span, win.frame());
    let content = win.contentView();
    let (effect_class, material, blending, effect_state) =
        content.as_deref().map(effect_of).unwrap_or_default();
    let radius = content.as_deref().map(corner_radius).unwrap_or(-1.0);
    let accepts = content.as_ref().is_some_and(|v| v.acceptsFirstMouse(None));
    Fact {
        found: true,
        number: win.windowNumber() as i64,
        alpha: win.alphaValue(),
        key: win.isKeyWindow(),
        visible: win.isVisible(),
        on_space: win.isOnActiveSpace(),
        shadow: win.hasShadow(),
        level: win.level() as i64,
        behavior: win.collectionBehavior().0 as u64,
        animation: win.animationBehavior().0 as i64,
        style: win.styleMask().0 as u64,
        x: frame.x,
        y: frame.y,
        w: frame.w,
        h: frame.h,
        radius,
        effect_class,
        material,
        blending,
        effect_state,
        can_key: win.canBecomeKeyWindow(),
        ignores_mouse: win.ignoresMouseEvents(),
        accepts_first_mouse: accepts,
        fullscreen: win.styleMask().contains(NSWindowStyleMask::FullScreen),
        class_name: class_name(&win),
    }
}

pub fn missing() -> Fact {
    Fact {
        found: false,
        number: -1,
        alpha: -1.0,
        key: false,
        visible: false,
        on_space: false,
        shadow: false,
        level: -1,
        behavior: 0,
        animation: -1,
        style: 0,
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        radius: -1.0,
        effect_class: String::new(),
        material: -1,
        blending: -1,
        effect_state: -1,
        can_key: false,
        ignores_mouse: false,
        accepts_first_mouse: false,
        fullscreen: false,
        class_name: String::new(),
    }
}

pub fn snap(title: &str) -> Snap {
    let fact = fact(title);
    let front = front();
    Snap {
        alpha: fact.alpha,
        key: fact.key,
        visible: fact.visible,
        on_space: fact.on_space,
        app_active: front.app_active,
        front: front.bundle,
        pid: front.pid,
    }
}

pub fn set_animation_none(title: &str) -> bool {
    let Some(win) = by_title(title) else {
        return false;
    };
    win.setAnimationBehavior(NSWindowAnimationBehavior::None);
    true
}

pub fn set_shadow(title: &str, on: bool) -> bool {
    let Some(win) = by_title(title) else {
        return false;
    };
    win.setHasShadow(on);
    true
}

pub fn resign_key(title: &str) -> bool {
    let Some(win) = by_title(title) else {
        return false;
    };
    win.resignKeyWindow();
    true
}

pub fn toggle_fullscreen(title: &str) -> bool {
    let Some(win) = by_title(title) else {
        return false;
    };
    win.toggleFullScreen(None);
    true
}

pub fn activate_bundle(bundle: &str) -> bool {
    let id = NSString::from_str(bundle);
    NSRunningApplication::runningApplicationsWithBundleIdentifier(&id)
        .iter()
        .next()
        .is_some_and(|app| {
            app.activateWithOptions(
                objc2_app_kit::NSApplicationActivationOptions::ActivateAllWindows,
            )
        })
}

pub fn send_key(title: &str, ch: &str, code: u16) -> bool {
    let Some(win) = by_title(title) else {
        return false;
    };
    let mtm = marker();
    let text = NSString::from_str(ch);
    let Some(event) = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::empty(),
        0.0,
        win.windowNumber(),
        None,
        &text,
        &text,
        false,
        code,
    ) else {
        return false;
    };
    NSApplication::sharedApplication(mtm).sendEvent(&event);
    true
}

pub fn send_click(title: &str) -> Option<bool> {
    let win = by_title(title)?;
    let mtm = marker();
    let frame = win.frame();
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDown,
        NSPoint::new(frame.size.width / 2.0, frame.size.height / 2.0),
        NSEventModifierFlags::empty(),
        0.0,
        win.windowNumber(),
        None,
        1,
        1,
        1.0,
    )?;
    let accepts = win
        .contentView()
        .is_some_and(|v| v.acceptsFirstMouse(Some(&event)));
    NSApplication::sharedApplication(mtm).sendEvent(&event);
    Some(accepts)
}

fn effect_of(view: &NSView) -> (String, i64, i64, i64) {
    let mut found = (String::new(), -1, -1, -1);
    walk(view, &mut found, 0);
    found
}

fn walk(view: &NSView, found: &mut (String, i64, i64, i64), depth: usize) {
    if depth > 8 {
        return;
    }
    let name = view.class().name().to_string_lossy();
    if name.contains("NSVisualEffectView") {
        // SAFETY: NSVisualEffectView's material, blendingMode and state are integer getters.
        let (material, blending, state): (isize, isize, isize) = unsafe {
            (
                msg_send![view, material],
                msg_send![view, blendingMode],
                msg_send![view, state],
            )
        };
        *found = (
            name.into_owned(),
            material as i64,
            blending as i64,
            state as i64,
        );
    } else if found.0.is_empty() && name.contains("NSGlassEffectView") {
        *found = (name.into_owned(), -1, -1, -1);
    }
    let subs = view.subviews();
    for sub in subs.iter() {
        walk(&sub, found, depth + 1);
    }
}

fn corner_radius(view: &NSView) -> f64 {
    // SAFETY: `layer` is null or a CALayer; cornerRadius is a CGFloat getter.
    let layer: *mut AnyObject = unsafe { msg_send![view, layer] };
    if layer.is_null() {
        return -1.0;
    }
    unsafe { msg_send![layer, cornerRadius] }
}

pub fn behavior_names(bits: u64) -> Vec<&'static str> {
    let table = [
        (1_u64, "CanJoinAllSpaces"),
        (1 << 1, "MoveToActiveSpace"),
        (1 << 2, "Managed"),
        (1 << 3, "Transient"),
        (1 << 4, "Stationary"),
        (1 << 5, "ParticipatesInCycle"),
        (1 << 6, "IgnoresCycle"),
        (1 << 7, "FullScreenPrimary"),
        (1 << 8, "FullScreenAuxiliary"),
        (1 << 9, "FullScreenNone"),
        (1 << 11, "FullScreenAllowsTiling"),
        (1 << 12, "FullScreenDisallowsTiling"),
        (1 << 16, "Primary"),
        (1 << 17, "Auxiliary"),
        (1 << 18, "CanJoinAllApplications"),
    ];
    table
        .iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| *name)
        .collect()
}

pub fn appkit_local_ok(measured: Frame, expected: Frame, tol: f64) -> bool {
    place::near(measured.x, expected.x, tol)
        && place::near(measured.y, expected.y, tol)
        && place::near(measured.w, expected.w, tol)
        && place::near(measured.h, expected.h, tol)
}
