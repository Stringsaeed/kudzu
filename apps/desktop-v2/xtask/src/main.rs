mod check_deps;

fn main() {
    if std::env::args().nth(1).as_deref() != Some("check-deps") {
        eprintln!("usage: cargo xtask check-deps");
        std::process::exit(2);
    }
    if let Err(error) = check_deps::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!("workspace dependency and containment checks passed");
}
