//! A shortcut owned by one screen, not by [`crate::actions::main::MainAction`].
//!
//! The cheat sheet reads the chord from here. The screen still handles the key.

/// Identity, chord and sheet label for a screen-local shortcut.
pub struct LocalShortcut {
    pub id: &'static str,
    pub chord: &'static str,
    pub label: &'static str,
}
