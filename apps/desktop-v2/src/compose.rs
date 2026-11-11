use crate::{cli::LaunchMode, identity::Identity};

pub fn windows(mode: LaunchMode, hidden: bool, identity: Identity) -> Vec<rune::AnyWindowSpec> {
    // No platform adapters exist in S01. S40 must keep native assembly in this arm.
    let mut windows = match mode {
        LaunchMode::Fixtures => placeholder(false, identity),
        LaunchMode::Native => placeholder(hidden, identity),
        LaunchMode::Bridge => unreachable!("bridge never constructs windows"),
    };
    // The gallery: always headless, and in debug builds when launched with `--gallery`.
    let asked = cfg!(debug_assertions) && rune::extra_args().iter().any(|a| a == "--gallery");
    if mode == LaunchMode::Fixtures || asked {
        windows.push(kudzu_ui::gallery::window());
    }
    windows
}

fn placeholder(hidden: bool, identity: Identity) -> Vec<rune::AnyWindowSpec> {
    let spec = rune::AppSpec::new(
        identity.name,
        (1180.0, 760.0),
        || Identity::from_bundle_id(None),
        |identity, _| {
            rune::col()
                .w_full()
                .h_full()
                .padding(32.0)
                .child(rune::text(identity.name).key("workspace-title"))
        },
    );
    let mut recorded = false;
    let app = rune::App::new(spec).on_frame_drawn(move |_| {
        if !recorded {
            recorded = true;
            crate::timing::mark("main_first_frame");
        }
    });
    vec![kudzu_ui::theme::window::spec("main", app)
        .state(identity)
        .options(rune::WindowOptions {
            visible: !hidden,
            ..Default::default()
        })
        .into()]
}
