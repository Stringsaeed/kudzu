//! The editor's reference state (`docs/migration/reference/STATES.md`, `capture.mjs:47`).
//!
//! `snippet-actions` (the sample library on its first snippet, More actions open) shows the
//! whole window, and the S42 sidebar is not built yet, so it is not in `main::scenes::SCENES`:
//! `cargo xtask parity` lists it as not yet built (PLAN S73, "From wave 9"). The editor's own
//! landmarks are checked by `main/tests/editor_landmarks.rs` from this builder.

use crate::main::library;
use crate::shell::MainState;

/// The sample library opened on its first snippet, with its preview rendered.
pub fn sample(s: &mut MainState) {
    library::scenes::seed_sample(s);
    super::open_selected(s);
    super::preview::render_now(s);
}

/// `snippet-actions`: [`sample`] with More actions open.
pub fn snippet_actions(s: &mut MainState) {
    sample(s);
    s.editor.header.more_open = true;
}
