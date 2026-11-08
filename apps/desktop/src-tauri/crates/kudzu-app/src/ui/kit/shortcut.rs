//! `components/app/shortcut.tsx` and the label half of `lib/keys.ts`: a keyboard shortcut drawn
//! as Kbd keycaps, from a combo (`"mod+shift+k"`) or ready-made labels (a recorded accelerator).
//!
//! Hidden from assistive technology, as on the web: controls announce their shortcut through
//! `shortcut_hint`. A shortcut inside a sentence sets [`Shortcut::spoken`], which names it
//! ("Command Return").

// The kit's whole API is ported now; screens adopt it as they port.
#![allow(dead_code)]

use rune::{Color, El, TextStyle};

use crate::ui::theme::Theme;

use super::kbd::{self, Kbd, KbdContext};

/// `lib/keys.ts` `SYMBOLS` on macOS.
fn symbol(key: &str) -> Option<&'static str> {
    Some(match key {
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

/// `keyLabels`: `"mod+shift+k"` → `["⌘", "⇧", "K"]`.
pub fn key_labels(combo: &str) -> Vec<String> {
    combo
        .split('+')
        .map(|k| match symbol(&k.to_lowercase()) {
            Some(s) => s.to_string(),
            None if k.chars().count() == 1 => k.to_uppercase(),
            None => k.to_string(),
        })
        .collect()
}

/// `acceleratorLabels`: `"Control+Alt+Space"` → `["⌃", "⌥", "Space"]`.
pub fn accelerator_labels(accel: &str) -> Vec<String> {
    accel
        .split('+')
        .map(|p| {
            let k = p.to_lowercase();
            match k.as_str() {
                "commandorcontrol" | "cmdorctrl" | "super" | "command" | "cmd" | "meta" => "⌘".to_string(),
                _ => match symbol(&k) {
                    Some(s) => s.to_string(),
                    None if p.chars().count() == 1 => p.to_uppercase(),
                    None => p.to_string(),
                },
            }
        })
        .collect()
}

/// `spokenShortcut`: `"mod+enter"` → `"Command Return"`.
pub fn spoken(combo: &str) -> String {
    combo
        .split('+')
        .map(|k| {
            let w = match k.to_lowercase().as_str() {
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
                _ => return k.to_uppercase(),
            };
            w.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `TONES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Tone {
    /// A keycap on a regular surface.
    #[default]
    Default,
    /// A hint that recedes (revealed on hover): `bg-transparent text-faint shadow-none`.
    Ghost,
    /// On an accent fill that isn't a Button: `bg-black/15 text-current` with an inset shadow.
    OnAccent,
    /// Highlighted (the global quick-search shortcut): `bg-iris-soft text-iris-foreground`.
    Accent,
    /// Inside a tooltip (Kbd's own contextual look).
    Tooltip,
}

/// `size`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShortcutSize {
    /// `h-[18px] min-w-[18px] text-[10.5px]`.
    #[default]
    Sm,
    /// Kbd's own 20 pt keycaps.
    Md,
}

/// `<Shortcut combo="mod+k" />`.
pub struct Shortcut {
    labels: Vec<String>,
    combo: Option<String>,
    tone: Tone,
    size: ShortcutSize,
    spoken: bool,
    current: Option<Color>,
}

impl Shortcut {
    /// From a combo in the shared grammar (`"mod+shift+k"`).
    pub fn new(combo: impl Into<String>) -> Shortcut {
        let combo = combo.into();
        Shortcut { labels: key_labels(&combo), combo: Some(combo), tone: Tone::Default, size: ShortcutSize::Sm, spoken: false, current: None }
    }
    /// From ready-made labels (`keys`), e.g. [`accelerator_labels`].
    pub fn keys(labels: impl IntoIterator<Item = impl Into<String>>) -> Shortcut {
        Shortcut { labels: labels.into_iter().map(Into::into).collect(), combo: None, tone: Tone::Default, size: ShortcutSize::Sm, spoken: false, current: None }
    }
    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = t;
        self
    }
    pub fn size(mut self, s: ShortcutSize) -> Self {
        self.size = s;
        self
    }
    /// Inside a sentence: assistive technology still hears it ("Command Return").
    pub fn spoken(mut self) -> Self {
        self.spoken = true;
        self
    }
    /// `currentColor`, for the OnAccent tone's `text-current`.
    pub fn color(mut self, c: Color) -> Self {
        self.current = Some(c);
        self
    }

    pub fn build(self, th: &Theme) -> El {
        let t = &th.t;
        let caps = self.labels.into_iter().map(|k| {
            let mut cap = Kbd::new(k);
            if self.size == ShortcutSize::Sm {
                cap = cap.height(18.0).font(TextStyle { size: 10.5, ..kbd::FONT });
            }
            match self.tone {
                Tone::Default => {}
                Tone::Ghost => cap = cap.fill(Color::TRANSPARENT).color(t.faint).shadows(Vec::new()),
                Tone::OnAccent => cap = cap.context(KbdContext::FilledButton).current(self.current.unwrap_or(t.primary_foreground)),
                Tone::Accent => cap = cap.fill(t.iris_soft).color(t.iris_foreground).shadows(Vec::new()),
                Tone::Tooltip => cap = cap.context(KbdContext::Tooltip),
            }
            cap.build(th)
        });
        let e = kbd::group(caps, 2.0);
        match self.combo.filter(|_| self.spoken) {
            Some(c) => rune::row().center_y().label(spoken(&c)).child(e),
            None => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_match_keys_ts() {
        assert_eq!(key_labels("mod+shift+k"), ["⌘", "⇧", "K"]);
        assert_eq!(key_labels("enter"), ["↵"]);
        assert_eq!(key_labels("escape"), ["esc"]);
        assert_eq!(accelerator_labels("Control+Alt+Space"), ["⌃", "⌥", "Space"]);
        assert_eq!(accelerator_labels("CommandOrControl+Shift+V"), ["⌘", "⇧", "V"]);
        assert_eq!(spoken("mod+enter"), "Command Return");
    }
}
