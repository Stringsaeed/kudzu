//! The component gallery: one window (headless and debug builds only), one scene per page.
//! S04 adds the `tokens` scene; S05 and S06 add theirs to `SCENES` and `Page`.
pub mod components;
pub mod icons;
pub mod tokens;
pub mod widgets;

use std::sync::LazyLock;

use rune::{AppSpec, Cx, El};

use crate::theme::{fonts, Appearance, Theme};

/// The window key (`--window gallery`).
pub const KEY: &str = "gallery";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Tokens,
    Icons,
    Component(components::Primitive),
    Widget(widgets::Widget),
}

#[derive(Debug, Default)]
pub struct GalleryState {
    page: Page,
    /// The field the widget scenes edit (the stepper's text).
    scratch: rune::TextEdit,
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

/// The gallery's own scenes, then one per shadcn primitive (S06).
static ALL_SCENES: LazyLock<Vec<Scene>> =
    LazyLock::new(|| [SCENES, components::SCENES, widgets::SCENES].concat());

fn view(s: &GalleryState, cx: &mut Cx<GalleryState>) -> El {
    // The gallery follows `--theme`; the app follows the Appearance setting.
    let theme = Theme::resolve(Appearance::System, cx.system_dark());
    match s.page {
        Page::Tokens => tokens::view(theme),
        Page::Icons => icons::scene(theme),
        Page::Component(p) => components::view(p, theme),
        Page::Widget(w) => widgets::view(w, s, cx, theme),
    }
}

/// The gallery window, opened only when asked for.
pub fn window() -> rune::AnyWindowSpec {
    let spec = AppSpec {
        scenes: ALL_SCENES.as_slice(),
        fonts: fonts::FILES,
        families: fonts::families(),
        ..AppSpec::new("Kudzu Gallery", tokens::SIZE, GalleryState::default, view)
    };
    let app = rune::App::new(spec).font_smoothing(rune::FontSmoothing::Off);
    crate::theme::window::spec(KEY, app).into()
}
