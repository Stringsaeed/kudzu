#[cfg(target_os = "macos")]
mod native;

#[cfg(target_os = "macos")]
mod spike;

fn main() {
    #[cfg(target_os = "macos")]
    if std::env::args().any(|arg| arg == "--probe") {
        spike::run();
        return;
    }
    eprintln!("usage: menubar --probe (macOS only)");
    std::process::exit(2);
}
