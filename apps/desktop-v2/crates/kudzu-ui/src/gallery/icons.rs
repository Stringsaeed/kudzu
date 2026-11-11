//! Icons gallery scene: every Kudzu Lucide glyph at 16 pt, in the theme's foreground on its
//! background (goldens `gallery-icons.{light,dark}`).

use crate::theme::{icons, Theme};
use rune::{col, icon, row, text, El, TextStyle};

/// Point size the Accept list rasterises.
pub const SIZE_PT: f32 = 16.0;

const COLS: usize = 8;

/// Headless / debug gallery scene `icons`.
pub fn scene(theme: &Theme) -> El {
    let (ink, muted) = (theme.c.foreground, theme.c.muted_foreground);
    let label = TextStyle::new(10.0, 400.0);
    let mut root = col()
        .key("icons")
        .w_full()
        .h_full()
        .gap(16.0)
        .padding(24.0)
        .fill(theme.c.background);
    root = root.child(text("Icons").font(TextStyle::new(18.0, 600.0)).color(ink));
    for chunk in icons::ALL.chunks(COLS) {
        let mut line = row().gap(20.0);
        for glyph in chunk {
            line = line.child(
                col()
                    .gap(6.0)
                    .child(icon(glyph, SIZE_PT).color(ink).key(glyph.name))
                    .child(text(glyph.name).font(label).color(muted)),
            );
        }
        root = root.child(line);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_draws_every_static_at_16pt() {
        assert_eq!(SIZE_PT, 16.0);
        let _tree = scene(Theme::dark());
        // The import-list test in `theme::icons` guards the set; the gallery shows all of it.
        assert!(!icons::ALL.is_empty());
    }

    /// `lucide!` puts Lucide's root paint attributes on a wrapping `<g>`; Rune must inherit them
    /// exactly as it reads them inline (S05 review).
    #[test]
    fn group_inherited_stroke_covers_the_inline_pixels() {
        static INLINE_X: rune::Icon = rune::Icon {
            name: "x-inline",
            view: 24.0,
            body: r#"<path d="M18 6 6 18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="m6 6 12 12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>"#,
        };
        for px in [16, 24, 48] {
            let grouped = rune::text::icon_coverage(&icons::X, px);
            let inline = rune::text::icon_coverage(&INLINE_X, px);
            assert!(grouped.iter().any(|&c| c != 0), "X is blank at {px}");
            assert_eq!(grouped, inline, "X at {px} px");
        }
    }
}
