//! Kudzu's design system, hand-authored once from the Tauri app's `app/globals.css`.
//!
//! `palette`, `depth`, `radius`, `type_`, `motion` and `fonts` hold the tokens; [`Theme`] is one
//! resolved appearance. `icons` (S05) and `kit` (S06, which adds `Theme::kit`) build on them.
//! The table tests compare every token with a verbatim excerpt of the CSS (`css` below).

pub mod depth;
pub mod fonts;
mod gamut;
pub mod icons;
pub mod kit;
pub mod motion;
pub mod palette;
pub mod radius;
pub mod type_;
pub mod window;

use std::sync::LazyLock;

/// The Appearance setting: Dark (the Tauri app's default), Light, or follow the system.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
    System,
}

impl Appearance {
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Appearance::Dark => true,
            Appearance::Light => false,
            Appearance::System => system_dark,
        }
    }
}

/// One appearance, resolved: colours, depth and type. Motion, radius and fonts do not change
/// with the theme and are read from their modules.
#[derive(Debug)]
pub struct Theme {
    pub dark: bool,
    /// The CSS values behind `c`, for code that needs to mix in OKLCH or read alphas.
    pub tokens: &'static palette::Tokens,
    pub c: palette::Palette,
    pub depth: depth::Depth,
    pub ty: type_::Type,
}

static LIGHT: LazyLock<Theme> = LazyLock::new(|| Theme::build(false));
static DARK: LazyLock<Theme> = LazyLock::new(|| Theme::build(true));

impl Theme {
    fn build(dark: bool) -> Theme {
        let (tokens, depth) = if dark {
            (&palette::DARK, &depth::DARK)
        } else {
            (&palette::LIGHT, &depth::LIGHT)
        };
        Theme {
            dark,
            tokens,
            c: tokens.resolve(),
            depth: depth.resolve(),
            ty: type_::TYPE,
        }
    }

    pub fn light() -> &'static Theme {
        &LIGHT
    }

    pub fn dark() -> &'static Theme {
        &DARK
    }

    /// The theme for `preference` when the system is dark or not (`Cx::system_dark()`).
    pub fn resolve(preference: Appearance, system_dark: bool) -> &'static Theme {
        if preference.is_dark(system_dark) {
            Theme::dark()
        } else {
            Theme::light()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preference_picks_the_theme() {
        assert!(Theme::resolve(Appearance::Dark, false).dark);
        assert!(!Theme::resolve(Appearance::Light, true).dark);
        assert!(Theme::resolve(Appearance::System, true).dark);
        assert!(!Theme::resolve(Appearance::System, false).dark);
        assert_eq!(Appearance::default(), Appearance::Dark);
    }

    #[test]
    fn themes_resolve_their_own_tables() {
        assert_eq!(Theme::dark().c, palette::DARK.resolve());
        assert_eq!(Theme::light().depth, depth::LIGHT.resolve());
        assert_eq!(Theme::light().c.background, rune::Color::WHITE);
    }
}

/// Test-only: a verbatim excerpt of the Tauri app's `app/globals.css` and a reader for the few
/// CSS forms it uses. The token tables are checked against it, so a transcription slip fails a
/// test. Lines 74–80 (radius scale, inside `@theme inline`) and 156–319 (`:root`, `.vibrant`,
/// `.dark`), unchanged.
#[cfg(test)]
pub(crate) mod css {
    use std::collections::HashMap;

    use super::depth::Layer;
    use super::palette::Oklch;

    // Source: bca79f179f5b6e7c869161d80ce6d3ebb1106689.
    // Repository path components: apps / desktop / app / globals.css (full path in docs/migration/reviews/S04-theme-fixes.md).
    // Frozen excerpt for the S73 parity re-check; no build-time dependency on the old app.
    pub const GLOBALS: &str = r#"
@theme inline {
  --radius-sm: calc(var(--radius) * 0.6);
  --radius-md: calc(var(--radius) * 0.8);
  --radius-lg: var(--radius);
  --radius-xl: calc(var(--radius) * 1.4);
  --radius-2xl: calc(var(--radius) * 1.8);
  --radius-3xl: calc(var(--radius) * 2.2);
  --radius-4xl: calc(var(--radius) * 2.6);
}

:root {
  --radius: 0.625rem;

  /*
   * Motion. Three speeds: the bigger the thing that moves, the slower it goes. Two damped-spring
   * curves: soft (1.5% overshoot) for things that travel, pop (10%) for things that land.
   * Reduced motion is handled once, in the base layer below.
   */
  --motion-fast: 80ms;
  --motion-moderate: 160ms;
  --motion-slow: 240ms;
  --spring-soft: linear(0, 0.053, 0.175, 0.324, 0.473, 0.609, 0.723, 0.814, 0.883, 0.934, 0.968, 0.991, 1.005, 1.012, 1.015, 1.015, 1);
  --spring-pop: linear(0, 0.067, 0.225, 0.422, 0.618, 0.789, 0.923, 1.018, 1.075, 1.102, 1.106, 1.096, 1.077, 1.056, 1.035, 1.018, 1.005, 0.996, 0.991, 0.989, 1);

  /*
   * SF Pro on macOS (-apple-system / BlinkMacSystemFont), Figtree everywhere else. `system-ui` is
   * deliberately absent: on Windows/Linux it would resolve to Segoe/Cantarell before Figtree.
   */
  --app-sans: -apple-system, BlinkMacSystemFont, "Figtree Variable", sans-serif;
  --app-mono: "Paper Mono", ui-monospace, "SF Mono", Menlo, monospace;
  --app-features: normal;

  /* Palette: shadcn theme (stone neutrals, blue primary). Kudzu-only tokens are derived below it. */
  --background: oklch(1 0 0);
  --foreground: oklch(0.147 0.004 49.25);
  --card: oklch(1 0 0);
  --card-foreground: oklch(0.147 0.004 49.25);
  --popover: oklch(1 0 0);
  --popover-foreground: oklch(0.147 0.004 49.25);
  --primary: oklch(0.488 0.243 264.376);
  --primary-foreground: oklch(0.97 0.014 254.604);
  --secondary: oklch(0.967 0.001 286.375);
  --secondary-foreground: oklch(0.21 0.006 285.885);
  --muted: oklch(0.97 0.001 106.424);
  --muted-foreground: oklch(0.553 0.013 58.071);
  --accent: oklch(0.97 0.001 106.424);
  --accent-foreground: oklch(0.216 0.006 56.043);
  --destructive: oklch(0.577 0.245 27.325);
  --border: oklch(0.923 0.003 48.717);
  --input: oklch(0.923 0.003 48.717);
  --ring: oklch(0.709 0.01 56.259);
  --chart-1: oklch(0.809 0.105 251.813);
  --chart-2: oklch(0.623 0.214 259.815);
  --chart-3: oklch(0.546 0.245 262.881);
  --chart-4: oklch(0.488 0.243 264.376);
  --chart-5: oklch(0.424 0.199 265.638);
  --sidebar: oklch(0.985 0.001 106.423);
  --sidebar-foreground: oklch(0.147 0.004 49.25);
  --sidebar-primary: oklch(0.546 0.245 262.881);
  --sidebar-primary-foreground: oklch(0.97 0.014 254.604);
  --sidebar-accent: oklch(0.97 0.001 106.424);
  --sidebar-accent-foreground: oklch(0.216 0.006 56.043);
  --sidebar-border: oklch(0.923 0.003 48.717);
  --sidebar-ring: oklch(0.709 0.01 56.259);

  /* Kudzu extensions, derived from the palette above. */
  --surface: var(--card);
  --surface-raised: oklch(1 0 0);
  --hairline: oklch(0.147 0.004 49.25 / 0.08);
  --faint: oklch(0.65 0.01 56);
  /* "iris" is the app's single accent (focus, selection, tokens); it follows primary. */
  --iris: var(--primary);
  --iris-soft: oklch(0.488 0.243 264.376 / 0.1);
  --iris-foreground: oklch(0.488 0.243 264.376);
  --iris-contrast: var(--primary-foreground);
  /* Elevation: light surfaces need far less shadow than dark ones to read as raised. */
  --elev-card: 0 0 0 1px oklch(0.147 0.004 49.25 / 0.07), 0 1px 2px oklch(0.147 0.01 50 / 0.05), 0 6px 16px -10px oklch(0.147 0.02 50 / 0.12);
  --elev-raised: inset 0 1px 0 oklch(1 0 0 / 0.7), 0 0 0 1px oklch(0.147 0.004 49.25 / 0.1), 0 1px 2px oklch(0.147 0.01 50 / 0.08);
  --elev-raised-hover: inset 0 1px 0 oklch(1 0 0 / 0.7), 0 0 0 1px oklch(0.147 0.004 49.25 / 0.13), 0 2px 5px -1px oklch(0.147 0.01 50 / 0.12);
  --elev-pressed: inset 0 1px 2px oklch(0.147 0.01 50 / 0.12), 0 0 0 1px oklch(0.147 0.004 49.25 / 0.1);
  --elev-well: inset 0 1px 2px oklch(0.147 0.01 50 / 0.06), inset 0 0 0 1px oklch(0.147 0.004 49.25 / 0.09);
  --elev-knob: 0 0 0 0.5px oklch(0.147 0.01 50 / 0.12), 0 1px 3px oklch(0.147 0.01 50 / 0.2);
  --elev-key: inset 0 -1px 0 oklch(0.147 0.004 49.25 / 0.16), 0 0 0 0.5px oklch(0.147 0.004 49.25 / 0.08);
  --elev-selected: 0 0 0 1px oklch(0.147 0.004 49.25 / 0.05), 0 1px 2px oklch(0.147 0.01 50 / 0.06);
  --elev-accent: inset 0 1px 0 oklch(1 0 0 / 0.22), 0 0 0 1px oklch(0.424 0.199 265.638 / 0.6), 0 1px 2px oklch(0.424 0.199 265.638 / 0.25), 0 4px 12px -4px oklch(0.488 0.243 264.376 / 0.45);
  --elev-soft: inset 0 1px 0 oklch(1 0 0 / 0.7), 0 0 0 1px oklch(0.147 0.004 49.25 / 0.06), 0 1px 2px oklch(0.147 0.01 50 / 0.07), 0 2px 8px -3px oklch(0.147 0.01 50 / 0.1);
  --elev-float: 0 0 0 1px var(--hairline), 0 12px 32px -10px oklch(0.147 0.02 50 / 0.18), 0 2px 6px oklch(0.147 0.01 50 / 0.06);
  --elev-panel: 0 0 0 1px var(--hairline), 0 24px 60px -14px oklch(0.147 0.02 50 / 0.28), 0 4px 14px oklch(0.147 0.01 50 / 0.08);
  /* Template chips */
  --chip-time: oklch(0.47 0.13 235);
  --chip-clipboard: oklch(0.47 0.12 158);
  --chip-cursor: var(--iris-foreground);
  --chip-form: oklch(0.52 0.13 62);
  --chip-error: oklch(0.52 0.19 24);
  --chip-neutral: var(--foreground);
  --success: oklch(0.6 0.15 155);
  --warning: oklch(0.68 0.15 70);
  --favorite: oklch(0.72 0.16 65);
}

/* macOS: let NSVisualEffectView show through the sidebar (same colours, translucent). */
.vibrant {
  --sidebar: oklch(0.985 0.001 106.423 / 0.72);
}
.vibrant.dark {
  --sidebar: oklch(0.216 0.006 56.043 / 0.55);
}

.dark {
  --background: oklch(0.147 0.004 49.25);
  --foreground: oklch(0.985 0.001 106.423);
  --card: oklch(0.216 0.006 56.043);
  --card-foreground: oklch(0.985 0.001 106.423);
  --popover: oklch(0.216 0.006 56.043);
  --popover-foreground: oklch(0.985 0.001 106.423);
  --primary: oklch(0.424 0.199 265.638);
  --primary-foreground: oklch(0.97 0.014 254.604);
  --secondary: oklch(0.274 0.006 286.033);
  --secondary-foreground: oklch(0.985 0 0);
  --muted: oklch(0.268 0.007 34.298);
  --muted-foreground: oklch(0.709 0.01 56.259);
  --accent: oklch(0.268 0.007 34.298);
  --accent-foreground: oklch(0.985 0.001 106.423);
  --destructive: oklch(0.704 0.191 22.216);
  --border: oklch(1 0 0 / 10%);
  --input: oklch(1 0 0 / 15%);
  --ring: oklch(0.553 0.013 58.071);
  --chart-1: oklch(0.809 0.105 251.813);
  --chart-2: oklch(0.623 0.214 259.815);
  --chart-3: oklch(0.546 0.245 262.881);
  --chart-4: oklch(0.488 0.243 264.376);
  --chart-5: oklch(0.424 0.199 265.638);
  --sidebar: oklch(0.216 0.006 56.043);
  --sidebar-foreground: oklch(0.985 0.001 106.423);
  --sidebar-primary: oklch(0.623 0.214 259.815);
  --sidebar-primary-foreground: oklch(0.97 0.014 254.604);
  --sidebar-accent: oklch(0.268 0.007 34.298);
  --sidebar-accent-foreground: oklch(0.985 0.001 106.423);
  --sidebar-border: oklch(1 0 0 / 10%);
  --sidebar-ring: oklch(0.553 0.013 58.071);

  --surface: var(--card);
  --surface-raised: oklch(0.245 0.006 56);
  --hairline: oklch(1 0 0 / 0.08);
  --faint: oklch(0.58 0.012 56);
  /* Dark primary is deep (buttons), so the accent used for icons, switches and selection is
     shadcn's brighter sidebar-primary from the same blue ramp. */
  --iris: var(--sidebar-primary);
  --iris-soft: oklch(0.623 0.214 259.815 / 0.18);
  --iris-foreground: oklch(0.809 0.105 251.813);
  --iris-contrast: var(--primary-foreground);
  /* Dark: light comes from above, so raised things get a faint top edge and a dark contact shadow. */
  --elev-card: inset 0 1px 0 oklch(1 0 0 / 0.045), 0 0 0 1px oklch(1 0 0 / 0.06), 0 1px 2px oklch(0 0 0 / 0.25), 0 8px 24px -12px oklch(0 0 0 / 0.45);
  --elev-raised: inset 0 1px 0 oklch(1 0 0 / 0.08), 0 0 0 1px oklch(1 0 0 / 0.07), 0 1px 2px oklch(0 0 0 / 0.35), 0 2px 6px -2px oklch(0 0 0 / 0.3);
  --elev-raised-hover: inset 0 1px 0 oklch(1 0 0 / 0.1), 0 0 0 1px oklch(1 0 0 / 0.1), 0 1px 2px oklch(0 0 0 / 0.4), 0 4px 10px -3px oklch(0 0 0 / 0.4);
  --elev-pressed: inset 0 1px 3px oklch(0 0 0 / 0.35), 0 0 0 1px oklch(1 0 0 / 0.06);
  --elev-well: inset 0 1px 2px oklch(0 0 0 / 0.3), inset 0 0 0 1px oklch(1 0 0 / 0.07), 0 1px 0 oklch(1 0 0 / 0.03);
  --elev-knob: 0 0 0 0.5px oklch(0 0 0 / 0.3), 0 1px 3px oklch(0 0 0 / 0.45);
  --elev-key: inset 0 1px 0 oklch(1 0 0 / 0.06), inset 0 -1px 0 oklch(0 0 0 / 0.45), 0 0 0 0.5px oklch(0 0 0 / 0.3);
  --elev-selected: inset 0 1px 0 oklch(1 0 0 / 0.05), 0 1px 2px oklch(0 0 0 / 0.2);
  --elev-accent: inset 0 1px 0 oklch(1 0 0 / 0.35), 0 0 0 1px oklch(0.623 0.214 259.815 / 0.5), 0 1px 2px oklch(0 0 0 / 0.4), 0 4px 14px -4px oklch(0.623 0.214 259.815 / 0.45);
  --elev-soft: inset 0 1px 0 oklch(1 0 0 / 0.07), 0 0 0 1px oklch(1 0 0 / 0.05), 0 1px 2px oklch(0 0 0 / 0.3), 0 4px 12px -4px oklch(0 0 0 / 0.3);
  --elev-float: 0 0 0 1px var(--hairline), 0 12px 32px -8px oklch(0 0 0 / 0.45), 0 2px 6px oklch(0 0 0 / 0.2);
  --elev-panel: 0 0 0 1px var(--hairline), 0 24px 64px -12px oklch(0 0 0 / 0.55), 0 4px 16px oklch(0 0 0 / 0.25);
  --chip-time: oklch(0.8 0.1 220);
  --chip-clipboard: oklch(0.8 0.12 158);
  --chip-cursor: var(--iris-foreground);
  --chip-form: oklch(0.84 0.11 75);
  --chip-error: oklch(0.75 0.16 24);
  --chip-neutral: var(--foreground);
  --success: oklch(0.76 0.14 158);
  --warning: oklch(0.8 0.13 75);
  --favorite: oklch(0.82 0.14 75);
}
"#;

    /// Declarations by selector.
    pub struct Sheet {
        blocks: Vec<(String, HashMap<String, String>)>,
    }

    impl Sheet {
        pub fn parse(text: &str) -> Sheet {
            let mut text = text.to_owned();
            while let Some(start) = text.find("/*") {
                let end = text[start..]
                    .find("*/")
                    .map_or(text.len(), |e| start + e + 2);
                text.replace_range(start..end, "");
            }
            let mut blocks = Vec::new();
            let mut rest = text.as_str();
            while let Some(open) = rest.find('{') {
                let close = open + rest[open..].find('}').expect("closed block");
                let selector = rest[..open].trim().to_owned();
                let decls = rest[open + 1..close]
                    .split(';')
                    .filter_map(|d| d.split_once(':'))
                    .map(|(k, v)| {
                        (
                            k.trim().trim_start_matches("--").to_owned(),
                            v.trim().to_owned(),
                        )
                    })
                    .collect();
                blocks.push((selector, decls));
                rest = &rest[close + 1..];
            }
            Sheet { blocks }
        }

        fn block(&self, selector: &str) -> &HashMap<String, String> {
            &self
                .blocks
                .iter()
                .find(|(s, _)| s == selector)
                .unwrap_or_else(|| panic!("no {selector} block"))
                .1
        }

        /// A property's raw value as the theme sees it: `.dark` over `:root` when dark.
        fn raw(&self, name: &str, dark: bool) -> &str {
            let found = if dark {
                self.block(".dark").get(name)
            } else {
                None
            };
            found
                .or_else(|| self.block(":root").get(name))
                .unwrap_or_else(|| panic!("no --{name}"))
        }

        fn resolve<'a>(&'a self, value: &'a str, dark: bool) -> &'a str {
            match value
                .strip_prefix("var(--")
                .and_then(|v| v.strip_suffix(')'))
            {
                Some(name) => self.resolve(self.raw(name, dark), dark),
                None => value,
            }
        }

        pub fn colour(&self, name: &str, dark: bool) -> Oklch {
            oklch(self.resolve(self.raw(name, dark), dark))
        }

        pub fn vibrant_sidebar(&self, dark: bool) -> Oklch {
            oklch(&self.block(if dark { ".vibrant.dark" } else { ".vibrant" })["sidebar"])
        }

        /// The `:root` properties that are colours.
        pub fn colour_names(&self) -> Vec<String> {
            let root = self.block(":root");
            let mut names: Vec<String> = root
                .keys()
                .filter(|k| {
                    self.resolve(self.raw(k, false), false)
                        .starts_with("oklch(")
                })
                .cloned()
                .collect();
            names.retain(|n| !n.starts_with("elev-"));
            names.sort();
            names
        }

        pub fn names_with_prefix(&self, prefix: &str) -> Vec<String> {
            let mut names: Vec<String> = self
                .block(":root")
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            names.sort();
            names
        }

        pub fn shadows(&self, name: &str, dark: bool) -> Vec<Layer> {
            split_top(self.raw(name, dark), ',')
                .iter()
                .map(|layer| self.layer(layer, dark))
                .collect()
        }

        fn layer(&self, text: &str, dark: bool) -> Layer {
            let parts = split_top(text, ' ');
            let inset = parts.first().map(String::as_str) == Some("inset");
            let parts = &parts[inset as usize..];
            let (lengths, colour) = parts.split_at(parts.len() - 1);
            let len: Vec<f32> = lengths
                .iter()
                .map(|l| l.trim_end_matches("px").parse().expect("a length"))
                .collect();
            let at = |i: usize| len.get(i).copied().unwrap_or(0.0);
            Layer {
                x: at(0),
                y: at(1),
                blur: at(2),
                spread: at(3),
                color: oklch(self.resolve(&colour[0], dark)),
                inset,
            }
        }

        /// A length in points (`rem` is 16 px).
        pub fn length(&self, name: &str, dark: bool) -> f32 {
            let raw = self.raw(name, dark);
            match raw.strip_suffix("rem") {
                Some(rem) => rem.parse::<f32>().unwrap() * 16.0,
                None => raw.trim_end_matches("px").parse().unwrap(),
            }
        }

        /// `calc(var(--radius) * k)` → k; `var(--radius)` → 1.
        pub fn radius_factor(&self, name: &str) -> f32 {
            let raw = &self.block("@theme inline")[name];
            raw.split_once('*').map_or(1.0, |(_, k)| {
                k.trim_end_matches(')').trim().parse().unwrap()
            })
        }

        /// The numbers of a `linear(...)` easing.
        pub fn linear(&self, name: &str) -> Vec<f32> {
            let raw = self.raw(name, false);
            let inner = raw
                .strip_prefix("linear(")
                .and_then(|v| v.strip_suffix(')'))
                .expect("linear()");
            inner
                .split(',')
                .map(|n| n.trim().parse().unwrap())
                .collect()
        }

        pub fn millis(&self, name: &str) -> f32 {
            self.raw(name, false)
                .strip_suffix("ms")
                .expect("ms")
                .parse()
                .unwrap()
        }
    }

    /// Splits on `sep` outside parentheses, dropping empty pieces.
    fn split_top(text: &str, sep: char) -> Vec<String> {
        let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0);
        for ch in text.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                c if c == sep && depth == 0 => {
                    out.push(std::mem::take(&mut cur));
                    continue;
                }
                _ => {}
            }
            cur.push(ch);
        }
        out.push(cur);
        out.into_iter()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// `oklch(l c h)` or `oklch(l c h / a)`, `a` a number or a percentage.
    pub fn oklch(text: &str) -> Oklch {
        let inner = text
            .strip_prefix("oklch(")
            .and_then(|t| t.strip_suffix(')'))
            .unwrap_or_else(|| panic!("not oklch: {text}"));
        let (lch, alpha) = inner
            .split_once('/')
            .map_or((inner, None), |(l, a)| (l, Some(a.trim())));
        let n: Vec<f32> = lch.split_whitespace().map(|v| v.parse().unwrap()).collect();
        let a = alpha.map_or(1.0, |a| match a.strip_suffix('%') {
            Some(p) => p.parse::<f32>().unwrap() / 100.0,
            None => a.parse().unwrap(),
        });
        Oklch::new(n[0], n[1], n[2]).alpha(a)
    }

    #[test]
    fn reader_resolves_vars_and_percent_alphas() {
        let sheet = Sheet::parse(GLOBALS);
        assert_eq!(
            sheet.colour("iris", true),
            sheet.colour("sidebar-primary", true)
        );
        assert_eq!(sheet.colour("border", true).a, 0.1);
        assert_eq!(
            sheet.shadows("elev-float", false)[0].color,
            sheet.colour("hairline", false)
        );
        assert_eq!(sheet.shadows("elev-panel", true).len(), 3);
    }
}
