// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--mcp") {
        std::process::exit(kudzu_desktop_lib::mcp_bridge(args.iter().any(|a| a == "--dev")));
    }
    kudzu_desktop_lib::run();
}
