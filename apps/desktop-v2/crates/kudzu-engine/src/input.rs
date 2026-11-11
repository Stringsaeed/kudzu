//! Platform-independent decisions about keyboard input: whether the input source composes text,
//! which key a shortcut letter sits on, what a dead key or a held key does to the next keystroke,
//! and how re-typed text maps to key codes.

/// Input mode ID of an input method's plain Latin mode (Japanese "Romaji"), shared by Apple's and
/// third-party input methods (`kTextServiceInputModeRoman`).
pub(crate) const ROMAN_INPUT_MODE: &str = "com.apple.inputmethod.Roman";

/// What kind of keyboard input source is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    /// A keyboard layout (`kTISTypeKeyboardLayout`): every key produces its text directly.
    Layout,
    /// An input method or one of its modes: keys may go into a composition first.
    InputMethod,
    /// Not reported. Treated like a layout, which is how the engine behaved before it looked.
    Unknown,
}

/// The selected keyboard input source, as Text Input Sources describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputSource<'a> {
    pub kind: SourceKind,
    /// `kTISPropertyInputSourceIsASCIICapable`.
    pub ascii_capable: bool,
    /// `kTISPropertyInputModeID`.
    pub input_mode: Option<&'a str>,
}

impl InputSource<'_> {
    /// Whether typed keys may be held in a composition (marked text) instead of reaching the
    /// document as the text their key events carry. While this is true the keystrokes say nothing
    /// reliable about the document, so triggers must not match and text must not be typed.
    ///
    /// A layout never composes, non-Latin ones included. An input method always may, except in its
    /// ASCII-capable Roman mode. Some input methods that produce Latin text (Vietnamese Telex) are
    /// ASCII-capable and still compose, so that flag alone is not enough.
    pub fn composes(&self) -> bool {
        match self.kind {
            SourceKind::Layout | SourceKind::Unknown => false,
            SourceKind::InputMethod => {
                !(self.ascii_capable && self.input_mode == Some(ROMAN_INPUT_MODE))
            }
        }
    }
}

/// Virtual key codes a layout can map (the range `UCKeyTranslate` accepts).
pub(crate) const KEY_CODES: std::ops::Range<u16> = 0..128;

/// The key code that produces `letter` under Command, given what each key code produces there.
///
/// `ansi` (the key's position on a US keyboard) wins when it produces the letter, and is the
/// answer when no key does, so layouts whose Command table is QWERTY keep the usual key.
pub(crate) fn shortcut_key(produces: impl Fn(u16) -> Option<char>, letter: char, ansi: u16) -> u16 {
    let hit = |code: u16| produces(code).is_some_and(|c| c.eq_ignore_ascii_case(&letter));
    if hit(ansi) {
        return ansi;
    }
    KEY_CODES
        .into_iter()
        .find(|&code| hit(code))
        .unwrap_or(ansi)
}

/// How the keystrokes just before a key-down affect what that key-down means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Follow {
    /// Nothing pending: the key's text is what the document receives.
    Plain,
    /// The key completed a dead-key sequence. Its text is right, but the app only commits it when
    /// it sees this key, so the key must not be withheld.
    Composed,
    /// The document may not have received what the keys said (an accent popup was open, or a dead
    /// key was cancelled): the typed context is unknown.
    Broken,
}

/// Remembers a pending dead key and a held (auto-repeating) key between key-downs.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyContext {
    dead_key: bool,
    repeated: bool,
}

impl KeyContext {
    /// A key-down on a character key that produced no text: a dead key waiting for its base letter.
    pub fn silent(&mut self) {
        self.dead_key = true;
    }

    /// An auto-repeated key-down on a key that may have opened the accent popup, where the next
    /// key picks an accent rather than typing.
    pub fn repeated(&mut self) {
        self.repeated = true;
    }

    /// Forgets everything (click, focus change, input source change).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// A key-down that produced text (`is_text`) or an editing/navigation key. Consumes the state.
    pub fn next(&mut self, is_text: bool) -> Follow {
        let was = std::mem::take(self);
        if was.repeated {
            Follow::Broken
        } else if !was.dead_key {
            Follow::Plain
        } else if is_text {
            Follow::Composed
        } else {
            Follow::Broken
        }
    }
}

/// Whether holding the key that types `text` can open the press-and-hold accent popup. Only
/// letters have one; a held Backspace, arrow or punctuation key just repeats.
pub(crate) fn can_open_accent_popup(text: &str) -> bool {
    text.chars().any(char::is_alphabetic)
}

/// Whether a key-down without text on `key` can be a dead key. Dead keys only sit on the main
/// character block; other textless keys (JIS Eisu/Kana, system keys) change no pending state.
pub(crate) fn could_be_dead_key(key: i64) -> bool {
    // 36 and 48 are Return and Tab; 93 and 94 are the JIS Yen and Underscore keys.
    matches!(key, 0..=50 | 93 | 94) && key != 36 && key != 48
}

/// A piece of text the user typed that is typed again for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retyped<'a> {
    /// Posted as a Unicode string.
    Text(&'a str),
    Return,
    Tab,
}

/// Splits re-typed text so Return and Tab are pressed as their own keys. Apps and remote-desktop
/// clients that read key codes would otherwise see the placeholder key that carries a string.
pub(crate) fn retyped(text: &str) -> Vec<Retyped<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        let key = match c {
            '\n' => Retyped::Return,
            '\r' => {
                // CRLF is one Return.
                chars.next_if(|&(_, next)| next == '\n');
                Retyped::Return
            }
            '\t' => Retyped::Tab,
            _ => continue,
        };
        if start < index {
            out.push(Retyped::Text(&text[start..index]));
        }
        out.push(key);
        start = chars.peek().map_or(text.len(), |&(next, _)| next);
    }
    if start < text.len() {
        out.push(Retyped::Text(&text[start..]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(kind: SourceKind, ascii_capable: bool, input_mode: Option<&str>) -> InputSource<'_> {
        InputSource {
            kind,
            ascii_capable,
            input_mode,
        }
    }

    #[test]
    fn layouts_never_compose() {
        // ABC, Dvorak.
        assert!(!source(SourceKind::Layout, true, None).composes());
        // Russian, Arabic, Hebrew: not ASCII-capable, still one key one character.
        assert!(!source(SourceKind::Layout, false, None).composes());
    }

    #[test]
    fn input_methods_compose_outside_their_roman_mode() {
        // Japanese Hiragana, Pinyin, Korean 2-Set.
        assert!(source(
            SourceKind::InputMethod,
            false,
            Some("com.apple.inputmethod.Japanese")
        )
        .composes());
        assert!(source(
            SourceKind::InputMethod,
            false,
            Some("com.apple.inputmethod.SCIM.ITABC")
        )
        .composes());
        assert!(source(
            SourceKind::InputMethod,
            false,
            Some("com.apple.inputmethod.Korean.2SetKorean")
        )
        .composes());
        // An input method without modes.
        assert!(source(SourceKind::InputMethod, false, None).composes());
        // Vietnamese Telex produces Latin text and still composes ("aa" becomes "â").
        assert!(source(
            SourceKind::InputMethod,
            true,
            Some("com.apple.inputmethod.VietnameseTelex")
        )
        .composes());
        assert!(source(SourceKind::InputMethod, true, None).composes());
        // Full-width Roman is a Roman-named mode that isn't ASCII.
        assert!(source(
            SourceKind::InputMethod,
            false,
            Some("com.apple.inputmethod.Japanese.FullWidthRoman")
        )
        .composes());
        assert!(source(SourceKind::InputMethod, false, Some(ROMAN_INPUT_MODE)).composes());
    }

    #[test]
    fn roman_mode_and_unknown_sources_match_like_a_layout() {
        assert!(!source(SourceKind::InputMethod, true, Some(ROMAN_INPUT_MODE)).composes());
        assert!(!source(SourceKind::Unknown, false, None).composes());
        assert!(!source(
            SourceKind::Unknown,
            true,
            Some("com.apple.inputmethod.Japanese")
        )
        .composes());
    }

    /// A Command table from (key code, character) pairs.
    fn table(pairs: &'static [(u16, char)]) -> impl Fn(u16) -> Option<char> {
        move |code| pairs.iter().find(|(c, _)| *c == code).map(|(_, ch)| *ch)
    }

    #[test]
    fn shortcut_key_follows_the_layout() {
        // QWERTY, AZERTY, QWERTZ, and "Dvorak – QWERTY ⌘" (whose Command table is QWERTY).
        let qwerty = table(&[(8, 'c'), (9, 'v'), (47, '.'), (34, 'i')]);
        assert_eq!(shortcut_key(&qwerty, 'v', 9), 9);
        assert_eq!(shortcut_key(&qwerty, 'c', 8), 8);
        // Dvorak: physical V is "k", "v" is on the physical "." key; "c" is on physical "I".
        let dvorak = table(&[(8, 'j'), (9, 'k'), (47, 'v'), (34, 'c')]);
        assert_eq!(shortcut_key(&dvorak, 'v', 9), 47);
        assert_eq!(shortcut_key(&dvorak, 'c', 8), 34);
        // Workman: "v" is on physical B, "c" on physical V.
        let workman = table(&[(8, 'm'), (9, 'c'), (11, 'v')]);
        assert_eq!(shortcut_key(&workman, 'v', 9), 11);
        assert_eq!(shortcut_key(&workman, 'c', 8), 9);
    }

    #[test]
    fn shortcut_key_falls_back_to_the_ansi_position() {
        // A layout with no Latin letters under Command, or no table at all.
        assert_eq!(shortcut_key(table(&[(9, 'м'), (8, 'с')]), 'v', 9), 9);
        assert_eq!(shortcut_key(|_| None, 'c', 8), 8);
        // Upper-case output still counts, and the ANSI key wins over a duplicate elsewhere.
        assert_eq!(shortcut_key(table(&[(47, 'V')]), 'v', 9), 47);
        assert_eq!(shortcut_key(table(&[(3, 'v'), (9, 'v')]), 'v', 9), 9);
    }

    #[test]
    fn plain_typing_has_no_pending_state() {
        let mut keys = KeyContext::default();
        assert_eq!(keys.next(true), Follow::Plain);
        assert_eq!(keys.next(false), Follow::Plain);
    }

    #[test]
    fn a_dead_key_marks_only_the_next_key() {
        let mut keys = KeyContext::default();
        keys.silent();
        assert_eq!(keys.next(true), Follow::Composed);
        assert_eq!(keys.next(true), Follow::Plain);
        // Two dead keys in a row are still one pending composition.
        keys.silent();
        keys.silent();
        assert_eq!(keys.next(true), Follow::Composed);
        assert_eq!(keys.next(true), Follow::Plain);
    }

    #[test]
    fn a_cancelled_dead_key_breaks_the_context() {
        let mut keys = KeyContext::default();
        keys.silent();
        // Backspace or an arrow removes the pending accent, not a typed character.
        assert_eq!(keys.next(false), Follow::Broken);
        assert_eq!(keys.next(true), Follow::Plain);
        keys.silent();
        keys.clear();
        assert_eq!(keys.next(true), Follow::Plain);
    }

    #[test]
    fn the_key_after_a_held_key_is_not_trusted() {
        let mut keys = KeyContext::default();
        keys.repeated();
        keys.repeated();
        // "1" in the accent popup replaces the letter instead of typing a digit.
        assert_eq!(keys.next(true), Follow::Broken);
        assert_eq!(keys.next(true), Follow::Plain);
        keys.repeated();
        keys.silent();
        assert_eq!(keys.next(true), Follow::Broken);
    }

    #[test]
    fn only_letters_open_the_accent_popup() {
        assert!(can_open_accent_popup("e"));
        assert!(can_open_accent_popup("E"));
        assert!(can_open_accent_popup("я"));
        assert!(!can_open_accent_popup("-"));
        assert!(!can_open_accent_popup(" "));
        assert!(!can_open_accent_popup("\n"));
        assert!(!can_open_accent_popup("1"));
        assert!(!can_open_accent_popup(""));
    }

    #[test]
    fn dead_keys_sit_on_character_keys_only() {
        assert!(could_be_dead_key(14)); // E (Option-E)
        assert!(could_be_dead_key(50)); // grave
        assert!(could_be_dead_key(10)); // ISO section key
        assert!(could_be_dead_key(93)); // JIS Yen
        assert!(!could_be_dead_key(36));
        assert!(!could_be_dead_key(48));
        assert!(!could_be_dead_key(51));
        assert!(!could_be_dead_key(102)); // JIS Eisu
        assert!(!could_be_dead_key(104)); // JIS Kana
        assert!(!could_be_dead_key(63)); // Fn
        assert!(!could_be_dead_key(179));
        assert!(!could_be_dead_key(-1));
    }

    #[test]
    fn retyped_text_presses_return_and_tab_as_keys() {
        assert_eq!(retyped("\n"), vec![Retyped::Return]);
        assert_eq!(retyped("\t"), vec![Retyped::Tab]);
        assert_eq!(retyped(" "), vec![Retyped::Text(" ")]);
        assert_eq!(
            retyped(";sig\n"),
            vec![Retyped::Text(";sig"), Retyped::Return]
        );
        assert_eq!(
            retyped("a\tb\r\nc"),
            vec![
                Retyped::Text("a"),
                Retyped::Tab,
                Retyped::Text("b"),
                Retyped::Return,
                Retyped::Text("c")
            ]
        );
        assert_eq!(
            retyped("\r\r\n\n"),
            vec![Retyped::Return, Retyped::Return, Retyped::Return]
        );
        assert_eq!(retyped("é😀\t"), vec![Retyped::Text("é😀"), Retyped::Tab]);
        assert!(retyped("").is_empty());
    }
}
