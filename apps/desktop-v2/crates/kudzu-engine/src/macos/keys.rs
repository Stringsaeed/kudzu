//! Synthesized keystrokes, marked so our own tap ignores them.

use std::sync::OnceLock;

use unicode_segmentation::UnicodeSegmentation;

use super::events::Events;
use super::ffi::*;
use super::input_source;
use crate::input::{self, Retyped};

/// `eventSourceUserData` on events we synthesize; the tap lets them through unmatched.
pub(crate) const SYNTHETIC_MARKER: i64 = 0x004B_5544_5A55;
/// `eventSourceUserData` on held physical keys we re-post; the tap matches them like real typing.
pub(crate) const REPLAY_MARKER: i64 = 0x004B_5552_504C;

pub(crate) const KEY_DELETE: CGKeyCode = 51;
pub(crate) const KEY_LEFT: CGKeyCode = 123;
pub(crate) const KEY_RIGHT: CGKeyCode = 124;
pub(crate) const KEY_DOWN: CGKeyCode = 125;
pub(crate) const KEY_UP: CGKeyCode = 126;
pub(crate) const KEY_TAB: CGKeyCode = 48;
pub(crate) const KEY_RETURN: CGKeyCode = 36;
pub(crate) const KEY_ESCAPE: CGKeyCode = 53;
pub(crate) const KEY_SPACE: CGKeyCode = 49;
pub(crate) const KEY_FORWARD_DELETE: CGKeyCode = 117;
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
    SOURCE
        .get_or_init(|| Source(unsafe { CGEventSourceCreate(kCGEventSourceStatePrivate) }))
        .0
}

/// Whether synthetic input can reach other apps right now.
pub(crate) fn can_post() -> bool {
    unsafe { CGPreflightPostEventAccess() && IsSecureEventInputEnabled() == 0 }
}

/// Posts one key press as its two halves: `create(down)` makes the key-down or the key-up
/// (`None` when it can't be created) and `post` sends it. Both are created before either is
/// posted, so a failure posts nothing: a key-down sent without its key-up would leave the key
/// pressed and its effect (a deleted character, a paste) applied while the caller is told the
/// press failed.
pub(crate) fn press_with<E>(
    mut create: impl FnMut(bool) -> Option<E>,
    mut post: impl FnMut(E),
) -> Result<(), KeyError> {
    let down = create(true).ok_or(KeyError::EventCreation)?;
    let up = create(false).ok_or(KeyError::EventCreation)?;
    post(down);
    post(up);
    Ok(())
}

/// A synthetic key event, released when dropped.
struct Synthetic(CGEventRef);

impl Synthetic {
    /// Creates and configures one half of a key press, marked [`SYNTHETIC_MARKER`].
    fn new(key: CGKeyCode, down: bool, flags: CGEventFlags, unicode: &[u16]) -> Option<Self> {
        // SAFETY: the event is created and configured here; `Drop` releases it.
        unsafe {
            let event = CGEventCreateKeyboardEvent(source(), key, down);
            if event.is_null() {
                return None;
            }
            CGEventSetFlags(event, flags);
            CGEventSetIntegerValueField(event, kCGEventSourceUserData, SYNTHETIC_MARKER);
            if !unicode.is_empty() {
                CGEventKeyboardSetUnicodeString(event, unicode.len(), unicode.as_ptr());
            }
            Some(Synthetic(event))
        }
    }

    fn post(self) {
        unsafe { CGEventPost(kCGSessionEventTap, self.0) };
    }
}

impl Drop for Synthetic {
    fn drop(&mut self) {
        unsafe { core_foundation_sys::base::CFRelease(self.0 as _) };
    }
}

/// Posts one key press (down, then up) marked [`SYNTHETIC_MARKER`]: the real
/// [`Events::press`](super::events::Events::press).
pub(super) fn post(key: CGKeyCode, flags: CGEventFlags, unicode: &[u16]) -> Result<(), KeyError> {
    if !can_post() {
        return Err(KeyError::NotAllowed);
    }
    press_with(
        |down| Synthetic::new(key, down, flags, unicode),
        Synthetic::post,
    )
}

fn repeat(events: &dyn Events, key: CGKeyCode, count: usize) -> Result<(), KeyError> {
    for _ in 0..count {
        events.press(key, 0, &[])?;
    }
    Ok(())
}

/// Presses Delete `count` times.
pub(crate) fn delete_backward(events: &dyn Events, count: usize) -> Result<(), KeyError> {
    repeat(events, KEY_DELETE, count)
}

/// Presses Left Arrow `count` times.
pub(crate) fn move_left(events: &dyn Events, count: usize) -> Result<(), KeyError> {
    repeat(events, KEY_LEFT, count)
}

/// Presses Right Arrow `count` times.
pub(crate) fn move_right(events: &dyn Events, count: usize) -> Result<(), KeyError> {
    repeat(events, KEY_RIGHT, count)
}

/// The virtual key code of a `{{key}}` tag's key (ANSI layout-independent keys only).
fn key_code(key: kudzu_core::template::Key) -> CGKeyCode {
    use kudzu_core::template::Key;
    match key {
        Key::Tab => KEY_TAB,
        Key::Return => KEY_RETURN,
        Key::Escape => KEY_ESCAPE,
        Key::Left => KEY_LEFT,
        Key::Right => KEY_RIGHT,
        Key::Up => KEY_UP,
        Key::Down => KEY_DOWN,
        Key::Backspace => KEY_DELETE,
        Key::Delete => KEY_FORWARD_DELETE,
        Key::Space => KEY_SPACE,
    }
}

/// Presses `key` `count` times, without modifiers.
pub(crate) fn press_key(
    events: &dyn Events,
    key: kudzu_core::template::Key,
    count: u32,
) -> Result<(), KeyError> {
    repeat(events, key_code(key), count as usize)
}

/// Presses ⌘C: Command with whichever key is "c" on the current layout.
pub(crate) fn copy(events: &dyn Events) -> Result<(), KeyError> {
    events.press(input_source::copy_key(), kCGEventFlagMaskCommand, &[])
}

/// Presses ⌘V: Command with whichever key is "v" on the current layout.
pub(crate) fn paste(events: &dyn Events) -> Result<(), KeyError> {
    events.press(input_source::paste_key(), kCGEventFlagMaskCommand, &[])
}

/// Types `text` as Unicode keystrokes, chunked on grapheme boundaries so emoji ZWJ sequences
/// and combining marks never split across events.
pub(crate) fn type_text(events: &dyn Events, text: &str) -> Result<(), KeyError> {
    for chunk in chunks(text) {
        events.press(0, 0, &chunk)?;
    }
    Ok(())
}

/// Types again what the user typed (a withheld key, a trigger, a separator). Unlike
/// [`type_text`], Return and Tab are pressed as their own keys, as the user pressed them.
pub(crate) fn retype(events: &dyn Events, text: &str) -> Result<(), KeyError> {
    for piece in input::retyped(text) {
        match piece {
            Retyped::Text(text) => type_text(events, text)?,
            Retyped::Return => events.press(KEY_RETURN, 0, &[])?,
            Retyped::Tab => events.press(KEY_TAB, 0, &[])?,
        }
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
    fn every_template_key_has_its_own_key_code() {
        let codes: std::collections::HashSet<CGKeyCode> = kudzu_core::template::Key::ALL
            .into_iter()
            .map(key_code)
            .collect();
        assert_eq!(codes.len(), kudzu_core::template::Key::ALL.len());
        assert_eq!(key_code(kudzu_core::template::Key::Tab), 48);
        assert_eq!(key_code(kudzu_core::template::Key::Return), 36);
        assert_eq!(key_code(kudzu_core::template::Key::Backspace), 51);
        assert_eq!(key_code(kudzu_core::template::Key::Delete), 117);
    }

    /// Both halves exist before either is posted: a key-down whose key-up can't be created
    /// would leave the key pressed, and its effect (a deleted character, a paste) applied.
    #[test]
    fn a_press_posts_nothing_unless_both_halves_were_created() {
        for refused in [true, false] {
            let mut posted = Vec::new();
            let result = press_with(
                |down| (down != refused).then_some(down),
                |half| posted.push(half),
            );
            assert_eq!(result, Err(KeyError::EventCreation));
            assert!(posted.is_empty(), "key-down refused: {refused}");
        }
        let mut posted = Vec::new();
        assert_eq!(press_with(Some, |half| posted.push(half)), Ok(()));
        assert_eq!(posted, [true, false]);
    }

    #[test]
    fn chunks_respect_graphemes() {
        let family = "👨‍👩‍👧‍👦"; // 11 UTF-16 units, one grapheme
        let text = format!("abcdefghij{family}{family}");
        let parts = chunks(&text);
        let lengths: Vec<usize> = parts.iter().map(Vec::len).collect();
        assert_eq!(lengths, vec![10, 11, 11]);
        assert_eq!(String::from_utf16(&parts[1]).unwrap(), family);
        assert_eq!(
            chunks(&"x".repeat(45))
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            vec![20, 20, 5]
        );
        assert!(chunks("").is_empty());
        let joined: Vec<u16> = parts.concat();
        assert_eq!(String::from_utf16(&joined).unwrap(), text);
    }
}
