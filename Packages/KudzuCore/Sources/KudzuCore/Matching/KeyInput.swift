/// One keystroke-level event fed to the matcher by the app's event tap.
public enum KeyInput: Hashable, Sendable {
    /// Text produced by a key press. Usually one character, but may be several
    /// scalars (emoji, dead-key composition) or several grapheme clusters.
    case character(String)
    /// Delete-backward. Drops the last buffered character.
    case backspace
    /// Anything that makes the buffered context unreliable: click, app switch,
    /// arrow/navigation keys, Escape, modifier shortcuts.
    case reset
}
