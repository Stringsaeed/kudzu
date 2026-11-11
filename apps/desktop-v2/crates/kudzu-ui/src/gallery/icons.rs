//! Icons gallery scene: every Kudzu Lucide glyph at 16 pt.

use crate::theme::icons;
use rune::{col, icon, row, text, Color, El, TextStyle};

/// Point size the Accept list rasterises.
pub const SIZE_PT: f32 = 16.0;

const COLS: usize = 8;

/// Headless / debug gallery scene `icons`.
pub fn scene() -> El {
    let label = TextStyle::new(10.0, 400.0);
    let mut root = col().gap(16.0).padding(24.0);
    root = root.child(
        text("Icons")
            .font(TextStyle::new(18.0, 600.0))
            .color(Color::WHITE),
    );
    for chunk in icons::ALL.chunks(COLS) {
        let mut line = row().gap(20.0);
        for glyph in chunk {
            line = line.child(
                col()
                    .gap(6.0)
                    .child(icon(glyph, SIZE_PT).color(Color::WHITE).key(glyph.name))
                    .child(text(glyph.name).font(label).color(Color::WHITE)),
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
        let _tree = scene();
        // The import-list test in `theme::icons` guards the set; the gallery shows all of it.
        assert!(!icons::ALL.is_empty());
    }
}
