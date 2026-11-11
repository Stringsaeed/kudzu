mod drag;
mod pack;
mod place;
mod settings;

#[cfg(target_os = "macos")]
mod bundle;
#[cfg(target_os = "macos")]
mod companion;
#[cfg(target_os = "macos")]
mod guide;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod probe;
#[cfg(target_os = "macos")]
mod report;

fn main() {
    if std::env::args().any(|arg| arg == "--probe") {
        #[cfg(target_os = "macos")]
        {
            pack::ensure_bundled();
            probe::run();
            return;
        }
        #[cfg(not(target_os = "macos"))]
        {
            eprintln!("permissions-spike --probe is macOS-only");
            std::process::exit(2);
        }
    }
    eprintln!("usage: permissions-spike --probe");
    std::process::exit(2);
}
