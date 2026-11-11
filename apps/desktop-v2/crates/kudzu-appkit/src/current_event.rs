//! NSApp.currentEvent as a CGEvent stamp and optional key code (S57).
//!
//! The form window reports this as `PanelFocus.after` and `FormSubmission.event`. A missing
//! event is `None`; nothing here invents a timestamp.
use objc2::msg_send;
use objc2_app_kit::{NSApplication, NSEvent, NSEventType};
use objc2_foundation::MainThreadMarker;
use std::ffi::c_void;

/// CGEventGetTimestamp and, for a key event, kCGKeyboardEventKeycode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentEvent {
    pub timestamp: u64,
    pub key_code: Option<u16>,
}

/// Opaque `CGEventRef`. objc2 debug verification requires `^{__CGEvent=}`, not `^v`.
#[repr(C)]
struct CgEventOpaque {
    _p: [u8; 0],
}

unsafe impl objc2::RefEncode for CgEventOpaque {
    const ENCODING_REF: objc2::Encoding =
        objc2::Encoding::Pointer(&objc2::Encoding::Struct("__CGEvent", &[]));
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventGetTimestamp(event: *const c_void) -> u64;
}

/// `NSApp.currentEvent`'s CGEvent on the main thread. `None` off the main thread, when there
/// is no current event, or when that event has no CGEvent.
pub fn current_event() -> Option<CurrentEvent> {
    current_event_on(MainThreadMarker::new()?)
}

pub fn current_event_on(mtm: MainThreadMarker) -> Option<CurrentEvent> {
    let event = NSApplication::sharedApplication(mtm).currentEvent()?;
    from_ns_event(&event)
}

pub(crate) fn from_ns_event(event: &NSEvent) -> Option<CurrentEvent> {
    let cg = cg_event(event)?;
    Some(CurrentEvent {
        timestamp: unsafe { cg_event_timestamp(cg) },
        key_code: key_code_of(event),
    })
}

/// SAFETY: `event` is a non-null `CGEventRef` from AppKit.
unsafe fn cg_event_timestamp(event: *const CgEventOpaque) -> u64 {
    unsafe { CGEventGetTimestamp(event.cast()) }
}

fn cg_event(event: &NSEvent) -> Option<*const CgEventOpaque> {
    // SAFETY: NSEvent implements -CGEvent, returning a nullable CGEventRef.
    let ptr: *const CgEventOpaque = unsafe { msg_send![event, CGEvent] };
    if ptr.is_null() {
        None
    } else {
        Some(ptr)
    }
}

fn key_code_of(event: &NSEvent) -> Option<u16> {
    let ty = event.r#type();
    matches!(
        ty,
        NSEventType::KeyDown | NSEventType::KeyUp | NSEventType::FlagsChanged
    )
    .then(|| event.keyCode())
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_app_kit::NSEventModifierFlags;
    use objc2_foundation::{NSPoint, NSString};

    #[test]
    fn off_the_main_thread_there_is_no_event() {
        let from_worker = std::thread::spawn(current_event).join().expect("worker");
        assert_eq!(from_worker, None);
    }

    #[test]
    fn a_synthetic_return_key_has_a_stamp_and_key_code() {
        let text = NSString::from_str("\r");
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::empty(),
            0.0,
            0,
            None,
            &text,
            &text,
            false,
            36,
        )
        .expect("synthetic key event");
        let current = from_ns_event(&event).expect("CGEvent stamp");
        assert_eq!(current.key_code, Some(36));
        let _ = current.timestamp;
    }
}
