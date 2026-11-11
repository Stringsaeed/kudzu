mod place;
mod stats;

#[cfg(target_os = "macos")]
mod bus;
#[cfg(target_os = "macos")]
mod cycle;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod panel;
#[cfg(target_os = "macos")]
mod probe;
#[cfg(target_os = "macos")]
mod report;
#[cfg(target_os = "macos")]
mod scenes;

fn main() {
    #[cfg(target_os = "macos")]
    if std::env::args().any(|arg| arg == "--probe") {
        probe::run();
        return;
    }
    eprintln!("usage: panels --probe");
    std::process::exit(2);
}
