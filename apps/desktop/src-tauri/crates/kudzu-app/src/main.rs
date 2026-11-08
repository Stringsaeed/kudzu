//! Kudzu, the native app: Rune UI over the Tauri shell's Rust.

// The Tauri shell's settings and store, compiled as they are, so both apps share one copy.
#[path = "../../../src/settings.rs"]
mod settings;
// Shared with the Tauri shell (edition 2021), so no let-chains.
#[allow(clippy::collapsible_if)]
#[path = "../../../src/store.rs"]
mod store;

mod backend;
mod ui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--mcp") {
        std::process::exit(backend::agent::bridge(args.iter().any(|a| a == "--dev")));
    }
    rune::prewarm_gpu();
    rune_mac::launch();
    backend::logging::init();
    let first_run = backend::load();
    ui::run(first_run);
}
