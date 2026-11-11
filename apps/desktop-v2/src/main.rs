mod cli;
mod compose;
mod identity;
mod logging;

fn main() {
    let mode = cli::mode(std::env::args().any(|a| a == "--mcp"), rune::is_headless());
    if mode == cli::LaunchMode::Bridge {
        eprintln!("MCP bridge is implemented in S32.");
        std::process::exit(2);
    }
    rune::prewarm_gpu();
    let args = std::env::args().skip(1);
    rune::run_windows_with_args(rune::RunOptions { accessory: true }, args, move |_| {
        compose::windows(mode)
    });
}
