//! Keyboard shortcut grammar and labels.
//!
//! Mac symbols, spoken names, and the recorder's physical-code rules from `lib/keys.ts`.
//! Kudzu is macOS-only, so `mod` is always Command.

/// A keydown as the shortcut recorder sees it: physical `code` plus modifier flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: String,
    pub code: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Splits a shortcut into display keys: `"mod+shift+k"` → `["⌘", "⇧", "K"]`.
pub fn key_labels(combo: &str) -> Vec<String> {
    combo
        .split('+')
        .map(|k| {
            symbol(&k.to_ascii_lowercase())
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if k.chars().count() == 1 {
                        k.to_ascii_uppercase()
                    } else {
                        k.to_string()
                    }
                })
        })
        .collect()
}

/// Accelerator string (`"Control+Alt+Space"`) → display keys.
pub fn accelerator_labels(accel: &str) -> Vec<String> {
    accel
        .split('+')
        .map(|p| {
            let k = p.to_ascii_lowercase();
            if matches!(
                k.as_str(),
                "commandorcontrol" | "cmdorctrl" | "super" | "command" | "cmd" | "meta"
            ) {
                "⌘".to_string()
            } else {
                symbol(&k).map(str::to_string).unwrap_or_else(|| {
                    if p.chars().count() == 1 {
                        p.to_ascii_uppercase()
                    } else {
                        p.to_string()
                    }
                })
            }
        })
        .collect()
}

/// Words for a combo, e.g. `"mod+enter"` → `"Command Return"`.
pub fn spoken_shortcut(combo: &str) -> String {
    combo
        .split('+')
        .map(|k| {
            spoken_word(&k.to_ascii_lowercase())
                .map(str::to_string)
                .unwrap_or_else(|| k.to_ascii_uppercase())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a keydown matches a combo such as `"mod+k"`, `"shift+?"`, `"alt+up"`.
pub fn matches(event: &KeyEvent, combo: &str) -> bool {
    let parts: Vec<String> = combo.split('+').map(|p| p.to_ascii_lowercase()).collect();
    let Some((key, mods)) = parts.split_last() else {
        return false;
    };
    let want: Vec<&str> = mods.iter().map(String::as_str).collect();
    let want_mod = want.contains(&"mod");
    if want_mod != event.meta {
        return false;
    }
    if !want.contains(&"ctrl") && event.ctrl {
        return false;
    }
    if want.contains(&"ctrl") && !event.ctrl {
        return false;
    }
    if want.contains(&"alt") != event.alt {
        return false;
    }
    if want.contains(&"shift") && !event.shift {
        return false;
    }
    let key_ref: &str = key;
    if !want.contains(&"shift") && event.shift && is_shift_sensitive(key_ref) {
        return false;
    }
    let k = event.key.to_ascii_lowercase();
    if let Some(named) = named_key(key_ref) {
        return k == named;
    }
    if key_ref.len() == 1 && is_alnum_byte(key_ref.as_bytes()[0]) {
        let code = event.code.to_ascii_lowercase();
        let expected_code = if key_ref.as_bytes()[0].is_ascii_digit() {
            format!("digit{key_ref}")
        } else {
            format!("key{key_ref}")
        };
        return k == *key_ref || code == expected_code;
    }
    k == *key_ref
}

/// Builds a Tauri accelerator from a keydown, or [`None`] if it's only modifiers / has none.
///
/// Accepts a modifier plus a key, or a bare F-key. Lone modifiers and Shift-only non-F
/// chords are refused. Letters, digits and punctuation come from the physical `code`.
pub fn accelerator_from_event(event: &KeyEvent) -> Option<String> {
    let mut mods = Vec::new();
    if event.ctrl {
        mods.push("Control");
    }
    if event.alt {
        mods.push("Alt");
    }
    if event.shift {
        mods.push("Shift");
    }
    if event.meta {
        mods.push("Command");
    }
    let code = event.code.as_str();
    let key = if is_letter_code(code) {
        Some(code[3..].to_string())
    } else if is_digit_code(code) {
        Some(code[5..].to_string())
    } else if code == "Space" {
        Some("Space".into())
    } else if is_f_key(code) || is_named_code(code) {
        Some(code.to_string())
    } else {
        None
    };
    let key = key?;
    let fkey = is_f_key(&key);
    if mods.is_empty() && !fkey {
        return None;
    }
    if mods.len() == 1 && mods[0] == "Shift" && !fkey {
        return None;
    }
    mods.push(key.as_str());
    Some(mods.join("+"))
}

fn symbol(k: &str) -> Option<&'static str> {
    Some(match k {
        "mod" | "cmd" => "⌘",
        "ctrl" | "control" => "⌃",
        "alt" | "option" => "⌥",
        "shift" => "⇧",
        "enter" => "↵",
        "backspace" | "delete" => "⌫",
        "escape" => "esc",
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "tab" => "⇥",
        "space" => "Space",
        _ => return None,
    })
}

fn spoken_word(k: &str) -> Option<&'static str> {
    Some(match k {
        "mod" => "Command",
        "ctrl" => "Control",
        "shift" => "Shift",
        "alt" => "Option",
        "enter" => "Return",
        "escape" | "esc" => "Escape",
        "tab" => "Tab",
        "space" => "Space",
        "backspace" => "Delete",
        "up" => "Up Arrow",
        "down" => "Down Arrow",
        "left" => "Left Arrow",
        "right" => "Right Arrow",
        _ => return None,
    })
}

fn named_key(key: &str) -> Option<&'static str> {
    Some(match key {
        "up" => "arrowup",
        "down" => "arrowdown",
        "left" => "arrowleft",
        "right" => "arrowright",
        "esc" => "escape",
        "space" => " ",
        "del" => "delete",
        _ => return None,
    })
}

fn is_shift_sensitive(key: &str) -> bool {
    if key.len() == 1 {
        let b = key.as_bytes()[0];
        return b.is_ascii_lowercase() || b.is_ascii_digit();
    }
    matches!(
        key,
        "enter" | "backspace" | "delete" | "up" | "down" | "tab"
    )
}

fn is_alnum_byte(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit()
}

fn is_letter_code(code: &str) -> bool {
    code.len() == 4 && code.starts_with("Key") && code.as_bytes()[3].is_ascii_uppercase()
}

fn is_digit_code(code: &str) -> bool {
    code.len() == 6 && code.starts_with("Digit") && code.as_bytes()[5].is_ascii_digit()
}

fn is_f_key(code: &str) -> bool {
    let Some(rest) = code.strip_prefix('F') else {
        return false;
    };
    (1..=2).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_digit())
}

fn is_named_code(code: &str) -> bool {
    matches!(
        code,
        "Semicolon"
            | "Quote"
            | "Comma"
            | "Period"
            | "Slash"
            | "Backslash"
            | "BracketLeft"
            | "BracketRight"
            | "Backquote"
            | "Minus"
            | "Equal"
            | "Enter"
            | "Tab"
    )
}
