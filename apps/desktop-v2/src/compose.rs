use crate::cli::LaunchMode;

pub fn windows(mode: LaunchMode) -> Vec<rune::AnyWindowSpec> {
    // No platform adapters exist in S01. S40 must keep native assembly in this arm.
    let mut windows = match mode {
        LaunchMode::Fixtures => placeholder(),
        LaunchMode::Native => placeholder(),
        LaunchMode::Bridge => unreachable!("bridge never constructs windows"),
    };
    // The gallery: always headless, and in debug builds when launched with `--gallery`.
    let asked = cfg!(debug_assertions) && rune::extra_args().iter().any(|a| a == "--gallery");
    if mode == LaunchMode::Fixtures || asked {
        windows.push(kudzu_ui::gallery::window());
    }
    windows
}

fn placeholder() -> Vec<rune::AnyWindowSpec> {
    let spec = rune::AppSpec::new(
        "Kudzu Next",
        (1180.0, 760.0),
        || (),
        |_, _| {
            rune::col()
                .w_full()
                .h_full()
                .padding(32.0)
                .child(rune::text("Kudzu Next").key("workspace-title"))
        },
    );
    vec![rune::WindowSpec::from_app("main", rune::App::new(spec)).into()]
}
