//! Raw bindings to the CoreGraphics event, Accessibility and Carbon calls the engine uses.

#![allow(non_upper_case_globals, non_snake_case, dead_code)]

use std::ffi::c_void;

use core_foundation_sys::base::{CFTypeID, CFTypeRef};
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::mach_port::CFMachPortRef;
use core_foundation_sys::string::CFStringRef;

pub type CGEventRef = *mut c_void;
pub type CGEventSourceRef = *mut c_void;
pub type CGEventTapProxy = *mut c_void;
pub type AXUIElementRef = *mut c_void;
pub type AXValueRef = *mut c_void;
pub type AXError = i32;
pub type CGEventType = u32;
pub type CGEventFlags = u64;
pub type CGKeyCode = u16;
/// An opaque Text Input Sources object (a CF type; `Copy`/`Create` results are owned).
pub type TISInputSourceRef = *mut c_void;
pub type OSStatus = i32;

pub type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    kind: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

// CGEventType
pub const kCGEventLeftMouseDown: CGEventType = 1;
pub const kCGEventRightMouseDown: CGEventType = 3;
pub const kCGEventKeyDown: CGEventType = 10;
pub const kCGEventKeyUp: CGEventType = 11;
pub const kCGEventOtherMouseDown: CGEventType = 25;
pub const kCGEventTapDisabledByTimeout: CGEventType = 0xFFFF_FFFE;
pub const kCGEventTapDisabledByUserInput: CGEventType = 0xFFFF_FFFF;

// CGEventField
pub const kCGKeyboardEventAutorepeat: u32 = 8;
pub const kCGKeyboardEventKeycode: u32 = 9;
pub const kCGEventTargetUnixProcessID: u32 = 41;
pub const kCGEventSourceUserData: u32 = 42;
pub const kCGMouseEventWindowUnderMousePointer: u32 = 91;
pub const kCGMouseEventWindowUnderMousePointerThatCanHandleThisEvent: u32 = 92;
pub type CGWindowID = u32;

// CGEventFlags
pub const kCGEventFlagMaskControl: CGEventFlags = 1 << 18;
pub const kCGEventFlagMaskCommand: CGEventFlags = 1 << 20;

// CGEventTapLocation / placement / options
pub const kCGSessionEventTap: u32 = 1;
pub const kCGTailAppendEventTap: u32 = 1;
pub const kCGEventTapOptionDefault: u32 = 0;
pub const kCGEventTapOptionListenOnly: u32 = 1;

// CGEventSourceStateID
pub const kCGEventSourceStatePrivate: i32 = -1;

// AXValueType
pub const kAXValueCGPointType: u32 = 1;
pub const kAXValueCGSizeType: u32 = 2;
pub const kAXValueCGRectType: u32 = 3;

pub const kAXErrorSuccess: AXError = 0;

// UCKeyTranslate: key action, options, and the Command bit of a modifier state (`cmdKey >> 8`).
pub const kUCKeyActionDown: u16 = 0;
pub const kUCKeyTranslateNoDeadKeysMask: u32 = 1;
pub const kUCModifierCommand: u32 = 1;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    pub fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    pub fn CGEventGetFlags(event: CGEventRef) -> CGEventFlags;
    pub fn CGEventGetTimestamp(event: CGEventRef) -> u64;
    /// Only the protocol tests set timestamps (on events they never post).
    pub fn CGEventSetTimestamp(event: CGEventRef, timestamp: u64);
    pub fn CGEventSetFlags(event: CGEventRef, flags: CGEventFlags);
    pub fn CGEventKeyboardGetUnicodeString(
        event: CGEventRef,
        max_len: usize,
        actual_len: *mut usize,
        buffer: *mut u16,
    );
    pub fn CGEventKeyboardSetUnicodeString(event: CGEventRef, len: usize, buffer: *const u16);
    pub fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        key: CGKeyCode,
        key_down: bool,
    ) -> CGEventRef;
    pub fn CGEventCreateCopy(event: CGEventRef) -> CGEventRef;
    /// Only the protocol tests create mouse events (never posted).
    pub fn CGEventCreateMouseEvent(
        source: CGEventSourceRef,
        kind: CGEventType,
        location: CGPoint,
        button: u32,
    ) -> CGEventRef;
    pub fn CGEventPost(tap: u32, event: CGEventRef);
    pub fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
    pub fn CGPreflightListenEventAccess() -> bool;
    pub fn CGRequestListenEventAccess() -> bool;
    pub fn CGPreflightPostEventAccess() -> bool;
    pub fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn AXIsProcessTrusted() -> u8;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> u8;
    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementGetTypeID() -> CFTypeID;
    pub fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, seconds: f32) -> AXError;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXValueGetTypeID() -> CFTypeID;
    pub fn AXValueGetValue(value: AXValueRef, kind: u32, out: *mut c_void) -> u8;
}

#[link(name = "Carbon", kind = "framework")]
extern "C" {
    pub fn IsSecureEventInputEnabled() -> u8;

    // Text Input Sources (HIToolbox). Main thread only.
    pub static kTISPropertyInputSourceType: CFStringRef;
    pub static kTISPropertyInputSourceIsASCIICapable: CFStringRef;
    pub static kTISPropertyInputSourceID: CFStringRef;
    pub static kTISPropertyInputModeID: CFStringRef;
    pub static kTISPropertyUnicodeKeyLayoutData: CFStringRef;
    pub static kTISTypeKeyboardLayout: CFStringRef;
    pub static kTISTypeKeyboardInputMethodWithoutModes: CFStringRef;
    pub static kTISTypeKeyboardInputMethodModeEnabled: CFStringRef;
    pub static kTISTypeKeyboardInputMode: CFStringRef;
    pub static kTISNotifySelectedKeyboardInputSourceChanged: CFStringRef;
    pub fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    pub fn TISCopyCurrentKeyboardLayoutInputSource() -> TISInputSourceRef;
    pub fn TISCopyCurrentASCIICapableKeyboardLayoutInputSource() -> TISInputSourceRef;
    /// Get rule: the value belongs to `source`. Its type depends on `key`.
    pub fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *mut c_void;

    // CarbonCore.
    /// `layout` points at a `UCKeyboardLayout` (`uchr` data).
    pub fn UCKeyTranslate(
        layout: *const c_void,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        options: u32,
        dead_key_state: *mut u32,
        max_len: usize,
        actual_len: *mut usize,
        buffer: *mut u16,
    ) -> OSStatus;
    pub fn LMGetKbdType() -> u8;
}

/// `dispatch_function_t`.
pub type DispatchFunction = extern "C" fn(context: *mut c_void);

/// `struct dispatch_queue_s`, only ever used by address.
#[repr(C)]
pub struct DispatchQueue {
    _opaque: [u8; 0],
}

// libSystem (always linked).
extern "C" {
    /// The main queue object; `dispatch_get_main_queue()` is an inline function returning its address.
    pub static _dispatch_main_q: DispatchQueue;
    pub fn dispatch_async_f(
        queue: *const DispatchQueue,
        context: *mut c_void,
        work: DispatchFunction,
    );
    pub fn pthread_main_np() -> i32;
}
