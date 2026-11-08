//! The menu-bar item and its menu, ported from the Tauri shell's `tray.rs`: the same glyphs
//! (normal, paused, Secure Input, needs permission) as template images, the update dot, the same
//! items, titles and shortcuts. Rune's `Tray` has no macOS backend yet, so this is an
//! `NSStatusItem` with an `NSMenu` whose items send `kudzuTrayItem:` to a small target object.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use kudzu_engine::{EngineStatus, TapMode};
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{msg_send, sel};
use objc2_foundation::{NSData, NSSize, NSString};

use crate::backend::updater::UpdateState;

/// Which menu-bar glyph to show (the same set as the native app's).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Glyph {
    Normal = 0,
    Paused = 1,
    SecureInput = 2,
    NeedsPermission = 3,
}

static GLYPH: AtomicU8 = AtomicU8::new(Glyph::Normal as u8);
/// Whether a downloaded update is waiting (draws a dot on the glyph).
static UPDATE_DOT: AtomicBool = AtomicBool::new(false);

/// What a menu item does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pause,
    Search,
    Clipboard,
    FromSelection,
    Open,
    Settings,
    Updates,
    Quit,
}

const ACTIONS: [Action; 8] =
    [Action::Pause, Action::Search, Action::Clipboard, Action::FromSelection, Action::Open, Action::Settings, Action::Updates, Action::Quit];

struct Tray {
    item: Retained<AnyObject>,
    status: Retained<AnyObject>,
    pause: Retained<AnyObject>,
    search: Retained<AnyObject>,
    clipboard: Retained<AnyObject>,
    updates: Retained<AnyObject>,
    _target: Retained<AnyObject>,
    _menu: Retained<AnyObject>,
}

type Handler = Box<dyn Fn(Action)>;

thread_local! {
    // AppKit objects live on the main thread only.
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
    static HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
}

fn png(glyph: Glyph) -> &'static [u8] {
    match glyph {
        Glyph::Normal => include_bytes!("../../../../../icons/tray.png"),
        Glyph::Paused => include_bytes!("../../../../../icons/tray-paused.png"),
        Glyph::SecureInput => include_bytes!("../../../../../icons/tray-secure.png"),
        Glyph::NeedsPermission => include_bytes!("../../../../../icons/tray-permissions.png"),
    }
}

/// The glyph as RGBA, with the update dot when one is waiting. A small filled dot in the
/// top-right corner; template images use alpha only, so it takes the menu bar's text colour,
/// like the system's own badges. A clear ring separates it from the glyph underneath.
fn glyph_png(glyph: Glyph) -> Vec<u8> {
    let bytes = png(glyph);
    if !UPDATE_DOT.load(Ordering::Relaxed) {
        return bytes.to_vec();
    }
    let Ok(image) = image::load_from_memory(bytes) else { return bytes.to_vec() };
    let mut rgba = image.to_rgba8();
    let (w, h) = (rgba.width() as i64, rgba.height() as i64);
    let r = (w.min(h) as f64 * 0.17).max(2.0);
    let (cx, cy) = (w as f64 - r - 0.5, r + 0.5);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt();
            let p = rgba.get_pixel_mut(x as u32, y as u32);
            if d <= r {
                p.0 = [0, 0, 0, 255];
            } else if d <= r + r * 0.45 {
                p.0[3] = 0;
            }
        }
    }
    let mut out = Vec::new();
    let _ = rgba.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png);
    out
}

fn glyph_for(status: &EngineStatus) -> Glyph {
    if status.paused {
        Glyph::Paused
    } else if status.secure_input {
        Glyph::SecureInput
    } else if status.tap == TapMode::Off || !status.permissions.accessibility {
        Glyph::NeedsPermission
    } else {
        Glyph::Normal
    }
}

pub fn hotkey_label(accel: &str) -> String {
    accel
        .split('+')
        .map(|k| match k.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => "⌃".to_string(),
            "alt" | "option" => "⌥".to_string(),
            "shift" => "⇧".to_string(),
            "command" | "cmd" | "super" | "meta" | "cmdorctrl" | "commandorcontrol" => "⌘".to_string(),
            other => other.trim_start_matches("key").to_uppercase(),
        })
        .collect::<Vec<_>>()
        .join("")
}

pub fn summary(status: &EngineStatus) -> String {
    if status.paused {
        "Paused".into()
    } else if status.secure_input {
        match &status.secure_input_owner {
            Some(app) => format!("Blocked — Secure Input in {app}"),
            None => "Blocked — Secure Input is on".into(),
        }
    } else if status.tap == TapMode::Off {
        "Needs permission to work".into()
    } else {
        "Expanding".into()
    }
}

fn target_class() -> Option<&'static AnyClass> {
    const NAME: &std::ffi::CStr = c"KudzuTrayTarget";
    if let Some(class) = AnyClass::get(NAME) {
        return Some(class);
    }
    let mut builder = ClassBuilder::new(NAME, AnyClass::get(c"NSObject")?)?;
    unsafe extern "C-unwind" fn picked(_: &AnyObject, _: Sel, sender: *mut AnyObject) {
        // SAFETY: AppKit sends this on the main thread with the NSMenuItem.
        let tag: isize = unsafe { msg_send![sender, tag] };
        let Some(action) = ACTIONS.get(tag as usize).copied() else { return };
        HANDLER.with(|h| {
            if let Some(h) = h.borrow().as_ref() {
                h(action);
            }
        });
    }
    // SAFETY: the signature matches an action method (`- (void)kudzuTrayItem:(id)sender`).
    unsafe {
        builder.add_method(sel!(kudzuTrayItem:), picked as unsafe extern "C-unwind" fn(_, _, _));
    }
    Some(builder.register())
}

unsafe fn ns(s: &str) -> Retained<NSString> {
    NSString::from_str(s)
}

unsafe fn template_image(bytes: &[u8]) -> *mut AnyObject {
    unsafe {
        let Some(class) = AnyClass::get(c"NSImage") else { return std::ptr::null_mut() };
        let data = NSData::with_bytes(bytes);
        let alloc: *mut AnyObject = msg_send![class, alloc];
        let image: *mut AnyObject = msg_send![alloc, initWithData: &*data];
        if image.is_null() {
            return image;
        }
        let _: () = msg_send![image, setSize: NSSize::new(18.0, 18.0)];
        let _: () = msg_send![image, setTemplate: true];
        image
    }
}

/// One menu item. `key` is an equivalent like "p" with `mods` (NSEventModifierFlags bits).
unsafe fn item(menu: &AnyObject, target: &AnyObject, title: &str, action: Option<Action>, key: &str, mods: usize, enabled: bool) -> Retained<AnyObject> {
    unsafe {
        let class = AnyClass::get(c"NSMenuItem").expect("NSMenuItem");
        let alloc: *mut AnyObject = msg_send![class, alloc];
        let sel = if action.is_some() { Some(sel!(kudzuTrayItem:)) } else { None };
        let it: *mut AnyObject = msg_send![alloc, initWithTitle: &*ns(title), action: sel, keyEquivalent: &*ns(key)];
        let it = Retained::from_raw(it).expect("menu item");
        if let Some(a) = action {
            let _: () = msg_send![&*it, setTarget: target];
            let _: () = msg_send![&*it, setTag: ACTIONS.iter().position(|x| *x == a).unwrap_or(0) as isize];
        }
        if !key.is_empty() {
            let _: () = msg_send![&*it, setKeyEquivalentModifierMask: mods];
        }
        let _: () = msg_send![&*it, setEnabled: enabled];
        let _: () = msg_send![menu, addItem: &*it];
        it
    }
}

unsafe fn separator(menu: &AnyObject) {
    unsafe {
        let class = AnyClass::get(c"NSMenuItem").expect("NSMenuItem");
        let sep: *mut AnyObject = msg_send![class, separatorItem];
        let _: () = msg_send![menu, addItem: sep];
    }
}

const CMD: usize = 1 << 20;
const SHIFT: usize = 1 << 17;

pub struct TrayInit {
    pub dev: bool,
    pub paused: bool,
    pub palette_hotkey: String,
    pub clipboard_hotkey: String,
    pub clipboard_enabled: bool,
}

/// Creates the status item. `on_action` runs on the main thread for each pick.
pub fn create(init: TrayInit, on_action: impl Fn(Action) + 'static) {
    HANDLER.with(|h| *h.borrow_mut() = Some(Box::new(on_action)));
    // SAFETY: AppKit on the main thread; every object is retained in TRAY.
    unsafe {
        let Some(bar_class) = AnyClass::get(c"NSStatusBar") else { return };
        let Some(menu_class) = AnyClass::get(c"NSMenu") else { return };
        let Some(target_class) = target_class() else { return };
        let bar: *mut AnyObject = msg_send![bar_class, systemStatusBar];
        // NSVariableStatusItemLength
        let status_item: *mut AnyObject = msg_send![bar, statusItemWithLength: -1.0f64];
        let Some(status_item) = Retained::retain(status_item) else { return };
        let target: *mut AnyObject = msg_send![target_class, new];
        let target = Retained::from_raw(target).expect("tray target");
        let menu: *mut AnyObject = msg_send![menu_class, new];
        let menu = Retained::from_raw(menu).expect("tray menu");
        let _: () = msg_send![&*menu, setAutoenablesItems: false];

        let status = item(&menu, &target, "Starting…", None, "", 0, false);
        separator(&menu);
        let search = item(&menu, &target, &format!("Quick Search      {}", hotkey_label(&init.palette_hotkey)), Some(Action::Search), "", 0, true);
        let clipboard = item(
            &menu,
            &target,
            &format!("Clipboard History      {}", hotkey_label(&init.clipboard_hotkey)),
            Some(Action::Clipboard),
            "",
            0,
            init.clipboard_enabled,
        );
        item(&menu, &target, "New Snippet from Selection", Some(Action::FromSelection), "", 0, true);
        let pause = item(&menu, &target, if init.paused { "Resume Expanding" } else { "Pause Expanding" }, Some(Action::Pause), "p", CMD | SHIFT, true);
        separator(&menu);
        item(&menu, &target, if init.dev { "Open Kudzu Dev…" } else { "Open Kudzu…" }, Some(Action::Open), "o", CMD, true);
        item(&menu, &target, "Settings…", Some(Action::Settings), ",", CMD, true);
        let updates = item(&menu, &target, "Check for Updates…", Some(Action::Updates), "", 0, true);
        separator(&menu);
        item(&menu, &target, if init.dev { "Quit Kudzu Dev" } else { "Quit Kudzu" }, Some(Action::Quit), "", 0, true);

        let _: () = msg_send![&*status_item, setMenu: &*menu];
        let button: *mut AnyObject = msg_send![&*status_item, button];
        if !button.is_null() {
            let glyph = if init.paused { Glyph::Paused } else { Glyph::Normal };
            GLYPH.store(glyph as u8, Ordering::Relaxed);
            let image = template_image(&glyph_png(glyph));
            let _: () = msg_send![button, setImage: image];
            let _: () = msg_send![button, setToolTip: &*ns(if init.dev { "Kudzu Dev" } else { "Kudzu" })];
            if init.dev {
                let _: () = msg_send![button, setTitle: &*ns("Dev")];
                // NSImageLeading
                let _: () = msg_send![button, setImagePosition: 7isize];
            }
        }
        TRAY.with(|t| {
            *t.borrow_mut() = Some(Tray { item: status_item, status, pause, search, clipboard, updates, _target: target, _menu: menu });
        });
    }
}

fn with(f: impl FnOnce(&Tray)) {
    TRAY.with(|t| {
        if let Some(t) = t.borrow().as_ref() {
            f(t);
        }
    });
}

unsafe fn set_title(item: &AnyObject, title: &str) {
    unsafe {
        let _: () = msg_send![item, setTitle: &*ns(title)];
    }
}

fn refresh_icon() {
    let glyph = match GLYPH.load(Ordering::Relaxed) {
        1 => Glyph::Paused,
        2 => Glyph::SecureInput,
        3 => Glyph::NeedsPermission,
        _ => Glyph::Normal,
    };
    with(|t| unsafe {
        let button: *mut AnyObject = msg_send![&*t.item, button];
        if !button.is_null() {
            let image = template_image(&glyph_png(glyph));
            let _: () = msg_send![button, setImage: image];
        }
    });
}

/// Reflects engine status in the icon and menu.
pub fn update(status: &EngineStatus) {
    GLYPH.store(glyph_for(status) as u8, Ordering::Relaxed);
    refresh_icon();
    with(|t| unsafe {
        set_title(&t.status, &summary(status));
        set_title(&t.pause, if status.paused { "Resume Expanding" } else { "Pause Expanding" });
        let button: *mut AnyObject = msg_send![&*t.item, button];
        if !button.is_null() {
            let _: () = msg_send![button, setToolTip: &*ns(&format!("Kudzu — {}", summary(status)))];
        }
    });
}

/// Updates the hotkey hint next to "Quick Search".
pub fn set_hotkey_hint(accel: &str) {
    with(|t| unsafe { set_title(&t.search, &format!("Quick Search      {}", hotkey_label(accel))) });
}

/// Updates the hotkey hint next to "Clipboard History".
pub fn set_clipboard_hint(accel: &str) {
    with(|t| unsafe { set_title(&t.clipboard, &format!("Clipboard History      {}", hotkey_label(accel))) });
}

/// Greys out "Clipboard History" while recording is off.
pub fn set_clipboard_enabled(enabled: bool) {
    with(|t| unsafe {
        let _: () = msg_send![&*t.clipboard, setEnabled: enabled];
    });
}

/// Mirrors the update state in the menu ("Restart to Update to 0.3.0").
pub fn set_update(state: &UpdateState) {
    let label = match state {
        UpdateState::Ready { version, .. } => format!("Restart to Update to {version}"),
        UpdateState::Downloading { version, .. } => format!("Downloading {version}…"),
        UpdateState::Restarting { .. } => "Restarting…".into(),
        UpdateState::Checking => "Checking for Updates…".into(),
        _ => "Check for Updates…".into(),
    };
    with(|t| unsafe { set_title(&t.updates, &label) });
    UPDATE_DOT.store(matches!(state, UpdateState::Ready { .. }), Ordering::Relaxed);
    refresh_icon();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkey_hints_read_like_the_menu() {
        assert_eq!(hotkey_label("Control+Alt+Space"), "⌃⌥SPACE");
        assert_eq!(hotkey_label("Command+Shift+KeyK"), "⌘⇧K");
    }
}
