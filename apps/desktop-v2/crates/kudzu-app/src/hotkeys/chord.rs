//! Accelerator identity: Command/Cmd/Meta collapse to Super; modifier order is canonical.

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
            "command" | "cmd" | "meta" | "super" => super_key = true,
            "control" | "ctrl" => control = true,
            "alt" | "option" => alt = true,
            "shift" => shift = true,
            _ => return None,
        }
    }
    let fkey = is_f_key(key);
    if !(super_key || control || alt || shift || fkey) {
        return None;
    }
    if shift && !super_key && !control && !alt && !fkey {
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
        "command" | "cmd" | "meta" | "super" | "control" | "ctrl" | "alt" | "option" | "shift"
    )
}

fn is_f_key(token: &str) -> bool {
    let rest = match token.strip_prefix('F').or_else(|| token.strip_prefix('f')) {
        Some(rest) => rest,
        None => return false,
    };
    rest.parse::<u8>()
        .is_ok_and(|number| (1..=24).contains(&number))
}

fn canonical_key(token: &str) -> String {
    if is_f_key(token) {
        let digits = token.trim_start_matches(['F', 'f']);
        return format!("F{digits}");
    }
    if token.eq_ignore_ascii_case("space") {
        return "Space".into();
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
