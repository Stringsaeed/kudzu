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

fn from_ns_event(event: &NSEvent) -> Option<CurrentEvent> {
    let cg = cg_event(event)?;
    Some(CurrentEvent {
        timestamp: unsafe { cg_event_timestamp(cg) },
        key_code: key_code_of(event),
    })
}

/// SAFETY: `event` is a non-null `CGEventRef` from AppKit on the main thread.
unsafe fn cg_event_timestamp(event: *const c_void) -> u64 {
    unsafe { CGEventGetTimestamp(event) }
}

fn cg_event(event: &NSEvent) -> Option<*const c_void> {
    // SAFETY: NSEvent implements -CGEvent, returning a nullable CGEventRef.
    let ptr: *const c_void = unsafe { msg_send![event, CGEvent] };
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

    /// Map a CGEvent pointer the same way `from_ns_event` does: null means no stamp.
    fn stamp_of(cg_event: *const c_void) -> Option<u64> {
        if cg_event.is_null() {
            None
        } else {
            Some(unsafe { cg_event_timestamp(cg_event) })
        }
    }

    #[test]
    fn off_the_main_thread_there_is_no_event() {
        let from_worker = std::thread::spawn(current_event).join().expect("worker");
        assert_eq!(from_worker, None);
    }

    #[test]
    fn a_null_cgevent_is_no_stamp() {
        assert_eq!(stamp_of(std::ptr::null()), None);
    }

    #[test]
    fn main_thread_marker_is_absent_on_a_worker() {
        let marked = std::thread::spawn(|| MainThreadMarker::new().is_some())
            .join()
            .expect("worker");
        assert!(!marked);
    }
}
