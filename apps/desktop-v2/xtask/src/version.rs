use crate::release::{root, Result};

pub fn parse(raw: &str) -> Result<semver::Version> {
    Ok(semver::Version::parse(
        raw.strip_prefix('v').unwrap_or(raw),
    )?)
}

pub fn run(args: &[String]) -> Result<()> {
    let [raw] = args else {
        return Err("usage: cargo xtask set-version <semver>".into());
    };
    let version = parse(raw)?;
    std::fs::write(root().join("version.txt"), format!("{version}\n"))?;
    Ok(())
}
