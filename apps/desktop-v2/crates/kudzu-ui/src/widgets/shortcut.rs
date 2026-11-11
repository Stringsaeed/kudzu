//! RUNE-GAP(#573: rune-kit `Keybind` takes its cap height, padding, radius and type from `Kit`):
//! once it does, build the caps with `rune_kit::Keybind` and keep only the tones and the
//! spoken label here.
//!
//! A keyboard shortcut as keycaps (`components/app/shortcut.tsx` on shadcn's `Kbd`). rune-kit's
//! `Keybind` draws 14 point caps with 2 point corners; Kudzu's are `h-5 min-w-5 px-1`, 5 point
//! corners, 11 point medium tabular figures on `foreground/6%` with the `key` depth, and 18 /
//! 10.5 in the `sm` size most of the app uses. The caps are hidden from assistive technology:
//! a control announces its shortcut itself (`El::shortcut_hint`), and a shortcut inside a
//! sentence is [`Shortcut::spoken`], which names the combo in words ("Command Return").

use rune::{row, text, Color, El, TextStyle};

use crate::theme::{radius, Theme};

/// `TONES` in `shortcut.tsx`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// A keycap on a regular surface.
    #[default]
    Default,
    /// A hint that recedes: no fill, faint ink, no depth.
    Ghost,
    /// On an accent fill: `black/15` with an inset shadow, the fill's own ink.
    OnAccent,
    /// Highlighted (the quick-search shortcut): `iris-soft`, iris ink.
    Accent,
}

/// `size`: `sm` (18 tall, 10.5 point) is the default, `md` is shadcn's 20.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Size {
    #[default]
    Sm,
    Md,
}

impl Size {
    /// Cap height (and minimum width) and type size.
    pub fn metrics(self) -> (f32, f32) {
        match self {
            Size::Sm => (18.0, 10.5),
            Size::Md => (20.0, 11.0),
        }
    }
}

/// `Shortcut::combo("mod+shift+k").build(theme)`, or `Shortcut::keys(["⌃", "Space"])` for
/// labels made elsewhere (a recorded accelerator).
#[derive(Clone, Debug, PartialEq)]
pub struct Shortcut {
    labels: Vec<String>,
    combo: Option<String>,
    tone: Tone,
    size: Size,
    spoken: bool,
    ink: Option<Color>,
}

impl Shortcut {
    pub fn combo(combo: &str) -> Shortcut {
        Shortcut {
            labels: kudzu_core::keys::key_labels(combo),
            combo: Some(combo.to_owned()),
            ..Shortcut::keys(Vec::<String>::new())
        }
    }
    /// A recorded accelerator (`"Control+Alt+Space"`), as Settings stores the hotkeys.
    pub fn accelerator(accel: &str) -> Shortcut {
        Shortcut::keys(kudzu_core::keys::accelerator_labels(accel))
    }
    /// Labels made elsewhere; they are spoken by name (`⌃` is "Control").
    pub fn keys<I: IntoIterator<Item = T>, T: Into<String>>(keys: I) -> Shortcut {
        Shortcut {
            labels: keys.into_iter().map(Into::into).collect(),
            combo: None,
            tone: Tone::Default,
            size: Size::Sm,
            spoken: false,
            ink: None,
        }
    }
    pub fn tone(mut self, t: Tone) -> Self {
        self.tone = t;
        self
    }
    pub fn size(mut self, s: Size) -> Self {
        self.size = s;
        self
    }
    /// The shortcut sits inside a sentence: readers hear it in words.
    pub fn spoken(mut self, on: bool) -> Self {
        self.spoken = on;
        self
    }
    /// The ink of an `OnAccent` cap (`text-current`): the accent fill's foreground.
    pub fn ink(mut self, c: Color) -> Self {
        self.ink = Some(c);
        self
    }

    /// What a reader hears: the combo in words; labels by their keys' spoken names.
    pub fn spoken_label(&self) -> String {
        match &self.combo {
            Some(c) => kudzu_core::keys::spoken_shortcut(c),
            None => self
                .labels
                .iter()
                .map(|l| spoken_cap(l))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }

    pub fn build(self, theme: &Theme) -> El {
        let caps =
            row()
                .gap(2.0)
                .center_y()
                .a11y_hidden()
                .children(self.labels.iter().enumerate().map(|(i, l)| {
                    cap(theme, l, self.tone, self.size, self.ink).key(format!("cap{i}"))
                }));
        if !self.spoken {
            return caps.role("none");
        }
        row()
            .role("text")
            .label(self.spoken_label())
            .center_y()
            .child(caps)
    }
}

/// A keycap's spoken name: its glyph turned back into the combo token `kudzu_core::keys` names
/// (`⌃` → `ctrl` → "Control"), a letter as itself.
pub fn spoken_cap(label: &str) -> String {
    let token = match label {
        "⌘" => "mod",
        "⌃" => "ctrl",
        "⌥" => "alt",
        "⇧" => "shift",
        "↵" | "⏎" | "↩" => "enter",
        "⌫" => "backspace",
        "⌦" => return "Forward Delete".into(),
        "⇥" => "tab",
        "esc" => "escape",
        "↑" => "up",
        "↓" => "down",
        "←" => "left",
        "→" => "right",
        "Space" | "space" => "space",
        other => other,
    };
    kudzu_core::keys::spoken_shortcut(token)
}

/// One keycap: also the footer hints' caps and the preview's key badges.
pub fn cap(theme: &Theme, label: &str, tone: Tone, size: Size, ink: Option<Color>) -> El {
    let c = &theme.c;
    let (h, font) = size.metrics();
    let style = TextStyle {
        size: font,
        line_height: font,
        ..theme.ty.keycap
    };
    let (fill, ink, depth): (Color, Color, &[rune::Shadow]) = match tone {
        Tone::Default => (
            c.foreground.alpha(0.06),
            c.muted_foreground,
            &theme.depth.key,
        ),
        Tone::Ghost => (Color::TRANSPARENT, c.faint, &[]),
        Tone::OnAccent => (
            Color::BLACK.alpha(0.15),
            ink.unwrap_or(c.primary_foreground),
            &ON_ACCENT,
        ),
        Tone::Accent => (c.iris_soft, c.iris_foreground, &[]),
    };
    depth.iter().fold(
        row()
            .h(h)
            .min_w(h)
            .px(4.0)
            .center()
            .radius(radius::KEYCAP)
            .fill(fill)
            .child(text(label.to_owned()).font(style).color(ink)),
        |e, s| e.box_shadow(*s),
    )
}

/// `shadow-[inset_0_1px_1px_oklch(0_0_0/0.15)]`.
const ON_ACCENT: [rune::Shadow; 1] = [rune::Shadow {
    x: 0.0,
    y: 1.0,
    blur: 1.0,
    spread: 0.0,
    color: Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.15,
    },
    inset: true,
}];

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Cx, Env, Runtime};

    #[test]
    fn combos_become_mac_caps_and_spoken_words() {
        let s = Shortcut::combo("mod+shift+k");
        assert_eq!(s.labels, vec!["⌘", "⇧", "K"]);
        assert_eq!(s.spoken_label(), "Command Shift K");
        assert_eq!(
            Shortcut::combo("mod+enter").spoken_label(),
            "Command Return"
        );
        assert_eq!(
            Shortcut::keys(["⌃", "⌥", "Space"]).spoken_label(),
            "Control Option Space"
        );
        assert_eq!(
            Shortcut::accelerator("CommandOrControl+Shift+V").spoken_label(),
            "Command Shift V"
        );
        assert_eq!(
            Shortcut::keys(["⌦", "k"]).spoken_label(),
            "Forward Delete K"
        );
    }

    /// The accessibility tree as JSON text.
    fn tree(spoken: bool) -> String {
        let view = if spoken {
            |_: &(), _: &mut Cx<()>| {
                Shortcut::combo("mod+enter")
                    .spoken(true)
                    .build(Theme::light())
            }
        } else {
            |_: &(), _: &mut Cx<()>| Shortcut::combo("mod+enter").build(Theme::light())
        };
        let mut rt = Runtime::new((), view, (200.0, 60.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt.accessibility_json().to_string()
    }

    /// The tree speaks a shortcut in words when it sits in a sentence, and never reads its caps.
    #[test]
    fn spoken_shortcuts_are_named_in_words_in_the_tree() {
        let spoken = tree(true);
        assert!(spoken.contains("Command Return"), "{spoken}");
        for t in [spoken, tree(false)] {
            assert!(!t.contains('⌘') && !t.contains('↵'), "caps are hidden: {t}");
        }
    }

    /// Recorded accelerators (Settings' hotkeys) read by name, never by glyph.
    #[test]
    fn recorded_accelerators_are_spoken_by_name_in_the_tree() {
        fn view(_: &(), _: &mut Cx<()>) -> El {
            rune::row()
                .child(
                    Shortcut::accelerator("Control+Alt+Space")
                        .spoken(true)
                        .build(Theme::light()),
                )
                .child(
                    Shortcut::keys(["⌃", "⌥", "V"])
                        .spoken(true)
                        .build(Theme::light()),
                )
        }
        let mut rt = Runtime::new((), view, (300.0, 60.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let t = rt.accessibility_json().to_string();
        assert!(t.contains("Control Option Space"), "{t}");
        assert!(t.contains("Control Option V"), "{t}");
        assert!(!t.contains('⌃') && !t.contains('⌥'), "{t}");
    }

    #[test]
    fn caps_take_the_kbd_metrics() {
        assert_eq!(Size::Md.metrics(), (20.0, 11.0));
        assert_eq!(Size::Sm.metrics(), (18.0, 10.5));
    }
}
