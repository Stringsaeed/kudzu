//! A gallery of every kit component in each state, for headless snapshots while porting:
//! `KUDZU_GALLERY=controls` or `KUDZU_GALLERY=overlays` makes the main window draw it.

pub mod controls;
pub mod overlays;

use rune::{Cx, El};

use crate::ui::main::MainUi;

/// Which gallery the environment asks for.
pub fn requested() -> Option<&'static str> {
    static WHICH: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    WHICH.get_or_init(|| std::env::var("KUDZU_GALLERY").ok()).as_deref()
}

#[derive(Default)]
pub struct GalleryState {
    pub controls: controls::State,
    pub overlays: overlays::State,
}

pub fn view(which: &str, s: &MainUi, cx: &mut Cx<MainUi>) -> El {
    match which {
        "overlays" => overlays::view(s, cx),
        _ => controls::view(s, cx),
    }
}
