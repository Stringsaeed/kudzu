//! Seam: the session event stream, as the engine posts to it and listens on it.
//!
//! Everything the engine sends into other apps goes through [`Events`]: synthetic key presses,
//! held physical input posted again after an insertion, and the listener (the event tap).
//! [`SystemEvents`] is CoreGraphics. The protocol tests substitute a recorder, so no event
//! reaches the desktop.

use super::ffi::{CGEventFlags, CGKeyCode};
use super::keys::{self, KeyError};
use super::tap::{Handler, HeldEvent, TapHandle};
use crate::TapMode;

/// Where the engine's output goes, and where its input comes from.
pub(crate) trait Events: Send + Sync {
    /// Posts one key press (down, then up) marked as synthetic, so the listener lets it through
    /// unmatched. Fails when synthetic input can't reach other apps right now.
    fn press(&self, key: CGKeyCode, flags: CGEventFlags, unicode: &[u16]) -> Result<(), KeyError>;

    /// Posts a held physical event again, with `marker` in its source user data.
    fn post_held(&self, held: HeldEvent, marker: i64);

    /// Creates a listener in `mode`, disabled until [`Listener::enable`]. `None` when macOS
    /// refuses (missing permission). Dropping the listener stops it.
    fn listen(&self, mode: TapMode, handler: Handler) -> Option<Box<dyn Listener>>;
}

/// An attached listener. Dropping it stops it.
pub(crate) trait Listener: Send {
    /// How it was created.
    fn mode(&self) -> TapMode;
    /// Starts delivering events to its handler.
    fn enable(&self);
}

/// The real event stream: `CGEventPost` and a session `CGEventTap`.
pub(crate) struct SystemEvents;

impl Events for SystemEvents {
    fn press(&self, key: CGKeyCode, flags: CGEventFlags, unicode: &[u16]) -> Result<(), KeyError> {
        keys::post(key, flags, unicode)
    }

    fn post_held(&self, held: HeldEvent, marker: i64) {
        held.post(marker);
    }

    fn listen(&self, mode: TapMode, handler: Handler) -> Option<Box<dyn Listener>> {
        TapHandle::start(mode, handler).map(|tap| Box::new(tap) as Box<dyn Listener>)
    }
}

impl Listener for TapHandle {
    fn mode(&self) -> TapMode {
        TapHandle::mode(self)
    }

    fn enable(&self) {
        TapHandle::enable(self);
    }
}
