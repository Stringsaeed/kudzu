//! The component gallery: one window (headless and debug builds only), one scene per page.
//! S04 adds the `tokens` scene; S05 and S06 add theirs to `SCENES` and `Page`.
pub mod icons;
pub mod tokens;

use rune::{AppSpec, Cx, El};

use crate::theme::{fonts, Appearance, Theme};

/// The window key (`--window gallery`).
pub const KEY: &str = "gallery";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Tokens,
    Icons,
}

#[derive(Debug, Default)]
pub struct GalleryState {
    page: Page,
}

fn tokens_page(s: &mut GalleryState) {
    s.page = Page::Tokens;
}

fn icons_page(s: &mut GalleryState) {
    s.page = Page::Icons;
}

/// A named scene and how it sets the state up.
type Scene = (&'static str, fn(&mut GalleryState));

const SCENES: &[Scene] = &[("tokens", tokens_page), ("icons", icons_page)];

fn view(s: &GalleryState, cx: &mut Cx<GalleryState>) -> El {
    // The gallery follows `--theme`; the app follows the Appearance setting.
    let theme = Theme::resolve(Appearance::System, cx.system_dark());
    match s.page {
        Page::Tokens => tokens::view(theme),
        // The icon scene draws white glyphs, so it sits on black in both themes.
        Page::Icons => icons::scene().fill(rune::Color::BLACK),
    }
}

/// The gallery window, opened only when asked for.
pub fn window() -> rune::AnyWindowSpec {
    let spec = AppSpec {
        scenes: SCENES,
        fonts: fonts::FILES,
        families: fonts::families(),
        ..AppSpec::new("Kudzu Gallery", tokens::SIZE, GalleryState::default, view)
    };
    let app = rune::App::new(spec).font_smoothing(rune::FontSmoothing::Off);
    rune::WindowSpec::from_app(KEY, app).into()
}
