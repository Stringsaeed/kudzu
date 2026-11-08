//! Keyboard helpers (`lib/keys.ts`): one shortcut grammar (`"mod+shift+k"`) shared by
//! handlers and hint labels, plus global-hotkey accelerators (`"Control+Alt+Space"`).
//! `mac` selects macOS symbols and the meaning of `mod` (⌘ on a Mac, Ctrl elsewhere).

fn symbol(key: &str, mac: bool) -> Option<&'static str> {
    let (m, other) = match key {
        "mod" | "cmd" => ("⌘", "Ctrl"),
        "ctrl" | "control" => ("⌃", "Ctrl"),
        "alt" | "option" => ("⌥", "Alt"),
        "shift" => ("⇧", "Shift"),
        "enter" => ("↵", "Enter"),
        "backspace" => ("⌫", "Backspace"),
        "delete" => ("⌫", "Del"),
        "escape" => ("esc", "Esc"),
        "up" => ("↑", "↑"),
        "down" => ("↓", "↓"),
        "left" => ("←", "←"),
        "right" => ("→", "→"),
        "tab" => ("⇥", "Tab"),
        "space" => ("Space", "Space"),
        _ => return None,
    };
    Some(if mac { m } else { other })
}

fn single_upper(k: &str) -> String {
    if k.encode_utf16().count() == 1 {
        k.to_uppercase()
    } else {
        k.to_string()
    }
}

/// Splits a shortcut into display keys: `"mod+shift+k"` → `["⌘", "⇧", "K"]`.
pub fn key_labels(combo: &str, mac: bool) -> Vec<String> {
    combo.split('+').map(|k| symbol(&k.to_lowercase(), mac).map(str::to_string).unwrap_or_else(|| single_upper(k))).collect()
}

/// The parts of a keydown that shortcuts look at (DOM `KeyboardEvent` names).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeyEvent {
    /// `KeyboardEvent.key`, e.g. `"k"`, `"K"`, `"ArrowUp"`, `" "`.
    pub key: String,
    /// `KeyboardEvent.code`, e.g. `"KeyK"`, `"Digit1"`, `"Space"`.
    pub code: String,
    pub meta: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Whether a keydown matches a combo such as `"mod+k"`, `"shift+?"`, `"alt+up"`. Shift is
/// implied for symbols like `?`; it's only enforced when asked for, or for letters, digits
/// and a few named keys.
pub fn matches(e: &KeyEvent, combo: &str, mac: bool) -> bool {
    let combo = combo.to_lowercase();
    let parts: Vec<&str> = combo.split('+').collect();
    let key = *parts.last().unwrap_or(&"");
    let want = &parts[..parts.len() - 1];
    let has = |m: &str| want.contains(&m);
    let mod_down = if mac { e.meta } else { e.ctrl };
    if has("mod") != mod_down {
        return false;
    }
    if !mac && has("mod") && e.meta {
        return false;
    }
    if mac && !has("ctrl") && e.ctrl {
        return false;
    }
    if has("ctrl") && !e.ctrl {
        return false;
    }
    if has("alt") != e.alt {
        return false;
    }
    if has("shift") && !e.shift {
        return false;
    }
    let plain = key.len() == 1 && key.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    if !has("shift") && e.shift && (plain || matches!(key, "enter" | "backspace" | "delete" | "up" | "down" | "tab")) {
        return false;
    }
    let k = e.key.to_lowercase();
    let named = match key {
        "up" => Some("arrowup"),
        "down" => Some("arrowdown"),
        "left" => Some("arrowleft"),
        "right" => Some("arrowright"),
        "esc" => Some("escape"),
        "space" => Some(" "),
        "del" => Some("delete"),
        _ => None,
    };
    if let Some(named) = named {
        return k == named;
    }
    // `/[a-z0-9]/` on a one-unit key.
    if key.encode_utf16().count() == 1 && key.bytes().any(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        let code = if key.as_bytes()[0].is_ascii_digit() { format!("digit{key}") } else { format!("key{key}") };
        return k == key || e.code.to_lowercase() == code;
    }
    k == key
}

/// Accelerator string (`"Control+Alt+Space"`) → display keys (`["⌃", "⌥", "Space"]`).
pub fn accelerator_labels(accel: &str, mac: bool) -> Vec<String> {
    accel
        .split('+')
        .map(|p| {
            let k = p.to_lowercase();
            match k.as_str() {
                "commandorcontrol" | "cmdorctrl" => (if mac { "⌘" } else { "Ctrl" }).to_string(),
                "super" | "command" | "cmd" | "meta" => (if mac { "⌘" } else { "Win" }).to_string(),
                _ => symbol(&k, mac).map(str::to_string).unwrap_or_else(|| single_upper(p)),
            }
        })
        .collect()
}

const PUNCTUATION_CODES: [&str; 13] = ["Semicolon", "Quote", "Comma", "Period", "Slash", "Backslash", "BracketLeft", "BracketRight", "Backquote", "Minus", "Equal", "Enter", "Tab"];

fn is_fkey(s: &str) -> bool {
    s.strip_prefix('F').is_some_and(|d| (1..=2).contains(&d.len()) && d.bytes().all(|b| b.is_ascii_digit()))
}

/// Builds a Tauri accelerator from a keydown, or `None` when it's only modifiers, has no
/// modifier (function keys excepted), or only Shift.
pub fn accelerator_from_event(e: &KeyEvent, mac: bool) -> Option<String> {
    let mut mods = Vec::new();
    if e.ctrl {
        mods.push("Control");
    }
    if e.alt {
        mods.push("Alt");
    }
    if e.shift {
        mods.push("Shift");
    }
    if e.meta {
        mods.push(if mac { "Command" } else { "Super" });
    }
    let code = e.code.as_str();
    let key = if let Some(l) = code.strip_prefix("Key").filter(|l| l.len() == 1 && l.as_bytes()[0].is_ascii_uppercase()) {
        l
    } else if let Some(d) = code.strip_prefix("Digit").filter(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit()) {
        d
    } else if code == "Space" || is_fkey(code) || PUNCTUATION_CODES.contains(&code) {
        code
    } else {
        return None;
    };
    let fkey = is_fkey(key);
    if mods.is_empty() && !fkey {
        return None;
    }
    if mods == ["Shift"] && !fkey {
        return None;
    }
    mods.push(key);
    Some(mods.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(key: &str, code: &str, mods: &str) -> KeyEvent {
        KeyEvent { key: key.into(), code: code.into(), meta: mods.contains('m'), ctrl: mods.contains('c'), alt: mods.contains('a'), shift: mods.contains('s') }
    }

    #[test]
    fn labels() {
        assert_eq!(key_labels("mod+shift+k", true), ["⌘", "⇧", "K"]);
        assert_eq!(key_labels("mod+shift+k", false), ["Ctrl", "Shift", "K"]);
        assert_eq!(key_labels("alt+Up", true), ["⌥", "↑"]);
        assert_eq!(key_labels("escape", true), ["esc"]);
        assert_eq!(key_labels("delete", false), ["Del"]);
        assert_eq!(key_labels("F5", true), ["F5"]);
        assert_eq!(accelerator_labels("Control+Alt+Space", true), ["⌃", "⌥", "Space"]);
        assert_eq!(accelerator_labels("CommandOrControl+Shift+v", true), ["⌘", "⇧", "V"]);
        assert_eq!(accelerator_labels("Super+Semicolon", false), ["Win", "Semicolon"]);
        assert_eq!(accelerator_labels("Command+Alt+F12", false), ["Win", "Alt", "F12"]);
    }

    #[test]
    fn matching() {
        assert!(matches(&ev("k", "KeyK", "m"), "mod+k", true));
        assert!(!matches(&ev("k", "KeyK", "c"), "mod+k", true));
        assert!(matches(&ev("k", "KeyK", "c"), "mod+k", false));
        assert!(!matches(&ev("k", "KeyK", "cm"), "mod+k", false));
        assert!(!matches(&ev("k", "KeyK", "mc"), "mod+k", true)); // stray ctrl on a Mac
        assert!(matches(&ev("k", "KeyK", "mc"), "mod+ctrl+k", true));
        // Layout-independent letters and digits via `code`.
        assert!(matches(&ev("π", "KeyP", "ma"), "mod+alt+p", true));
        assert!(matches(&ev("!", "Digit1", "ms"), "mod+shift+1", true));
        // Shift is implied for symbols, refused for letters unless asked.
        assert!(matches(&ev("?", "Slash", "s"), "shift+?", true));
        assert!(matches(&ev("?", "Slash", "s"), "?", true));
        assert!(!matches(&ev("K", "KeyK", "ms"), "mod+k", true));
        assert!(matches(&ev("K", "KeyK", "ms"), "mod+shift+k", true));
        assert!(!matches(&ev("Enter", "Enter", "s"), "enter", true));
        assert!(matches(&ev("Escape", "Escape", "s"), "esc", true));
        assert!(matches(&ev("ArrowUp", "ArrowUp", "a"), "alt+up", true));
        assert!(!matches(&ev("ArrowUp", "ArrowUp", ""), "alt+up", true));
        assert!(matches(&ev(" ", "Space", "c"), "ctrl+space", true));
        assert!(matches(&ev("Delete", "Delete", ""), "del", true));
        assert!(matches(&ev("Backspace", "Backspace", ""), "backspace", true));
    }

    #[test]
    fn accelerators_from_events() {
        assert_eq!(accelerator_from_event(&ev(" ", "Space", "ca"), true).as_deref(), Some("Control+Alt+Space"));
        assert_eq!(accelerator_from_event(&ev("k", "KeyK", "m"), true).as_deref(), Some("Command+K"));
        assert_eq!(accelerator_from_event(&ev("k", "KeyK", "m"), false).as_deref(), Some("Super+K"));
        assert_eq!(accelerator_from_event(&ev("1", "Digit1", "cs"), true).as_deref(), Some("Control+Shift+1"));
        assert_eq!(accelerator_from_event(&ev("F5", "F5", ""), true).as_deref(), Some("F5"));
        assert_eq!(accelerator_from_event(&ev("F5", "F5", "s"), true).as_deref(), Some("Shift+F5"));
        assert_eq!(accelerator_from_event(&ev(";", "Semicolon", "a"), true).as_deref(), Some("Alt+Semicolon"));
        assert_eq!(accelerator_from_event(&ev("k", "KeyK", ""), true), None);
        assert_eq!(accelerator_from_event(&ev("K", "KeyK", "s"), true), None);
        assert_eq!(accelerator_from_event(&ev("Shift", "ShiftLeft", "s"), true), None);
        assert_eq!(accelerator_from_event(&ev("ArrowUp", "ArrowUp", "c"), true), None);
        assert_eq!(accelerator_from_event(&ev("F123", "F123", "c"), true), None);
    }
}
