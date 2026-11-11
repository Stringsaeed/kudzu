mod bundle;
mod check_deps;
mod parity;
mod run;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result: Result<(), Box<dyn std::error::Error>> = match args.first().map(String::as_str) {
        Some("check-deps") if args.len() == 1 => check_deps::run()
            .map(|()| println!("workspace dependency and containment checks passed"))
            .map_err(Into::into),
        Some("bundle") => bundle::parse(&args[1..])
            .and_then(|(variant, universal)| bundle::build(variant, universal))
            .map(|path| println!("Built {}", path.display())),
        Some("run") if args.len() == 1 => run::run(),
        Some("parity") => parity::run(&args[1..]),
        _ => Err(
            "usage: cargo xtask check-deps | bundle --variant dev|prod [--universal] | run | parity".into(),
        ),
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
