//! Known formats: PEM private keys, JWTs, SendGrid keys, vendor-prefixed tokens, and long
//! random-looking runs.

use std::ops::Range;

use super::shape::looks_random;
use super::{Finding, SecretKind};

// MARK: Known formats

pub(super) fn private_keys(text: &str, out: &mut Vec<Finding>) {
    const BEGIN: &str = "-----BEGIN ";
    let mut from = 0;
    while let Some(i) = text[from..].find(BEGIN).map(|i| i + from) {
        let header_end = text[i + BEGIN.len()..]
            .find("-----")
            .map(|j| i + BEGIN.len() + j + 5);
        let Some(header_end) = header_end else { break };
        let label = &text[i + BEGIN.len()..header_end - 5];
        if label.ends_with("PRIVATE KEY") || label == "PGP PRIVATE KEY BLOCK" {
            let end_marker = format!("-----END {label}-----");
            let end = text[header_end..]
                .find(&end_marker)
                .map(|j| header_end + j + end_marker.len())
                .unwrap_or(text.len());
            out.push(Finding {
                range: i..end,
                kind: SecretKind::PrivateKey,
            });
            from = end;
        } else {
            from = header_end;
        }
    }
}

pub(super) fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

pub(super) fn jwts(text: &str, out: &mut Vec<Finding>) {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(i) = text[from..].find("eyJ").map(|i| i + from) {
        from = i + 3;
        if i > 0 && is_word_byte(bytes[i - 1]) {
            continue;
        }
        let end = i + bytes[i..]
            .iter()
            .take_while(|&&b| is_word_byte(b) || b == b'.' || b == b'=')
            .count();
        let token = &text[i..end];
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() >= 3
            && parts[0].len() >= 10
            && parts[1].starts_with("eyJ")
            && parts[1].len() >= 4
        {
            out.push(Finding {
                range: i..end,
                kind: SecretKind::Token,
            });
            from = end;
        }
    }
}

pub(super) fn sendgrid(text: &str, out: &mut Vec<Finding>) {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(i) = text[from..].find("SG.").map(|i| i + from) {
        from = i + 3;
        if i > 0 && is_word_byte(bytes[i - 1]) {
            continue;
        }
        let a = bytes[i + 3..]
            .iter()
            .take_while(|&&b| is_word_byte(b))
            .count();
        let dot = i + 3 + a;
        if a != 22 || bytes.get(dot) != Some(&b'.') {
            continue;
        }
        let b = bytes[dot + 1..]
            .iter()
            .take_while(|&&b| is_word_byte(b))
            .count();
        if b == 43 {
            out.push(Finding {
                range: i..dot + 1 + b,
                kind: SecretKind::ApiKey,
            });
        }
    }
}

/// Vendor prefixes and the shortest token (prefix included) that counts.
const PREFIXES: [(&str, usize); 37] = [
    ("sk-ant-", 30),
    ("sk-proj-", 30),
    ("sk-svcacct-", 30),
    ("sk-or-v1-", 30),
    ("sk_live_", 20),
    ("sk_test_", 20),
    ("rk_live_", 20),
    ("rk_test_", 20),
    ("whsec_", 20),
    ("ghp_", 30),
    ("gho_", 30),
    ("ghu_", 30),
    ("ghs_", 30),
    ("ghr_", 30),
    ("github_pat_", 40),
    ("glpat-", 24),
    ("glptt-", 24),
    ("xoxb-", 20),
    ("xoxp-", 20),
    ("xoxa-", 20),
    ("xoxr-", 20),
    ("xoxs-", 20),
    ("xapp-", 20),
    ("npm_", 30),
    ("pypi-", 50),
    ("shpat_", 30),
    ("shpss_", 30),
    ("shpca_", 30),
    ("dop_v1_", 40),
    ("doo_v1_", 40),
    ("hf_", 30),
    ("lin_api_", 30),
    ("figd_", 30),
    ("dckr_pat_", 30),
    ("gsk_", 30),
    ("xai-", 30),
    ("sbp_", 30),
];

/// Characters that make up base64 and similar encodings. Padding (`=`) is left out so
/// `KEY=value` stays two runs; the padding of a real key doesn't change its verdict.
pub(super) fn is_b64_byte(b: u8) -> bool {
    is_word_byte(b) || b == b'+' || b == b'/'
}

fn runs(text: &str, keep: fn(u8) -> bool) -> impl Iterator<Item = Range<usize>> + '_ {
    let bytes = text.as_bytes();
    let mut i = 0;
    std::iter::from_fn(move || {
        while i < bytes.len() && !keep(bytes[i]) {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let start = i;
        while i < bytes.len() && keep(bytes[i]) {
            i += 1;
        }
        Some(start..i)
    })
}

pub(super) fn words(text: &str, out: &mut Vec<Finding>) {
    for r in runs(text, is_word_byte) {
        let word = &text[r.clone()];
        if vendor_key(word) {
            out.push(Finding {
                range: r,
                kind: SecretKind::ApiKey,
            });
        }
    }
    for r in runs(text, is_b64_byte) {
        // Trim padding and path separators that belong to the surroundings.
        let raw = &text[r.clone()];
        let lead = raw.len() - raw.trim_start_matches(['/', '+', '-']).len();
        let trail = raw.len() - raw.trim_end_matches(['/', '-']).len();
        if lead + trail >= raw.len() {
            continue;
        }
        let range = r.start + lead..r.end - trail;
        if looks_random(&text[range.clone()]) {
            out.push(Finding {
                range,
                kind: SecretKind::ApiKey,
            });
        }
    }
}

fn vendor_key(word: &str) -> bool {
    // AWS access key ids and Google API keys have fixed shapes.
    let aws = ["AKIA", "ASIA", "ABIA", "ACCA"]
        .iter()
        .any(|p| word.starts_with(p))
        && word.len() == 20
        && word
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    let google = word.starts_with("AIza") && word.len() == 39;
    if aws || google {
        return true;
    }
    PREFIXES.iter().any(|(prefix, min)| {
        word.len() >= *min && word.starts_with(prefix) && {
            let rest = &word[prefix.len()..];
            rest.bytes().any(|b| b.is_ascii_digit())
                && rest.bytes().any(|b| b.is_ascii_alphabetic())
        }
    })
}
