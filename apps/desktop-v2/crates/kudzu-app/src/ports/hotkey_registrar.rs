//! Registration is main-thread-only, including during rollback and shutdown.
//! Services never put this port in work joined by shutdown: Rune's off-thread call waits on UI.
use crate::error::PortError;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShortcutSlot {
    QuickSearch,
    Clipboard,
}
pub trait HotkeyRegistrar: Send + Sync {
    fn register(&self, slot: ShortcutSlot, chord: &str) -> Result<(), PortError>;
    fn unregister(&self, slot: ShortcutSlot) -> Result<(), PortError>;
}
