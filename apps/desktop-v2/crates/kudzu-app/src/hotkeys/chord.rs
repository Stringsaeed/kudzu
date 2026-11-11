//! Accelerator identity: Command/Cmd/Meta collapse to Super; modifier order is canonical.

use kudzu_core::keys::{function_key_number, recorder_chord_allowed};

const SUPER: &str = "Super";
const CONTROL: &str = "Control";
const ALT: &str = "Alt";
const SHIFT: &str = "Shift";

/// Comparable chord, or `None` when the recorder would refuse it.
pub fn identity(chord: &str) -> Option<String> {
    let mut tokens: Vec<&str> = chord
        .split('+')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect();
    let key = tokens.pop()?;
    if is_modifier(key) {
        return None;
    }
    let mut super_key = false;
    let mut control = false;
    let mut alt = false;
    let mut shift = false;
    for token in tokens {
        match token.to_ascii_lowercase().as_str() {
            "command" | "cmd" | "meta" | "super" | "commandorcontrol" | "cmdorctrl" => {
                super_key = true
            }
            "control" | "ctrl" => control = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            _ => return None,
        }
    }
    let fkey = function_key_number(key).is_some();
    if !recorder_chord_allowed(super_key || control || alt, shift, fkey) {
        return None;
    }
    let mut out = String::new();
    for (on, name) in [
        (super_key, SUPER),
        (control, CONTROL),
        (alt, ALT),
        (shift, SHIFT),
    ] {
        if on {
            if !out.is_empty() {
                out.push('+');
            }
            out.push_str(name);
        }
    }
    if !out.is_empty() {
        out.push('+');
    }
    out.push_str(&canonical_key(key));
    Some(out)
}

fn is_modifier(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "command"
            | "cmd"
            | "meta"
            | "super"
            | "commandorcontrol"
            | "cmdorctrl"
            | "control"
            | "ctrl"
            | "alt"
            | "option"
            | "shift"
    )
}

fn canonical_key(token: &str) -> String {
    if let Some(number) = function_key_number(token) {
        return format!("F{number}");
    }
    if token.eq_ignore_ascii_case("space") {
        return "Space".into();
    }
    if token.chars().count() == 1 {
        return token.to_ascii_uppercase();
    }
    token.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_meta_and_cmd_share_an_identity() {
        let expected = Some("Super+Space".into());
        assert_eq!(identity("Command+Space"), expected);
        assert_eq!(identity("Cmd+Space"), expected);
        assert_eq!(identity("Meta+Space"), expected);
        assert_eq!(identity("Super+Space"), expected);
    }

    #[test]
    fn modifier_aliases_and_order_do_not_create_a_second_shortcut() {
        assert_eq!(
            identity("Ctrl+Option+Shift+V"),
            identity("Shift+Alt+Control+V")
        );
        assert_eq!(identity("Control+Alt+Space"), identity("control+alt+space"));
        assert_eq!(identity("Control+Alt+V"), identity("control+alt+v"));
        assert_eq!(identity("F05"), identity("F5"));
        assert_eq!(
            identity("CommandOrControl+Space"),
            identity("CmdOrCtrl+Space")
        );
        assert_ne!(identity("Control+Alt+Space"), identity("Control+Alt+V"));
    }

    #[test]
    fn recorder_refusals_have_no_identity() {
        assert_eq!(identity(""), None);
        assert_eq!(identity("Control"), None);
        assert_eq!(identity("Control+Alt"), None);
        assert_eq!(identity("A"), None);
        assert_eq!(identity("Shift+A"), None);
        assert_eq!(identity("F5"), Some("F5".into()));
        assert_eq!(identity("Shift+F12"), Some("Shift+F12".into()));
        assert_eq!(identity("f8"), Some("F8".into()));
    }
}
