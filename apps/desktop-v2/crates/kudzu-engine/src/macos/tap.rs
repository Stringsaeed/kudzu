//! A CGEventTap serviced by its own thread and CFRunLoop.

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;
use std::thread;

use core_foundation_sys::base::{CFRelease, CFRetain};
use core_foundation_sys::mach_port::{
    CFMachPortCreateRunLoopSource, CFMachPortInvalidate, CFMachPortRef,
};
use core_foundation_sys::runloop::{
    kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoopAddSource, CFRunLoopGetCurrent,
    CFRunLoopRemoveSource, CFRunLoopRunInMode, CFRunLoopSourceRef,
};
use kudzu_core::matching::KeyInput;

use super::ffi::*;
use crate::TapMode;

/// Decides whether to swallow an event. Runs on the tap thread; must be fast.
pub(crate) type Handler = Box<dyn Fn(CGEventType, CGEventRef) -> bool + Send + Sync>;

struct Context {
    handler: Handler,
    active: bool,
    port: AtomicPtr<c_void>,
    stopped: Arc<AtomicBool>,
}

unsafe extern "C" fn callback(
    _proxy: CGEventTapProxy,
    kind: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    // SAFETY: `user_info` is the Context owned by the tap thread, alive while its run loop runs.
    let context = unsafe { &*(user_info as *const Context) };
    if kind == kCGEventTapDisabledByTimeout || kind == kCGEventTapDisabledByUserInput {
        let port = context.port.load(Ordering::Acquire);
        if !port.is_null() && !context.stopped.load(Ordering::Acquire) {
            unsafe { CGEventTapEnable(port as CFMachPortRef, true) };
        }
    }
    // A panic must never unwind into CoreGraphics.
    let swallow =
        catch_unwind(AssertUnwindSafe(|| (context.handler)(kind, event))).unwrap_or(false);
    if swallow && context.active {
        ptr::null_mut()
    } else {
        event
    }
}

struct SendPtr(*mut c_void);
// SAFETY: CF objects are thread-safe to retain/release and to hand to another thread's run loop.
unsafe impl Send for SendPtr {}

/// A running tap. Dropping it stops the tap.
pub(crate) struct TapHandle {
    port: CFMachPortRef,
    stopped: Arc<AtomicBool>,
    mode: TapMode,
}

// SAFETY: the retained port is only used for thread-safe enable/invalidate calls.
unsafe impl Send for TapHandle {}

const EVENT_MASK: u64 = (1 << kCGEventKeyDown)
    | (1 << kCGEventKeyUp)
    | (1 << kCGEventLeftMouseDown)
    | (1 << kCGEventRightMouseDown)
    | (1 << kCGEventOtherMouseDown);

impl TapHandle {
    /// Creates a session tap (disabled until [`enable`](Self::enable)), serviced on a new thread.
    /// `None` when macOS refuses (missing permission).
    pub fn start(mode: TapMode, handler: Handler) -> Option<TapHandle> {
        let options = match mode {
            TapMode::Active => kCGEventTapOptionDefault,
            TapMode::ListenOnly => kCGEventTapOptionListenOnly,
            TapMode::Off => return None,
        };
        let stopped = Arc::new(AtomicBool::new(false));
        let context = Box::into_raw(Box::new(Context {
            handler,
            active: mode == TapMode::Active,
            port: AtomicPtr::new(ptr::null_mut()),
            stopped: stopped.clone(),
        }));
        // SAFETY: plain CF calls; `context` outlives the tap (freed by the tap thread after invalidation).
        unsafe {
            let port = CGEventTapCreate(
                kCGSessionEventTap,
                kCGTailAppendEventTap,
                options,
                EVENT_MASK,
                callback,
                context as *mut c_void,
            );
            if port.is_null() {
                drop(Box::from_raw(context));
                return None;
            }
            // A new tap starts enabled; an enabled active tap whose run loop isn't serviced yet
            // would stall every keystroke, so keep it off until the caller enables it.
            CGEventTapEnable(port, false);
            (*context)
                .port
                .store(port as *mut c_void, Ordering::Release);
            let source = CFMachPortCreateRunLoopSource(ptr::null(), port, 0);
            if source.is_null() {
                CFMachPortInvalidate(port);
                CFRelease(port as _);
                drop(Box::from_raw(context));
                return None;
            }
            CFRetain(port as _); // the handle's reference
            let (port_ptr, source_ptr, context_ptr) = (
                SendPtr(port as *mut c_void),
                SendPtr(source as *mut c_void),
                SendPtr(context as *mut c_void),
            );
            let flag = stopped.clone();
            let spawned = thread::Builder::new()
                .name("kudzu-event-tap".into())
                .spawn(move || {
                    let (port, source, context) = (port_ptr, source_ptr, context_ptr);
                    run(
                        port.0 as CFMachPortRef,
                        source.0 as CFRunLoopSourceRef,
                        context.0 as *mut Context,
                        &flag,
                    );
                });
            if spawned.is_err() {
                CFMachPortInvalidate(port);
                CFRelease(source as _);
                CFRelease(port as _);
                CFRelease(port as _);
                drop(Box::from_raw(context));
                return None;
            }
            Some(TapHandle {
                port,
                stopped,
                mode,
            })
        }
    }

    /// How this tap was created.
    pub fn mode(&self) -> TapMode {
        self.mode
    }

    /// Starts delivering events.
    pub fn enable(&self) {
        if !self.stopped.load(Ordering::Acquire) {
            unsafe { CGEventTapEnable(self.port, true) };
        }
    }
}

impl Drop for TapHandle {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        // SAFETY: we hold our own retain on the port.
        unsafe {
            CGEventTapEnable(self.port, false);
            CFMachPortInvalidate(self.port);
            CFRelease(self.port as _);
        }
    }
}

/// Tap thread body: service the source until stopped, then free everything it owns.
unsafe fn run(
    port: CFMachPortRef,
    source: CFRunLoopSourceRef,
    context: *mut Context,
    stopped: &AtomicBool,
) {
    unsafe {
        let run_loop = CFRunLoopGetCurrent();
        CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
        while !stopped.load(Ordering::Acquire) {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0);
        }
        CFRunLoopRemoveSource(run_loop, source, kCFRunLoopCommonModes);
        CFMachPortInvalidate(port);
        CFRelease(source as _);
        CFRelease(port as _);
        // Callbacks only run inside this thread's run loop, which has stopped.
        drop(Box::from_raw(context));
    }
}

/// A retained copy of a physical key event, held for later replay.
pub(crate) struct HeldEvent {
    event: CGEventRef,
    key_down: bool,
}

// SAFETY: CGEvents are CF objects; we own one reference and only touch it from one thread at a time.
unsafe impl Send for HeldEvent {}

impl HeldEvent {
    /// Copies `event` (the original continues through the tap).
    pub fn copy(event: CGEventRef, key_down: bool) -> Option<Self> {
        let copy = unsafe { CGEventCreateCopy(event) };
        (!copy.is_null()).then_some(HeldEvent {
            event: copy,
            key_down,
        })
    }

    /// Whether this is a key-down (as opposed to key-up).
    pub fn is_key_down(&self) -> bool {
        self.key_down
    }

    /// Posts the event with `marker` in its source user data: the real
    /// [`Events::post_held`](super::events::Events::post_held).
    pub fn post(self, marker: i64) {
        self.mark(marker);
        unsafe { CGEventPost(kCGSessionEventTap, self.event) };
    }

    /// Puts `marker` in the event's source user data.
    pub fn mark(&self, marker: i64) {
        unsafe { CGEventSetIntegerValueField(self.event, kCGEventSourceUserData, marker) };
    }

    /// The held copy, for a fake event stream to read and deliver again.
    #[cfg(test)]
    pub fn raw(&self) -> CGEventRef {
        self.event
    }
}

impl Drop for HeldEvent {
    fn drop(&mut self) {
        unsafe { CFRelease(self.event as _) };
    }
}

/// The marker in an event's source user data.
pub(crate) fn marker(event: CGEventRef) -> i64 {
    unsafe { CGEventGetIntegerValueField(event, kCGEventSourceUserData) }
}

/// The virtual key code of a key event.
pub(crate) fn key_code(event: CGEventRef) -> i64 {
    unsafe { CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode) }
}

/// Whether a key-down was generated by holding the key.
pub(crate) fn is_autorepeat(event: CGEventRef) -> bool {
    unsafe { CGEventGetIntegerValueField(event, kCGKeyboardEventAutorepeat) != 0 }
}

/// What a key-down means to the matcher; `None` when it produces no text and changes no context
/// (e.g. a dead key waiting for its base letter).
pub(crate) fn classify(event: CGEventRef) -> Option<KeyInput> {
    let flags = unsafe { CGEventGetFlags(event) };
    let key = key_code(event);
    let mut buffer = [0u16; 16];
    let mut length = 0usize;
    unsafe {
        CGEventKeyboardGetUnicodeString(event, buffer.len(), &mut length, buffer.as_mut_ptr())
    };
    classify_parts(flags, key, &buffer[..length.min(buffer.len())])
}

/// Pure part of [`classify`].
pub(crate) fn classify_parts(flags: CGEventFlags, key: i64, text: &[u16]) -> Option<KeyInput> {
    if flags & (kCGEventFlagMaskCommand | kCGEventFlagMaskControl) != 0 {
        return Some(KeyInput::Reset);
    }
    match key {
        51 => return Some(KeyInput::Backspace),
        // Escape, Home, Page Up, Forward Delete, End, Page Down, arrows.
        53 | 115 | 116 | 117 | 119 | 121 | 123..=126 => return Some(KeyInput::Reset),
        // Return / Enter and Tab are word separators rather than navigation.
        36 | 76 => return Some(KeyInput::Character("\n".into())),
        48 => return Some(KeyInput::Character("\t".into())),
        _ => {}
    }
    if text.is_empty() {
        return None;
    }
    let Ok(text) = String::from_utf16(text) else {
        return Some(KeyInput::Reset);
    };
    let control =
        |c: char| (c as u32) < 32 || c == '\u{7F}' || (0xF700..=0xF8FF).contains(&(c as u32));
    if text.chars().any(control) {
        return Some(KeyInput::Reset);
    }
    Some(KeyInput::Character(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    #[test]
    fn shortcuts_and_navigation_reset() {
        assert_eq!(
            classify_parts(kCGEventFlagMaskCommand, 0, &utf16("a")),
            Some(KeyInput::Reset)
        );
        assert_eq!(
            classify_parts(kCGEventFlagMaskControl, 0, &utf16("a")),
            Some(KeyInput::Reset)
        );
        assert_eq!(classify_parts(0, 123, &[0xF702]), Some(KeyInput::Reset));
        assert_eq!(classify_parts(0, 53, &[0x1B]), Some(KeyInput::Reset));
        assert_eq!(classify_parts(0, 122, &[0xF704]), Some(KeyInput::Reset)); // F1
    }

    #[test]
    fn editing_keys() {
        assert_eq!(classify_parts(0, 51, &[0x7F]), Some(KeyInput::Backspace));
        assert_eq!(
            classify_parts(0, 36, &[0x0D]),
            Some(KeyInput::Character("\n".into()))
        );
        assert_eq!(
            classify_parts(0, 48, &[0x09]),
            Some(KeyInput::Character("\t".into()))
        );
    }

    #[test]
    fn text_keys() {
        // Shift and Option still produce text.
        assert_eq!(
            classify_parts(1 << 17, 0, &utf16("A")),
            Some(KeyInput::Character("A".into()))
        );
        assert_eq!(
            classify_parts(1 << 19, 0, &utf16("å")),
            Some(KeyInput::Character("å".into()))
        );
        assert_eq!(
            classify_parts(0, 0, &utf16("😀")),
            Some(KeyInput::Character("😀".into()))
        );
        // Dead key: no text yet, no context change.
        assert_eq!(classify_parts(1 << 19, 14, &[]), None);
    }
}
