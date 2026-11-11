mod cli;
mod compose;
mod identity;
mod logging;
mod timing;

fn main() {
    let args = cli::parse(std::env::args().skip(1), rune::is_headless());
    let identity = identity::Identity::resolve(args.dev);
    let dirs = rune::dirs::AppDirs::new(identity.bundle_id).expect("home directory is required");
    let version = include_str!("../version.txt").trim();
    if args.mode == cli::LaunchMode::Bridge {
        // `--mcp`: relay stdio to the running app's socket; never starts Rune or services.
        let paths = kudzu_app::paths::Paths {
            data: dirs.data().into(),
            logs: dirs.logs().into(),
        };
        let (input, output) = (std::io::stdin().lock(), std::io::stdout().lock());
        let result = kudzu_app::agents::bridge::run(input, output, &paths.agent_socket(), version);
        std::process::exit(i32::from(result.is_err()));
    }
    // Headless fixture runs can execute concurrently and need no product file sink.
    if args.mode != cli::LaunchMode::Fixtures {
        logging::init(dirs.logs()).expect("could not initialize logging");
    }
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
