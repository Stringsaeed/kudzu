mod cli;
mod compose;
mod identity;
mod logging;
mod timing;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = cli::mode(args.iter().any(|a| a == "--mcp"), rune::is_headless());
    if mode == cli::LaunchMode::Bridge {
        eprintln!("MCP bridge is implemented in S32.");
        std::process::exit(2);
    }
    let hidden = args.iter().any(|a| a == "--hidden");
    let identity = identity::Identity::resolve(false);
    let dirs = rune::dirs::AppDirs::new(identity.bundle_id).expect("home directory is required");
    let _logging = logging::init(dirs.logs()).expect("could not initialize logging");
    log::info!(
        "{} {} starting",
        identity.name,
        include_str!("../version.txt").trim()
    );
    timing::mark("services_ready");
    rune::prewarm_gpu();
    let args = args.into_iter().filter(|a| a != "--hidden" && a != "--dev");
    rune::run_windows_with_args(rune::RunOptions { accessory: true }, args, move |_| {
        compose::windows(mode, hidden)
    });
}
