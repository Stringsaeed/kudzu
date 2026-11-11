#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Bridge,
    Fixtures,
    Native,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Arguments {
    pub mode: LaunchMode,
    pub hidden: bool,
    pub dev: bool,
    pub rune_args: Vec<String>,
}

pub fn parse(args: impl IntoIterator<Item = String>, headless: bool) -> Arguments {
    let mut mcp = false;
    let mut hidden = false;
    let mut dev = false;
    let mut rune_args = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--mcp" => mcp = true,
            "--hidden" => hidden = true,
            "--dev" => dev = true,
            _ => rune_args.push(arg),
        }
    }
    let mode = match (mcp, headless) {
        (true, _) => LaunchMode::Bridge,
        (_, true) => LaunchMode::Fixtures,
        _ => LaunchMode::Native,
    };
    Arguments {
        mode,
        hidden,
        dev,
        rune_args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_precedes_headless_and_flags_are_not_forwarded() {
        let args = ["--mcp", "--hidden", "--dev", "--tree", "out.json"].map(String::from);
        assert_eq!(
            parse(args, true),
            Arguments {
                mode: LaunchMode::Bridge,
                hidden: true,
                dev: true,
                rune_args: vec!["--tree".into(), "out.json".into()]
            }
        );
        assert_eq!(parse(Vec::new(), true).mode, LaunchMode::Fixtures);
        assert_eq!(parse(Vec::new(), false).mode, LaunchMode::Native);
    }
}
