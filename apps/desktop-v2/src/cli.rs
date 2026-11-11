#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchMode {
    Bridge,
    Fixtures,
    Native,
}

pub fn mode(mcp: bool, headless: bool) -> LaunchMode {
    match (mcp, headless) {
        (true, _) => LaunchMode::Bridge,
        (_, true) => LaunchMode::Fixtures,
        _ => LaunchMode::Native,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_precedes_headless_and_native_is_only_the_default() {
        assert_eq!(mode(true, true), LaunchMode::Bridge);
        assert_eq!(mode(true, false), LaunchMode::Bridge);
        assert_eq!(mode(false, true), LaunchMode::Fixtures);
        assert_eq!(mode(false, false), LaunchMode::Native);
    }
}
