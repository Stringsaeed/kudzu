//! Synthesized keystrokes, marked so our own tap ignores them.

use std::sync::OnceLock;

use unicode_segmentation::UnicodeSegmentation;

use super::ffi::*;

/// `eventSourceUserData` on events we synthesize; the tap lets them through unmatched.
pub(crate) const SYNTHETIC_MARKER: i64 = 0x004B_5544_5A55;
/// `eventSourceUserData` on held physical keys we re-post; the tap matches them like real typing.
pub(crate) const REPLAY_MARKER: i64 = 0x004B_5552_504C;

const KEY_V: CGKeyCode = 9;
const KEY_C: CGKeyCode = 8;
const KEY_DELETE: CGKeyCode = 51;
const KEY_LEFT: CGKeyCode = 123;
const KEY_RIGHT: CGKeyCode = 124;
/// Longest Unicode string per synthetic event (apps truncate longer ones).
const CHUNK_UTF16: usize = 20;

/// Why a keystroke couldn't be posted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyError {
    /// No post-event permission, or Secure Input is on.
    NotAllowed,
    EventCreation,
}

struct Source(CGEventSourceRef);
// SAFETY: a CGEventSource is an immutable CF object once created; CoreGraphics allows sharing it.
unsafe impl Send for Source {}
unsafe impl Sync for Source {}

fn source() -> CGEventSourceRef {
    static SOURCE: OnceLock<Source> = OnceLock::new();
    // A private state source, so modifiers the user is physically holding don't leak into our keys.
    SOURCE.get_or_init(|| Source(unsafe { CGEventSourceCreate(kCGEventSourceStatePrivate) })).0
}

/// Whether synthetic input can reach other apps right now.
pub(crate) fn can_post() -> bool {
    unsafe { CGPreflightPostEventAccess() && IsSecureEventInputEnabled() == 0 }
}

fn press(key: CGKeyCode, flags: CGEventFlags, unicode: &[u16]) -> Result<(), KeyError> {
    if !can_post() {
        return Err(KeyError::NotAllowed);
    }
    for down in [true, false] {
        // SAFETY: the event is created, configured, posted and released here.
        unsafe {
            let event = CGEventCreateKeyboardEvent(source(), key, down);
            if event.is_null() {
                return Err(KeyError::EventCreation);
            }
            CGEventSetFlags(event, flags);
            CGEventSetIntegerValueField(event, kCGEventSourceUserData, SYNTHETIC_MARKER);
            if !unicode.is_empty() {
                CGEventKeyboardSetUnicodeString(event, unicode.len(), unicode.as_ptr());
            }
            CGEventPost(kCGSessionEventTap, event);
            core_foundation_sys::base::CFRelease(event as _);
        }
    }
    Ok(())
}

fn repeat(key: CGKeyCode, count: usize) -> Result<(), KeyError> {
    for _ in 0..count {
        press(key, 0, &[])?;
    }
    Ok(())
}

/// Presses Delete `count` times.
pub(crate) fn delete_backward(count: usize) -> Result<(), KeyError> {
    repeat(KEY_DELETE, count)
}

/// Presses Left Arrow `count` times.
pub(crate) fn move_left(count: usize) -> Result<(), KeyError> {
    repeat(KEY_LEFT, count)
}

/// Presses Right Arrow `count` times.
pub(crate) fn move_right(count: usize) -> Result<(), KeyError> {
    repeat(KEY_RIGHT, count)
}

/// Presses ⌘C.
pub(crate) fn copy() -> Result<(), KeyError> {
    press(KEY_C, kCGEventFlagMaskCommand, &[])
}

/// Presses ⌘V.
pub(crate) fn paste() -> Result<(), KeyError> {
    press(KEY_V, kCGEventFlagMaskCommand, &[])
}

/// Types `text` as Unicode keystrokes, chunked on grapheme boundaries so emoji ZWJ sequences
/// and combining marks never split across events.
pub(crate) fn type_text(text: &str) -> Result<(), KeyError> {
    for chunk in chunks(text) {
        press(0, 0, &chunk)?;
    }
    Ok(())
}

/// UTF-16 chunks of at most [`CHUNK_UTF16`] units (a longer single grapheme goes alone).
fn chunks(text: &str) -> Vec<Vec<u16>> {
    let mut out = Vec::new();
    let mut chunk: Vec<u16> = Vec::new();
    for g in text.graphemes(true) {
        let units: Vec<u16> = g.encode_utf16().collect();
        if !chunk.is_empty() && chunk.len() + units.len() > CHUNK_UTF16 {
            out.push(std::mem::take(&mut chunk));
        }
        chunk.extend(units);
    }
    if !chunk.is_empty() {
        out.push(chunk);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_respect_graphemes() {
        let family = "👨‍👩‍👧‍👦"; // 11 UTF-16 units, one grapheme
        let text = format!("abcdefghij{family}{family}");
        let parts = chunks(&text);
        let lengths: Vec<usize> = parts.iter().map(Vec::len).collect();
        assert_eq!(lengths, vec![10, 11, 11]);
        assert_eq!(String::from_utf16(&parts[1]).unwrap(), family);
        assert_eq!(chunks(&"x".repeat(45)).iter().map(Vec::len).collect::<Vec<_>>(), vec![20, 20, 5]);
        assert!(chunks("").is_empty());
        let joined: Vec<u16> = parts.concat();
        assert_eq!(String::from_utf16(&joined).unwrap(), text);
    }
}
