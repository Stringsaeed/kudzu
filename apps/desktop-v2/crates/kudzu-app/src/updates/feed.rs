use super::error::UpdateError;
use super::verify;
use super::{Channel, BETA_FEED, STABLE_FEED};
use semver::Version;
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;

pub fn url(channel: Channel) -> &'static str {
    match channel {
        Channel::Stable => STABLE_FEED,
        Channel::Beta => BETA_FEED,
    }
}

#[derive(Debug, Deserialize)]
pub struct Feed {
    pub version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub platforms: BTreeMap<String, Platform>,
}

#[derive(Debug, Deserialize)]
pub struct Platform {
    pub signature: String,
    pub url: String,
}

pub fn parse(bytes: &[u8]) -> Result<Feed, UpdateError> {
    serde_json::from_slice(bytes).map_err(|_| UpdateError::Feed)
}

pub fn is_newer(feed: &str, current: &str) -> Result<bool, UpdateError> {
    Ok(version(feed)?.cmp_precedence(&version(current)?) == Ordering::Greater)
}

fn version(raw: &str) -> Result<Version, UpdateError> {
    Version::parse(raw.strip_prefix('v').unwrap_or(raw)).map_err(|_| UpdateError::Feed)
}

pub fn platform(feed: &Feed) -> Result<&Platform, UpdateError> {
    let preferred = if cfg!(target_arch = "x86_64") {
        "darwin-x86_64"
    } else {
        "darwin-aarch64"
    };
    feed.platforms
        .get(preferred)
        .or_else(|| feed.platforms.get("darwin-aarch64"))
        .or_else(|| feed.platforms.get("darwin-x86_64"))
        .ok_or(UpdateError::Feed)
}

pub fn check_archive_url(url: &str, feed_version: &str) -> Result<(), UpdateError> {
    if !url.starts_with("https://github.com/") {
        return Err(UpdateError::Feed);
    }
    let name = verify::url_file_name(url).ok_or(UpdateError::Feed)?;
    if name != verify::expected_archive_name(feed_version) {
        return Err(UpdateError::Signature);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_metadata_does_not_count_as_newer() {
        assert!(!is_newer("1.0.0+bbb", "1.0.0+aaa").unwrap());
        assert!(!is_newer("1.0.0+build", "1.0.0").unwrap());
        assert!(is_newer("1.0.1+aaa", "1.0.0+zzz").unwrap());
    }

    #[test]
    fn release_candidates_order_between_the_stable_releases_around_them() {
        // The runbook stamps candidates 0.7.1-rc.1, 0.7.1-rc.2, ... over the installed 0.7.0.
        let order = ["0.7.0", "0.7.1-rc.1", "0.7.1-rc.2", "0.7.1-rc.10", "0.7.1"];
        for pair in order.windows(2) {
            assert!(is_newer(pair[1], pair[0]).unwrap(), "{pair:?}");
            assert!(!is_newer(pair[0], pair[1]).unwrap(), "{pair:?}");
        }
        assert!(!is_newer("0.7.1-rc.1", "0.7.1-rc.1").unwrap());
        assert!(
            check_archive_url(
                "https://github.com/Stringsaeed/kudzu/releases/download/v0.7.1-rc.1/Kudzu_0.7.1-rc.1_universal.app.tar.gz",
                "0.7.1-rc.1"
            )
            .is_ok()
        );
    }

    #[test]
    fn archive_url_must_be_https_github() {
        assert_eq!(
            check_archive_url(
                "http://github.com/Stringsaeed/kudzu/releases/download/v9.9.9/Kudzu_9.9.9_universal.app.tar.gz",
                "9.9.9"
            ),
            Err(UpdateError::Feed)
        );
        assert_eq!(
            check_archive_url(
                "https://evil.example/Kudzu_9.9.9_universal.app.tar.gz",
                "9.9.9"
            ),
            Err(UpdateError::Feed)
        );
        assert!(check_archive_url(
            "https://github.com/Stringsaeed/kudzu/releases/download/v9.9.9/Kudzu_9.9.9_universal.app.tar.gz",
            "9.9.9"
        )
        .is_ok());
    }
}
