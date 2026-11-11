mod compose;

#[cfg(target_os = "macos")]
mod backdrop;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod probe;
#[cfg(target_os = "macos")]
mod scene;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        #[cfg(target_os = "macos")]
        Some("--probe") => probe::run(&args[1..]),
        Some("compose") => compose::run(&args[1..]),
        _ => {
            eprintln!("usage: chrome --probe --out DIR | chrome compose --raw DIR --reference DIR --out DIR");
            std::process::exit(2);
        }
    }
}
