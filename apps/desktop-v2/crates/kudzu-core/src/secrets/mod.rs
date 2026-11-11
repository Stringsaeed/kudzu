//! Finds passwords, API keys, tokens and private keys in text, so Kudzu can withhold them from
//! AI agents.
//!
//! Detection is heuristic. It combines four kinds of evidence:
//!
//! - **Known formats**: PEM private keys, JWTs, and tokens with a vendor prefix (`ghp_`,
//!   `sk-ant-`, `AKIA…`, `xoxb-`, …).
//! - **Labels**: a value after `password=`, `"api_key": "…"`, `Secret: …`, or `Bearer …`.
//! - **Credentials in URLs**: the password in `scheme://user:password@host`, and webhook URLs
//!   whose path is the secret.
//! - **Shape**: a lone word that looks generated (mixed character classes, high entropy), or a
//!   long random-looking run inside other text.
//!
//! Short or dictionary-like passwords without a label (`hunter2`) can't be told apart from
//! ordinary words, so a miss is always possible. When unsure, the rules lean towards
//! withholding.
//!
//! - `formats`: known key formats and vendor prefixes.
//! - `shape`: generated-looking runs and lone words.
//! - `labels`: labelled values, credentials in URLs and `Authorization` headers.
//!
//! [`SecretKind`] is stored on clipboard entries, so it keeps its serde derives.

mod formats;
mod labels;
mod shape;

use std::ops::Range;

use serde::{Deserialize, Serialize};

use formats::{jwts, private_keys, sendgrid, words};
use labels::{authorization, labelled, url_credentials};
use shape::lone_word;

/// What a finding looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretKind {
    /// A PEM or OpenSSH private key block.
    PrivateKey,
    /// A vendor API key or a long random-looking token.
    ApiKey,
    /// A JWT or an `Authorization` header value.
    Token,
    /// A labelled password, or a lone word that looks like one.
    Password,
    /// A labelled secret, or a password inside a URL.
    Credential,
}

impl SecretKind {
    /// A short noun for messages ("withheld: API key").
    pub fn label(self) -> &'static str {
        match self {
            Self::PrivateKey => "private key",
            Self::ApiKey => "API key",
            Self::Token => "access token",
            Self::Password => "password",
            Self::Credential => "secret",
        }
    }
}

/// One secret: its byte range in the scanned text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub range: Range<usize>,
    pub kind: SecretKind,
}

/// Text with its secrets replaced by `[redacted: …]` markers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redacted {
    pub text: String,
    /// The kinds found, in order of first appearance, without repeats.
    pub kinds: Vec<SecretKind>,
    /// The text is a secret as a whole (nothing but secrets and whitespace).
    pub whole: bool,
}

impl Redacted {
    pub fn is_clean(&self) -> bool {
        self.kinds.is_empty()
    }
}

/// Every secret in `text`, sorted and non-overlapping.
pub fn scan(text: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    private_keys(text, &mut found);
    jwts(text, &mut found);
    sendgrid(text, &mut found);
    words(text, &mut found);
    url_credentials(text, &mut found);
    authorization(text, &mut found);
    labelled(text, &mut found);
    lone_word(text, &mut found);
    merge(found)
}

/// Text scanned by [`detect`]; a secret is almost always near the start of a copy.
pub const DETECT_LIMIT: usize = 256 * 1024;

/// The first kind of secret in `text` (its first [`DETECT_LIMIT`] bytes), if any.
pub fn detect(text: &str) -> Option<SecretKind> {
    let mut end = text.len().min(DETECT_LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    scan(&text[..end]).first().map(|f| f.kind)
}

/// Whether `text` contains anything that looks like a secret.
pub fn contains_secret(text: &str) -> bool {
    !scan(text).is_empty()
}

/// The marker that replaces a secret of `kind`.
pub fn marker(kind: SecretKind) -> String {
    format!("[redacted: {}]", kind.label())
}

/// Replaces every secret in `text` with a marker.
pub fn redact(text: &str) -> Redacted {
    let findings = scan(text);
    let mut out = String::with_capacity(text.len());
    let mut kinds = Vec::new();
    let mut at = 0;
    for f in &findings {
        out.push_str(&text[at..f.range.start]);
        out.push_str(&marker(f.kind));
        at = f.range.end;
        if !kinds.contains(&f.kind) {
            kinds.push(f.kind);
        }
    }
    out.push_str(&text[at..]);
    let mut covered = 0;
    let mut whole = !findings.is_empty();
    for f in &findings {
        if !text[covered..f.range.start].trim().is_empty() {
            whole = false;
        }
        covered = f.range.end;
    }
    whole &= text[covered..].trim().is_empty();
    Redacted {
        text: out,
        kinds,
        whole,
    }
}

fn merge(mut found: Vec<Finding>) -> Vec<Finding> {
    found.retain(|f| f.range.start < f.range.end);
    found.sort_by(|a, b| {
        a.range
            .start
            .cmp(&b.range.start)
            .then(b.range.end.cmp(&a.range.end))
    });
    let mut out: Vec<Finding> = Vec::with_capacity(found.len());
    for f in found {
        match out.last_mut() {
            Some(last) if f.range.start < last.range.end => {
                last.range.end = last.range.end.max(f.range.end)
            }
            _ => out.push(f),
        }
    }
    out
}

#[cfg(test)]
mod tests;
