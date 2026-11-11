//! Footer key hints that react to their key (`components/app/palette/key-hints.ts` and the
//! `key-hint` utility in `globals.css:550`): while a hint's key is held its keycaps sink to 90%
//! and turn `iris-soft` with iris ink, fast and eased out; on release they spring back on the pop
//! curve. The window's `on_key` names the hint with [`hint_for`] and keeps it in [`Held`]; Rune
//! reports key presses, not releases, so [`Held`] lets go after the key-repeat delay unless the
//! key repeats, and at the next press of another key.

use rune::{row, text, El, Key, KeyEvent};

use crate::theme::{motion, Theme};
use crate::widgets::shortcut::{cap, Shortcut, Size, Tone};

/// Which hint a key press belongs to (`hintFor`), matching each hint's `id`.
pub fn hint_for(e: &KeyEvent) -> Option<String> {
    let m = &e.mods;
    Some(match &e.key {
        Key::Enter if m.cmd => "mod+enter".into(),
        Key::Enter if m.shift => "shift+enter".into(),
        Key::Enter => "enter".into(),
        Key::Tab => "tab".into(),
        Key::Escape => "escape".into(),
        Key::Backspace | Key::Delete if m.cmd => "mod+backspace".into(),
        Key::Command(c) => format!("mod+{}", c.to_ascii_lowercase()),
        _ => return None,
    })
}

/// How long a hint stays held without a repeat: past macOS's default initial key-repeat delay.
pub const HOLD: f64 = 0.55;

/// The hint whose key is down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Held {
    hint: Option<(String, f64)>,
}

impl Held {
    /// A key press at `clock`; the deadline for the window's poll to call [`Held::poll`].
    pub fn press(&mut self, hint: Option<String>, clock: f64) -> Option<f64> {
        self.hint = hint.map(|h| (h, clock));
        self.hint.as_ref().map(|_| clock + HOLD)
    }
    /// True when the held hint let go (the view rebuilds).
    pub fn poll(&mut self, clock: f64) -> bool {
        if self
            .hint
            .as_ref()
            .is_some_and(|(_, at)| clock - at >= HOLD - 1e-6)
        {
            self.hint = None;
            return true;
        }
        false
    }
    pub fn release(&mut self) {
        self.hint = None;
    }
    pub fn is(&self, id: &str) -> bool {
        self.hint.as_ref().is_some_and(|(h, _)| h == id)
    }
}

/// One footer hint: its shortcut's caps, then its label (`text-[11.5px]` muted).
pub fn key_hint(theme: &Theme, id: &str, combo: &str, label: Option<&str>, pressed: bool) -> El {
    let caps = Shortcut::combo(combo);
    let spoken = caps.spoken_label();
    // Held, a cap turns `iris-soft` with iris ink and drops its depth: the `Accent` tone.
    let tone = if pressed { Tone::Accent } else { Tone::Default };
    let caps = row().key("caps").gap(2.0).a11y_hidden().children(
        kudzu_core::keys::key_labels(combo)
            .iter()
            .enumerate()
            .map(|(i, l)| {
                sink(cap(theme, l, tone, Size::Sm, None), pressed).key(format!("cap{i}"))
            }),
    );
    let mut hint = row()
        .key(format!("hint-{id}"))
        .role("text")
        .label(match label {
            Some(l) => format!("{spoken}: {l}"),
            None => spoken,
        })
        .gap(6.0)
        .center_y()
        .child(caps);
    if let Some(l) = label {
        hint = hint.child(
            text(l.to_owned())
                .font(theme.ty.meta_11_5)
                .color(theme.c.muted_foreground)
                .truncate()
                .a11y_hidden(),
        );
    }
    hint
}

/// The accent keycap of the palette's Enter hint (`size-[18px]` `bg-iris`, `shadow-accent`,
/// `sheen-strong`), sinking as the others do.
pub fn accent_cap(theme: &Theme, glyph: El, pressed: bool) -> El {
    let c = &theme.c;
    let [(_, top), (_, bottom)] = crate::theme::palette::fixed::SHEEN_STRONG;
    let e = theme.depth.accent.iter().fold(
        row()
            .size(18.0, 18.0)
            .center()
            .radius(crate::theme::radius::KEYCAP)
            .fill(c.iris)
            .gradient(top.color(), bottom.color())
            .child(glyph.color(c.iris_contrast)),
        |e, s| e.box_shadow(*s),
    );
    sink(e, pressed)
}

/// Down: 0.9 in `FAST`, eased out. Up: back on the pop curve over `SLOW`.
fn sink(e: El, pressed: bool) -> El {
    let (scale, curve) = if pressed {
        (motion::press::SCALE_KEY_HINT, motion::press::SINK)
    } else {
        (1.0, motion::press::RELEASE)
    };
    e.scale(scale).scale_motion(curve).origin(0.5, 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Mods;

    fn ev(key: Key, cmd: bool, shift: bool) -> KeyEvent {
        KeyEvent {
            key,
            mods: Mods {
                cmd,
                shift,
                ..Mods::default()
            },
        }
    }

    #[test]
    fn presses_map_to_their_hints() {
        assert_eq!(
            hint_for(&ev(Key::Enter, false, false)).as_deref(),
            Some("enter")
        );
        assert_eq!(
            hint_for(&ev(Key::Enter, true, false)).as_deref(),
            Some("mod+enter")
        );
        assert_eq!(
            hint_for(&ev(Key::Enter, false, true)).as_deref(),
            Some("shift+enter")
        );
        assert_eq!(
            hint_for(&ev(Key::Command('E'), true, false)).as_deref(),
            Some("mod+e")
        );
        assert_eq!(
            hint_for(&ev(Key::Backspace, true, false)).as_deref(),
            Some("mod+backspace")
        );
        assert_eq!(hint_for(&ev(Key::Backspace, false, false)), None);
    }

    #[test]
    fn a_held_hint_lets_go_after_the_repeat_delay() {
        let mut h = Held::default();
        assert_eq!(h.press(Some("tab".into()), 1.0), Some(1.0 + HOLD));
        assert!(h.is("tab"));
        assert!(!h.poll(1.2));
        assert!(h.poll(1.0 + HOLD));
        assert!(!h.is("tab"));
    }
}
