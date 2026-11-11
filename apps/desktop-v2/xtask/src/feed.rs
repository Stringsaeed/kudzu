use crate::{release::Result, version};
use serde_json::{json, Value};
use std::{cmp::Ordering, fs, path::Path};

pub fn newer(release: &str, current: &str) -> Result<bool> {
    Ok(version::parse(release)?.cmp_precedence(&version::parse(current)?) == Ordering::Greater)
}
pub fn manifest(version: &str, signature: &str, url: &str, notes: &str, date: &str) -> Value {
    let platform = json!({"signature": signature, "url": url});
    json!({"version": version, "notes": notes, "pub_date": date,
        "platforms": {"darwin-aarch64": platform, "darwin-x86_64": platform}})
}
pub fn run(args: &[String]) -> Result<()> {
    let mut release = None;
    let mut current = None;
    let mut out = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let value = iter.next().ok_or("missing feed argument value")?;
        match arg.as_str() {
            "--release" => release = Some(value),
            "--current" => current = Some(value),
            "--out" => out = Some(value),
            _ => return Err("unknown feed option".into()),
        }
    }
    let bytes = fs::read(release.ok_or("--release required")?)?;
    let candidate: Value = serde_json::from_slice(&bytes)?;
    let candidate_version = candidate["version"].as_str().ok_or("missing version")?;
    version::parse(candidate_version)?;
    let out = out.ok_or("--out required")?;
    if let Some(path) = current.filter(|p| Path::new(p).exists()) {
        let old: Value = serde_json::from_slice(&fs::read(path)?)?;
        if !newer(
            candidate_version,
            old["version"].as_str().ok_or("missing version")?,
        )? {
            println!("feed unchanged");
            std::process::exit(3);
        }
    }
    fs::write(out, bytes)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn semver_precedence_matches_old_feed() {
        let order = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.1.0",
            "2.0.0",
        ];
        for pair in order.windows(2) {
            assert!(newer(pair[1], pair[0]).unwrap());
            assert!(!newer(pair[0], pair[1]).unwrap());
        }
        assert!(!newer("v1.0.0+bbb", "1.0.0+aaa").unwrap());
        assert!(!newer("1.0.0", "1.0.0").unwrap());
        assert!(newer("broken", "1.0.0").is_err());
    }

    #[test]
    fn candidates_stamped_as_release_candidates_advance_the_feed_in_order() {
        // 0.7.0 is the frozen app's version. A candidate is 0.7.1-rc.N, and S72 needs a second,
        // higher one, because S36 installs only a strictly greater version.
        let order = ["0.7.0", "0.7.1-rc.1", "0.7.1-rc.2", "0.7.1"];
        for pair in order.windows(2) {
            assert!(newer(pair[1], pair[0]).unwrap(), "{pair:?}");
            assert!(!newer(pair[0], pair[1]).unwrap(), "{pair:?}");
        }
        assert!(!newer("0.7.1-rc.1", "0.7.1-rc.1").unwrap());
        // The feed file itself accepts and carries the candidate version unchanged.
        let manifest = manifest("0.7.1-rc.1", "sig", "https://example.invalid/x", "n", "d");
        assert_eq!(manifest["version"], "0.7.1-rc.1");
    }
}
