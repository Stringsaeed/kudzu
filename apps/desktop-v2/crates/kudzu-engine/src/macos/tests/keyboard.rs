//! Key events on the fake desktop: building the user's keys as real (never posted) `CGEvent`s,
//! decoding what the engine posts, and applying a key that reached the target to its text.

use std::ptr;

use core_foundation_sys::base::CFRelease;

use super::desk::{plain, Out, State};
use crate::macos::ffi::*;
use crate::macos::input_source;
use crate::macos::keys::*;
use crate::macos::tap;

/// Key code and Unicode string of a key the user presses.
pub(super) fn user_key(key: &str) -> (CGKeyCode, String) {
    match key {
        "⌫" => (KEY_DELETE, "\u{7f}".into()),
        "\n" | "⏎" => (KEY_RETURN, "\r".into()),
        "\t" | "⇥" => (KEY_TAB, "\t".into()),
        other => (0, other.into()),
    }
}

/// The label of a physical key event (see [`Key`]).
pub(super) fn label(code: CGKeyCode, text: &str) -> String {
    match code {
        KEY_DELETE => "⌫".into(),
        KEY_RETURN => "⏎".into(),
        KEY_TAB => "⇥".into(),
        _ => text.into(),
    }
}

/// The Unicode string carried by a key event.
pub(super) fn unicode(event: CGEventRef) -> String {
    let mut buffer = [0u16; 32];
    let mut length = 0usize;
    unsafe {
        CGEventKeyboardGetUnicodeString(event, buffer.len(), &mut length, buffer.as_mut_ptr())
    };
    String::from_utf16_lossy(&buffer[..length.min(buffer.len())])
}

/// What a synthetic press is.
pub(super) fn decode(code: CGKeyCode, flags: CGEventFlags, text: &str) -> Out {
    if flags & kCGEventFlagMaskCommand != 0 {
        return if code == input_source::paste_key() {
            Out::Paste
        } else if code == input_source::copy_key() {
            Out::Copy
        } else {
            Out::Other(code)
        };
    }
    match code {
        KEY_DELETE => Out::Backspace,
        KEY_FORWARD_DELETE => Out::Delete,
        KEY_LEFT => Out::Left,
        KEY_RIGHT => Out::Right,
        KEY_UP => Out::Up,
        KEY_DOWN => Out::Down,
        KEY_RETURN => Out::Return,
        KEY_TAB => Out::Tab,
        KEY_ESCAPE => Out::Escape,
        KEY_SPACE => Out::Space,
        0 if !text.is_empty() => Out::Type(text.into()),
        other => Out::Other(other),
    }
}

/// Applies a key-down that reached the target app.
pub(super) fn apply_key(s: &mut State, code: CGKeyCode, flags: CGEventFlags, text: &str) {
    if flags & kCGEventFlagMaskCommand != 0 {
        if code == input_source::paste_key() {
            let pasted = s.doc.paste_inserts.clone().or_else(|| plain(&s.items));
            if let Some(pasted) = pasted {
                s.doc.insert(&pasted);
            }
        }
        return;
    }
    match code {
        KEY_DELETE => s.doc.backspace(),
        KEY_FORWARD_DELETE => s.doc.forward_delete(),
        KEY_LEFT => s.doc.left(),
        KEY_RIGHT => s.doc.right(),
        KEY_RETURN => s.doc.insert("\n"),
        KEY_TAB => s.doc.insert("\t"),
        KEY_SPACE => s.doc.insert(" "),
        KEY_UP | KEY_DOWN | KEY_ESCAPE => {}
        _ if !text.is_empty() => s.doc.insert(text),
        _ => {}
    }
}

/// Applies a physical key event (a held copy the engine let go) to the target.
pub(super) fn apply_event(s: &mut State, event: CGEventRef) {
    let code = tap::key_code(event) as CGKeyCode;
    let flags = unsafe { CGEventGetFlags(event) };
    apply_key(s, code, flags, &unicode(event));
}

/// A physical key event the user is about to press.
pub(super) struct KeyEvent {
    pub event: CgEvent,
    pub code: CGKeyCode,
    pub text: String,
    pub down: bool,
}

/// An owned `CGEvent`, created for a test and never posted.
pub(super) struct CgEvent(pub CGEventRef);

impl CgEvent {
    pub fn key(code: CGKeyCode, text: &str, down: bool) -> CgEvent {
        let units: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            let event = CGEventCreateKeyboardEvent(ptr::null_mut(), code, down);
            assert!(!event.is_null(), "CGEventCreateKeyboardEvent failed");
            CGEventSetFlags(event, 0);
            CGEventKeyboardSetUnicodeString(event, units.len(), units.as_ptr());
            CgEvent(event)
        }
    }

    /// A mouse-down on `window` (0: the event names no window), as the window server routes it.
    pub fn click(window: i64) -> CgEvent {
        CgEvent::click_fields(window, window)
    }

    /// A mouse-down whose window under the pointer is `under` and whose window that can handle
    /// it is `handles` (0: not named).
    pub fn click_fields(under: i64, handles: i64) -> CgEvent {
        let at = CGPoint { x: 10.0, y: 10.0 };
        unsafe {
            let event = CGEventCreateMouseEvent(ptr::null_mut(), kCGEventLeftMouseDown, at, 0);
            assert!(!event.is_null(), "CGEventCreateMouseEvent failed");
            CGEventSetIntegerValueField(event, kCGMouseEventWindowUnderMousePointer, under);
            CGEventSetIntegerValueField(
                event,
                kCGMouseEventWindowUnderMousePointerThatCanHandleThisEvent,
                handles,
            );
            CgEvent(event)
        }
    }
}

impl Drop for CgEvent {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as _) };
    }
}
