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
    log::info!(
        "{} {} starting",
        identity.name,
        include_str!("../version.txt").trim()
    );
    rune::prewarm_gpu();
    rune::run_windows_with_args(
        rune::RunOptions { accessory: true },
        args.rune_args,
        move |_| {
            let windows = compose::windows(args.mode, args.hidden, identity);
            // S40 moves this mark to the point where real services finish starting.
            timing::mark("services_ready");
            windows
        },
    );
}
