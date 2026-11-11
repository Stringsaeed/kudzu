use crate::release::{root, Result};

pub fn parse(raw: &str) -> Result<semver::Version> {
    Ok(semver::Version::parse(
        raw.strip_prefix('v').unwrap_or(raw),
    )?)
}

/// `version.txt` is stamped into `Info.plist`, the archive name, the signed trusted comment and
/// `latest.json`. They must all spell the version the same way, so anything `parse` would
/// normalize (a `v` prefix, say) is an error rather than a silent difference.
pub fn canonical(raw: &str) -> Result<String> {
    let raw = raw.trim();
    let parsed = parse(raw)?.to_string();
    if parsed != raw {
        return Err(
            format!("version.txt must be canonical SemVer: write {parsed}, not {raw}").into(),
        );
    }
    Ok(parsed)
}

pub fn run(args: &[String]) -> Result<()> {
    let [raw] = args else {
        return Err("usage: cargo xtask set-version <semver>".into());
    };
    let version = parse(raw)?;
    std::fs::write(root().join("version.txt"), format!("{version}\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_canonical_versions_are_stamped() {
        assert_eq!(canonical("0.7.1-rc.1\n").unwrap(), "0.7.1-rc.1");
        assert_eq!(canonical("0.7.0").unwrap(), "0.7.0");
        for bad in ["v0.7.1", "0.7", "01.2.3", "0.7.1 rc", "", "0.7.1-rc.01"] {
            assert!(canonical(bad).is_err(), "{bad:?}");
        }
        assert_eq!(
            canonical("v0.7.1").unwrap_err().to_string(),
            "version.txt must be canonical SemVer: write 0.7.1, not v0.7.1"
        );
    }

    #[test]
    fn a_release_candidate_is_a_valid_stamp() {
        let version = parse("v0.7.1-rc.1").unwrap();
        assert_eq!(version.to_string(), "0.7.1-rc.1");
        assert!(!version.pre.is_empty());
        assert_eq!(canonical(&version.to_string()).unwrap(), "0.7.1-rc.1");
    }
}
