use super::error::UpdateError;
use super::{Channel, BETA_FEED, STABLE_FEED};
use semver::Version;
use serde::Deserialize;
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
    Ok(version(feed)? > version(current)?)
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
