//! All Kudzu windows and overlays use CSS sRGB compositing and grayscale font smoothing.
//! Construct through this function, including non-activating panels. Cached themes therefore
//! need no blend-dependent alpha compensation (`rune::theme::for_blend`).
pub fn spec<S: 'static>(key: impl Into<String>, app: rune::App<S>) -> rune::WindowSpec<S> {
    rune::WindowSpec::from_app(key, app)
        .srgb_blend(true)
        .font_smoothing(rune::FontSmoothing::Off)
}
