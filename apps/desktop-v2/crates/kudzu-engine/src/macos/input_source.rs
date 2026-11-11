//! The selected keyboard input source, cached for the tap and worker threads.
//!
//! Text Input Sources may only be called on the main thread, and the tap callback must never
//! wait. So the main thread reads the source (when macOS says it changed, and once a second as a
//! backstop) and publishes the answers as atomics that any thread reads for free.

use std::ffi::c_void;
use std::hash::{Hash, Hasher};
use std::panic::catch_unwind;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::Once;

use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation_sys::base::{CFEqual, CFGetTypeID, CFRelease, CFTypeRef};
use core_foundation_sys::data::{CFDataGetBytePtr, CFDataGetLength, CFDataGetTypeID, CFDataRef};
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::notification_center::{
    CFNotificationCenterAddObserver, CFNotificationCenterGetDistributedCenter,
    CFNotificationCenterRef, CFNotificationName,
    CFNotificationSuspensionBehaviorDeliverImmediately,
};
use core_foundation_sys::number::{kCFBooleanTrue, CFBooleanGetTypeID};
use core_foundation_sys::string::{CFStringGetTypeID, CFStringRef};

use super::ffi::*;
use crate::input::{self, InputSource, SourceKind};

/// ANSI positions of V and C, used until (and whenever) the layout can't be read.
const ANSI_V: CGKeyCode = 9;
const ANSI_C: CGKeyCode = 8;

static COMPOSING: AtomicBool = AtomicBool::new(false);
static PASTE_KEY: AtomicU16 = AtomicU16::new(ANSI_V);
static COPY_KEY: AtomicU16 = AtomicU16::new(ANSI_C);
/// Bumped whenever the selected source changes.
static EPOCH: AtomicU64 = AtomicU64::new(0);
/// Identifies what the last refresh saw.
static SEEN: AtomicU64 = AtomicU64::new(0);
static REFRESH_QUEUED: AtomicBool = AtomicBool::new(false);

/// Whether keys may currently go into an input method's composition (see [`InputSource::composes`]).
pub(crate) fn composing() -> bool {
    COMPOSING.load(Ordering::Relaxed)
}

/// Changes whenever the selected input source does; typed context from before is void.
pub(crate) fn epoch() -> u64 {
    EPOCH.load(Ordering::Relaxed)
}

/// The key that is "v" under Command on the current layout.
pub(crate) fn paste_key() -> CGKeyCode {
    PASTE_KEY.load(Ordering::Relaxed)
}

/// The key that is "c" under Command on the current layout.
pub(crate) fn copy_key() -> CGKeyCode {
    COPY_KEY.load(Ordering::Relaxed)
}

/// Starts following the input source. Safe to call repeatedly and from any thread; everything
/// happens later on the main thread, so without a main run loop (unit tests) the defaults stay.
pub(crate) fn install() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| on_main(start));
}

/// Asks the main thread to re-read the source. Cheap, never blocks, coalesces.
pub(crate) fn request_refresh() {
    if !REFRESH_QUEUED.swap(true, Ordering::AcqRel) {
        on_main(queued_refresh);
    }
}

fn on_main(work: DispatchFunction) {
    // SAFETY: the main queue is a process-lifetime object; `work` takes no context.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, ptr::null_mut(), work) };
}

extern "C" fn start(_context: *mut c_void) {
    // A panic must never unwind into libdispatch.
    let _ = catch_unwind(|| {
        // SAFETY: registers a plain function for the process lifetime; no observer or object pointers.
        unsafe {
            CFNotificationCenterAddObserver(
                CFNotificationCenterGetDistributedCenter(),
                ptr::null(),
                source_changed,
                kTISNotifySelectedKeyboardInputSourceChanged,
                ptr::null(),
                CFNotificationSuspensionBehaviorDeliverImmediately,
            );
        }
        refresh();
    });
}

extern "C" fn queued_refresh(_context: *mut c_void) {
    REFRESH_QUEUED.store(false, Ordering::Release);
    let _ = catch_unwind(refresh);
}

extern "C" fn source_changed(
    _center: CFNotificationCenterRef,
    _observer: *mut c_void,
    _name: CFNotificationName,
    _object: *const c_void,
    _info: CFDictionaryRef,
) {
    // Void the typed context at once, even if the re-read has to hop threads.
    EPOCH.fetch_add(1, Ordering::Relaxed);
    // SAFETY: no arguments, no preconditions.
    if unsafe { pthread_main_np() } != 0 {
        let _ = catch_unwind(refresh);
    } else {
        request_refresh();
    }
}

/// Reads the selected source and layout and publishes them. Main thread only.
fn refresh() {
    // SAFETY: on the main thread; every copied source is released exactly once below.
    let (composing, id, keys) = unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        let (composing, id) = if source.is_null() {
            (false, None)
        } else {
            let described = (
                composes(source),
                string_property(source, kTISPropertyInputSourceID),
            );
            CFRelease(source as CFTypeRef);
            described
        };
        // Under an input method this is the layout it types through.
        let keys = shortcut_keys_of(TISCopyCurrentKeyboardLayoutInputSource())
            .or_else(|| shortcut_keys_of(TISCopyCurrentASCIICapableKeyboardLayoutInputSource()))
            .unwrap_or((ANSI_V, ANSI_C));
        (composing, id, keys)
    };
    COMPOSING.store(composing, Ordering::Relaxed);
    PASTE_KEY.store(keys.0, Ordering::Relaxed);
    COPY_KEY.store(keys.1, Ordering::Relaxed);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (composing, id, keys).hash(&mut hasher);
    if SEEN.swap(hasher.finish(), Ordering::Relaxed) != hasher.finish() {
        EPOCH.fetch_add(1, Ordering::Relaxed);
    }
}

/// Whether `source` (not null) may compose. Main thread only.
pub(crate) unsafe fn composes(source: TISInputSourceRef) -> bool {
    unsafe {
        let kind = TISGetInputSourceProperty(source, kTISPropertyInputSourceType) as CFTypeRef;
        let is = |name: CFStringRef| !kind.is_null() && CFEqual(kind, name as CFTypeRef) != 0;
        let kind = if is(kTISTypeKeyboardLayout) {
            SourceKind::Layout
        } else if is(kTISTypeKeyboardInputMode)
            || is(kTISTypeKeyboardInputMethodWithoutModes)
            || is(kTISTypeKeyboardInputMethodModeEnabled)
        {
            SourceKind::InputMethod
        } else {
            SourceKind::Unknown
        };
        let ascii =
            TISGetInputSourceProperty(source, kTISPropertyInputSourceIsASCIICapable) as CFTypeRef;
        let ascii_capable = !ascii.is_null()
            && CFGetTypeID(ascii) == CFBooleanGetTypeID()
            && CFEqual(ascii, kCFBooleanTrue as CFTypeRef) != 0;
        let input_mode = string_property(source, kTISPropertyInputModeID);
        InputSource {
            kind,
            ascii_capable,
            input_mode: input_mode.as_deref(),
        }
        .composes()
    }
}

/// A string property of `source` (not null), copied out. Main thread only.
pub(crate) unsafe fn string_property(
    source: TISInputSourceRef,
    key: CFStringRef,
) -> Option<String> {
    unsafe {
        let value = TISGetInputSourceProperty(source, key) as CFTypeRef;
        if value.is_null() || CFGetTypeID(value) != CFStringGetTypeID() {
            return None;
        }
        // Get rule: retained for the duration of the copy, released when the wrapper drops.
        Some(CFString::wrap_under_get_rule(value as CFStringRef).to_string())
    }
}

/// Takes ownership of `layout` (may be null) and releases it.
unsafe fn shortcut_keys_of(layout: TISInputSourceRef) -> Option<(CGKeyCode, CGKeyCode)> {
    if layout.is_null() {
        return None;
    }
    unsafe {
        let keys = shortcut_keys(layout);
        CFRelease(layout as CFTypeRef);
        keys
    }
}

/// The key codes of ⌘V and ⌘C on `layout` (not null, borrowed); `None` when it has no Unicode
/// layout data. Translates with a throwaway dead-key state, so the user's own is never touched.
/// Main thread only.
pub(crate) unsafe fn shortcut_keys(layout: TISInputSourceRef) -> Option<(CGKeyCode, CGKeyCode)> {
    unsafe {
        // Get rule: the data belongs to `layout`, which the caller keeps alive.
        let data = TISGetInputSourceProperty(layout, kTISPropertyUnicodeKeyLayoutData) as CFDataRef;
        if data.is_null()
            || CFGetTypeID(data as CFTypeRef) != CFDataGetTypeID()
            || CFDataGetLength(data) <= 0
        {
            return None;
        }
        let bytes = CFDataGetBytePtr(data) as *const c_void;
        if bytes.is_null() {
            return None;
        }
        let keyboard = u32::from(LMGetKbdType());
        let under_command = |code: u16| -> Option<char> {
            let mut dead_keys = 0u32;
            let mut length = 0usize;
            let mut buffer = [0u16; 4];
            let status = UCKeyTranslate(
                bytes,
                code,
                kUCKeyActionDown,
                kUCModifierCommand,
                keyboard,
                kUCKeyTranslateNoDeadKeysMask,
                &mut dead_keys,
                buffer.len(),
                &mut length,
                buffer.as_mut_ptr(),
            );
            if status != 0 || length != 1 {
                return None;
            }
            char::from_u32(u32::from(buffer[0]))
        };
        Some((
            input::shortcut_key(under_command, 'v', ANSI_V),
            input::shortcut_key(under_command, 'c', ANSI_C),
        ))
    }
}
