//! The preview body of a clip, one module per kind. Every function builds an element from
//! plain data and calls back through [`Hooks`], so the main window and the large preview use
//! them alike and nothing here names a window state.

pub mod color;
pub mod files;
pub mod image;
pub mod link;
pub mod loader;
pub mod other;
pub mod text;

use kudzu_core::clipboard::{ClipEntry, ClipKind};
use rune::{col, row, El, Tx};

use crate::theme::Theme;
use loader::Media;

/// Characters drawn before "…and N more" (keeps huge pastes from stalling layout).
pub const RENDER_LIMIT: usize = 60_000;
/// Lines of code drawn before "…and N more lines".
pub const CODE_LINE_LIMIT: usize = 1_500;
/// Files listed before "…and N more".
pub const FILE_LIMIT: usize = 200;

/// What a body needs to draw. Borrowed from the window's state for one build.
pub struct Body<'a> {
    pub entry: &'a ClipEntry,
    /// The complete text once it has loaded; `None` while it is on its way.
    pub full: Option<&'a str>,
    pub terms: &'a [String],
    pub large: bool,
    pub media: Option<&'a Media>,
    /// The colour value just copied (a check shows beside it for a moment).
    pub copied_value: Option<&'a str>,
}

/// How a body reaches its window: pressed buttons call these with what they act on.
pub struct Hooks<S: 'static> {
    /// "Open in browser" or "Write email".
    pub open_link: fn(&mut S, &mut Tx),
    /// Open the file at this index of the entry.
    pub open_file: fn(&mut S, usize, &mut Tx),
    /// Show the file at this index in Finder.
    pub reveal_file: fn(&mut S, usize, &mut Tx),
    /// Copy a colour value (HEX, RGB, …).
    pub copy_value: fn(&mut S, String, &mut Tx),
}

impl<S: 'static> Clone for Hooks<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S: 'static> Copy for Hooks<S> {}

/// The body for `b.entry`, by kind.
pub fn body<S: 'static>(theme: &Theme, b: &Body, hooks: Hooks<S>) -> El {
    match b.entry.kind {
        ClipKind::Code => text::code(theme, b),
        ClipKind::Link => link::view(theme, b, hooks),
        ClipKind::Color => color::view(theme, b, hooks),
        ClipKind::Image => image::view(theme, b),
        ClipKind::File => files::view(theme, b, hooks),
        ClipKind::Other => other::view(theme, b),
        ClipKind::Text => text::view(theme, b),
    }
}

/// The inset well behind code, links and images (`bg-foreground/[0.025] shadow-well`).
pub fn well(theme: &Theme) -> El {
    theme.depth.well.iter().fold(
        col()
            .w_full()
            .radius(14.0)
            .fill(theme.c.foreground.alpha(0.025)),
        |el, shadow| el.box_shadow(*shadow),
    )
}

/// The card surface behind lists of values and files (`bg-surface shadow-card`).
pub fn card(theme: &Theme) -> El {
    theme.depth.card.iter().fold(
        col().w_full().radius(14.0).fill(theme.c.surface),
        |el, shadow| el.box_shadow(*shadow),
    )
}

/// A hairline between rows of a card.
pub fn divider(theme: &Theme) -> El {
    row().w_full().h(1.0).fill(theme.c.hairline)
}

/// Checkerboard behind transparent images: squares of `foreground/7%`, drawn as plain
/// elements because Rune has no tiled fill (RUNE-GAP: delete when it does). Covers up to
/// `w` × `h` points; the caller clips it to its box.
pub fn checkerboard(theme: &Theme, w: f32, h: f32) -> El {
    const CELL: f32 = 14.0;
    let ink = theme.c.foreground.alpha(0.07);
    let mut board = col().absolute(0.0, 0.0).size(w, h).a11y_hidden();
    let (cols, rows) = ((w / CELL).ceil() as usize, (h / CELL).ceil() as usize);
    for r in 0..rows {
        for c in (r % 2..cols).step_by(2) {
            board = board.child(
                row()
                    .absolute(c as f32 * CELL, r as f32 * CELL)
                    .size(CELL, CELL)
                    .fill(ink),
            );
        }
    }
    board
}
