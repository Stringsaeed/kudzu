//! PLAN §3, draft S65-wordmark: square Icon bounds include transparent letterboxing.
//! Decode the original SVG through Rune's image cache so its drawn bounds keep its aspect.
use crate::{shell::MainState, theme::Theme};
use rune::{el, El, Image, ImageCache};
use std::sync::Arc;

pub struct Brand {
    _cache: ImageCache,
    light: Image,
    dark: Image,
    ready: bool,
}
impl Brand {
    pub fn new(s: &MainState) -> Self {
        let handle = s.shell.handles().main.clone();
        let cache = ImageCache::new(Arc::new(move || handle.wake()));
        let svg = include_str!("../../../../../design/wordmark/kudzu-wordmark-echo.svg");
        let load = |color| {
            cache.svg(
                Arc::<[u8]>::from(svg.replace("currentColor", color).into_bytes()),
                224.0,
                2.0,
            )
        };
        let light = load("#0c0a09");
        let dark = load("#fafaf9");
        Self {
            _cache: cache,
            light,
            dark,
            ready: false,
        }
    }
    pub fn poll(&mut self) -> bool {
        let ready = self.light.is_ready() && self.dark.is_ready();
        let changed = ready != self.ready;
        self.ready = ready;
        changed
    }
    pub fn wait(&self) {
        self.light.wait().expect("bundled wordmark");
        self.dark.wait().expect("bundled wordmark");
    }
    pub fn view(&self, t: &Theme) -> El {
        let image = if t.dark { &self.dark } else { &self.light };
        el().size(224.0, 61.25)
            .pixels(image.pixels())
            .role("icon")
            .label("Kudzu")
    }
}
pub fn ensure(s: &mut MainState) {
    if s.onboarding.brand.is_none() {
        s.onboarding.brand = Some(Brand::new(s));
    }
}
