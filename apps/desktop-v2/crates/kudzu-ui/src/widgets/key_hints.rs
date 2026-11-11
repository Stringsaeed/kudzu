//! Footer key hints that react to their key (`components/app/palette/key-hints.ts` and the
//! `key-hint` utility in `globals.css:550`): while a hint's key is held its keycaps sink to 90%
//! and turn `iris-soft` with iris ink, fast and eased out; on release they spring back on the pop
//! curve. The window's key handling names the hint with [`hint_for`] and keeps it in [`Held`];
//! the real key-up lets it go (D21): [`release_on_key_up`] puts an `on_key_up` on the window's
//! root, which hears every release no control took, as the Tauri window's `keyup` listener does.

use rune::{row, text, El, Key, KeyEvent, Tx};

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

/// The hint whose key is down.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Held {
    hint: Option<String>,
}

impl Held {
    /// A key press: the hint it belongs to, if any, is held; any other key lets go.
    pub fn press(&mut self, hint: Option<String>) {
        self.hint = hint;
    }
    /// A key-up (`releaseHints` lets every hint go on any release); true when one let go.
    pub fn release(&mut self) -> bool {
        self.hint.take().is_some()
    }
    pub fn is(&self, id: &str) -> bool {
        self.hint.as_deref() == Some(id)
    }
}

/// Puts the release on the window's `root`: every key-up nothing else took lets the held hint
/// go. The release is not taken, so other handlers still hear it.
pub fn release_on_key_up<S: 'static>(root: El, held: fn(&mut S) -> &mut Held) -> El {
    root.on_key_up(move |s: &mut S, _: &KeyEvent, tx: &mut Tx| {
        if !held(s).release() {
            tx.live_only();
        }
        false
    })
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

    struct Footer {
        held: Held,
    }

    fn footer(s: &Footer, _: &mut rune::Cx<Footer>) -> El {
        let theme = Theme::light();
        // The palette's field holds focus; here the root does, so its `on_key` hears the key.
        let root = rune::col()
            .key("footer")
            .focusable(true)
            .child(key_hint(
                theme,
                "escape",
                "escape",
                None,
                s.held.is("escape"),
            ))
            .on_key(|s: &mut Footer, e: &KeyEvent, _: &mut Tx| {
                s.held.press(hint_for(e));
                false
            });
        release_on_key_up(root, |s: &mut Footer| &mut s.held)
    }

    /// The hint holds from the key's press to its real release, however long it is held.
    #[test]
    fn a_held_hint_lets_go_on_key_up() {
        let mut rt = rune::Runtime::new(
            Footer {
                held: Held::default(),
            },
            footer,
            (400.0, 60.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.update(|_, tx| tx.focus("footer", false));
        rt.frame(1.0 / 60.0);
        rt.key(ev(Key::Escape, false, false));
        rt.frame(1.0 / 60.0);
        assert!(rt.state.held.is("escape"));
        for _ in 0..120 {
            rt.frame(1.0 / 60.0);
        }
        assert!(rt.state.held.is("escape"), "still held after two seconds");
        rt.key_up(ev(Key::Escape, false, false));
        rt.frame(1.0 / 60.0);
        assert!(!rt.state.held.is("escape"));
    }
}
