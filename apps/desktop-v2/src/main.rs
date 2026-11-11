mod cli;
mod compose;
mod identity;
mod logging;
mod timing;

fn main() {
    let args = cli::parse(std::env::args().skip(1), rune::is_headless());
    if args.mode == cli::LaunchMode::Bridge {
        eprintln!("MCP bridge is implemented in S32.");
        std::process::exit(2);
    }
    let identity = identity::Identity::resolve(args.dev);
    let dirs = rune::dirs::AppDirs::new(identity.bundle_id).expect("home directory is required");
    // Headless fixture runs can execute concurrently and need no product file sink.
    if args.mode != cli::LaunchMode::Fixtures {
        logging::init(dirs.logs()).expect("could not initialize logging");
    }
    let version = include_str!("../version.txt").trim();
    log::info!("{} {version} starting", identity.name);
    // The instance lock is taken here, before NSApplication starts; a second copy exits.
    let Some(app) = compose::start(args.mode, args.dev, identity, &dirs) else {
        log::info!("{} is already running", identity.name);
        return;
    };
    timing::mark("services_ready");
    let app = std::sync::Arc::new(app);
    let windows = app.clone();
    rune::prewarm_gpu();
    rune::run_windows_with_args(
        rune::RunOptions { accessory: true },
        args.rune_args,
        move |_| {
            let specs = compose::windows(&windows, args.mode, args.hidden, identity);
            timing::mark("windows_built");
            specs
        },
    );
    let _ = app.kudzu.shutdown(); // A headless loop returns here; quit paths already ran it.
    if let Some(scratch) = &app.scratch {
        let _ = std::fs::remove_dir_all(scratch);
    }
}
