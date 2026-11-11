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

use std::ops::Range;

use serde::{Deserialize, Serialize};

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

// MARK: Known formats

fn private_keys(text: &str, out: &mut Vec<Finding>) {
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

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn jwts(text: &str, out: &mut Vec<Finding>) {
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

fn sendgrid(text: &str, out: &mut Vec<Finding>) {
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
fn is_b64_byte(b: u8) -> bool {
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

fn words(text: &str, out: &mut Vec<Finding>) {
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

// MARK: Shape

#[derive(Default)]
struct Classes {
    lower: usize,
    upper: usize,
    digit: usize,
    symbol: usize,
    /// Symbols other than the ones identifiers and file names use.
    strong_symbol: usize,
}

impl Classes {
    fn of(s: &str) -> Self {
        let mut c = Self::default();
        for ch in s.chars() {
            if ch.is_lowercase() {
                c.lower += 1;
            } else if ch.is_uppercase() {
                c.upper += 1;
            } else if ch.is_numeric() {
                c.digit += 1;
            } else {
                c.symbol += 1;
                if !matches!(ch, '.' | '_' | '-' | '/' | ':') {
                    c.strong_symbol += 1;
                }
            }
        }
        c
    }

    fn count(&self) -> usize {
        [self.lower, self.upper, self.digit, self.symbol]
            .iter()
            .filter(|&&n| n > 0)
            .count()
    }
}

/// Shannon entropy relative to the most a string of this length (over this alphabet) can have.
fn relative_entropy(s: &str, alphabet: f64) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut n = 0usize;
    for ch in s.chars() {
        *counts.entry(ch).or_insert(0usize) += 1;
        n += 1;
    }
    if n < 2 {
        return 0.0;
    }
    let len = n as f64;
    let h: f64 = counts
        .values()
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum();
    h / len.min(alphabet).log2()
}

fn is_hex(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|p| p.len()).eq([8, 4, 4, 4, 12])
        && parts.iter().all(|p| is_hex(p))
}

/// A git commit id: 40 lowercase hex digits. Common on developers' clipboards and not secret.
fn is_commit_id(s: &str) -> bool {
    s.len() == 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A long run of characters that looks generated rather than written.
fn looks_random(s: &str) -> bool {
    let len = s.len();
    if !(20..=512).contains(&len) || is_uuid(s) || is_commit_id(s) {
        return false;
    }
    let c = Classes::of(s);
    if c.digit < 2 || c.lower + c.upper < 2 {
        return false;
    }
    if is_hex(s) {
        return len >= 32 && relative_entropy(s, 16.0) >= 0.85;
    }
    c.lower > 0 && c.upper > 0 && relative_entropy(s, 64.0) >= 0.85
}

/// A lone word that looks like a password: the whole text is one word with mixed classes.
fn lone_word(text: &str, out: &mut Vec<Finding>) {
    // A labelled value or a known format already says which part is secret.
    if !out.is_empty() {
        return;
    }
    let trimmed = text.trim();
    if trimmed.chars().any(char::is_whitespace) {
        return;
    }
    let n = trimmed.chars().count();
    if !(8..=128).contains(&n) || !looks_like_password(trimmed) {
        return;
    }
    let start = text.len() - text.trim_start().len();
    out.push(Finding {
        range: start..start + trimmed.len(),
        kind: SecretKind::Password,
    });
}

fn looks_like_password(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    let is_url = lower.contains("://") || lower.starts_with("www.");
    let is_path = s.starts_with('/')
        || s.starts_with('~')
        || s.starts_with("./")
        || s.starts_with("../")
        || s.contains('\\');
    let is_email = s.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty()
            && domain.contains('.')
            && domain
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    });
    let is_number = s
        .chars()
        .all(|c| c.is_ascii_digit() || ",.+-%$€£".contains(c));
    if is_url
        || is_path
        || is_email
        || is_number
        || is_uuid(s)
        || is_commit_id(s)
        || s.starts_with('#')
    {
        return false;
    }
    let c = Classes::of(s);
    if c.strong_symbol > 0 && c.count() >= 3 && c.lower + c.upper > 0 {
        return true;
    }
    let len = s.chars().count();
    if c.lower > 0 && c.upper > 0 && c.digit > 0 && (len >= 12 || (c.digit >= 2 && c.upper >= 2)) {
        return true;
    }
    // Lowercase letters and digits mixed throughout (not `release2024notes`).
    let alnum: Vec<char> = s.chars().filter(char::is_ascii_alphanumeric).collect();
    let switches = alnum
        .windows(2)
        .filter(|w| w[0].is_ascii_digit() != w[1].is_ascii_digit())
        .count();
    if len >= 12
        && c.digit >= 3
        && c.lower + c.upper > 0
        && switches >= 6
        && relative_entropy(s, 36.0) >= 0.85
    {
        return true;
    }
    looks_random(s)
}

// MARK: Labels and URLs

fn url_credentials(text: &str, out: &mut Vec<Finding>) {
    let bytes = text.as_bytes();
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = text[from..].find("://").map(|i| i + from) {
        from = i + 3;
        let scheme_len = bytes[..i]
            .iter()
            .rev()
            .take_while(|b| b.is_ascii_alphanumeric() || **b == b'+' || **b == b'.' || **b == b'-')
            .count();
        if scheme_len == 0 {
            continue;
        }
        let rest = &text[i + 3..];
        let url_len = rest
            .find(|c: char| c.is_whitespace() || "\"'<>`)]".contains(c))
            .unwrap_or(rest.len());
        let authority_len = rest[..url_len].find(['/', '?', '#']).unwrap_or(url_len);
        let authority = &rest[..authority_len];
        if let Some(at) = authority.rfind('@') {
            if let Some(colon) = authority[..at].find(':') {
                let start = i + 3 + colon + 1;
                let end = i + 3 + at;
                if end > start {
                    out.push(Finding {
                        range: start..end,
                        kind: SecretKind::Credential,
                    });
                }
            }
        }
        // Webhook URLs carry their secret in the path.
        let host = &lower[i + 3..i + 3 + authority_len];
        let path = &lower[i + 3 + authority_len..i + 3 + url_len];
        let webhook = (host.ends_with("hooks.slack.com") && path.starts_with("/services/"))
            || ((host.ends_with("discord.com") || host.ends_with("discordapp.com"))
                && path.starts_with("/api/webhooks/"));
        if webhook {
            let start = i + 3 + authority_len + path[1..].find('/').map(|j| j + 2).unwrap_or(1);
            out.push(Finding {
                range: start..i + 3 + url_len,
                kind: SecretKind::Credential,
            });
        }
        from = i + 3 + url_len.max(1).min(rest.len());
    }
}

fn authorization(text: &str, out: &mut Vec<Finding>) {
    let lower = text.to_ascii_lowercase();
    for scheme in ["bearer ", "basic ", "token "] {
        let mut from = 0;
        while let Some(i) = lower[from..].find(scheme).map(|i| i + from) {
            from = i + scheme.len();
            if i > 0 && lower.as_bytes()[i - 1].is_ascii_alphanumeric() {
                continue;
            }
            // "token " is only a header scheme right after "Authorization:".
            if scheme == "token " && !lower[..i].trim_end().ends_with("authorization:") {
                continue;
            }
            let start = from + lower[from..].len() - lower[from..].trim_start_matches(' ').len();
            let len = text[start..]
                .bytes()
                .take_while(|&b| is_b64_byte(b) || b == b'.' || b == b'~' || b == b'=')
                .count();
            if len >= 8 {
                out.push(Finding {
                    range: start..start + len,
                    kind: SecretKind::Token,
                });
            }
        }
    }
}

/// Key names whose value is a secret, compared after lowercasing and dropping punctuation.
const SECRET_LABELS: [&str; 10] = [
    "password",
    "passwd",
    "passphrase",
    "secret",
    "token",
    "apikey",
    "accesskey",
    "privatekey",
    "credential",
    "authkey",
];
const PASSWORD_LABELS: [&str; 3] = ["password", "passwd", "passphrase"];
const SHORT_LABELS: [&str; 4] = ["pwd", "pass", "pw", "pin"];

/// Values that stand in for a secret rather than being one: types, placeholders, references.
fn is_placeholder(value: &str) -> bool {
    let v = value.to_ascii_lowercase();
    const WORDS: [&str; 14] = [
        "string",
        "str",
        "null",
        "none",
        "nil",
        "true",
        "false",
        "undefined",
        "required",
        "optional",
        "number",
        "boolean",
        "redacted",
        "hidden",
    ];
    WORDS.contains(&v.as_str())
        || value.starts_with(['$', '%', '{', '<', '(', '[', '*', '&'])
        || value
            .chars()
            .all(|c| c == 'x' || c == 'X' || c == '*' || c == '.' || c == '•')
        || v.starts_with("process.env")
        || v.starts_with("os.environ")
        || v.starts_with("env.")
}

fn labelled(text: &str, out: &mut Vec<Finding>) {
    let bytes = text.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'=' && b != b':' {
            continue;
        }
        let next = bytes.get(i + 1).copied();
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        if b == b':' && (text[i + 1..].starts_with("//") || next == Some(b':') || prev == b':') {
            continue;
        }
        if b == b'=' && (next == Some(b'=') || matches!(prev, b'=' | b'!' | b'<' | b'>')) {
            continue;
        }
        let Some(key) = key_before(text, i) else {
            continue;
        };
        let normalized: String = key
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        let short = key
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        let secret = SECRET_LABELS.iter().any(|l| normalized.contains(l))
            || SHORT_LABELS.contains(&short.as_str());
        if !secret {
            continue;
        }
        let Some(value) = value_after(text, i, next == Some(b'>')) else {
            continue;
        };
        let v = &text[value.clone()];
        if v.chars().count() < 4 || is_placeholder(v) {
            continue;
        }
        // `max_tokens=1024`, `token_count: 3`: counts, not secrets.
        if normalized.contains("token") && v.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let kind = if PASSWORD_LABELS.iter().any(|l| normalized.contains(l))
            || SHORT_LABELS.contains(&short.as_str())
        {
            SecretKind::Password
        } else {
            SecretKind::Credential
        };
        out.push(Finding { range: value, kind });
    }
}

/// The label in front of a separator at `sep`: up to three words, skipping a closing quote.
fn key_before(text: &str, sep: usize) -> Option<&str> {
    let head = text[..sep].trim_end_matches([' ', '\t']);
    let head = head.strip_suffix(['"', '\'']).unwrap_or(head);
    let start = head
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '))
        .last()
        .map(|(i, _)| i)?;
    let key = head[start..].trim();
    let words = key.split(' ').filter(|w| !w.is_empty()).count();
    if key.is_empty() || words > 3 || key.len() > 48 {
        // Take only the word nearest the separator.
        let last = key.rsplit(' ').next()?;
        return (!last.is_empty()).then_some(last);
    }
    Some(key)
}

fn value_after(text: &str, sep: usize, arrow: bool) -> Option<Range<usize>> {
    let mut i = sep + 1 + usize::from(arrow);
    let bytes = text.as_bytes();
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let quote = bytes
        .get(i)
        .copied()
        .filter(|&b| b == b'"' || b == b'\'' || b == b'`');
    if let Some(q) = quote {
        let start = i + 1;
        let mut j = start;
        while j < bytes.len() && bytes[j] != q && bytes[j] != b'\n' {
            j += if bytes[j] == b'\\' { 2 } else { 1 };
        }
        let end = j.min(bytes.len());
        return (end > start).then_some(start..end);
    }
    let start = i;
    let len = text[start..]
        .find(|c: char| c.is_whitespace() || ",;)}]\"'".contains(c))
        .unwrap_or(text.len() - start);
    (len > 0).then_some(start..start + len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<SecretKind> {
        redact(text).kinds
    }

    fn redacted(text: &str) -> String {
        redact(text).text
    }

    #[test]
    fn ordinary_text_is_untouched() {
        for text in [
            "Best regards,\nSaeed",
            "Meeting moved to 3pm tomorrow, see https://example.com/agenda?id=42",
            "hunter",
            "getElementById",
            "README.md",
            "config.v2.json",
            "node_modules",
            "user@example.com",
            "/Users/saeed/Projects/App2/src/Main.tsx",
            "#FF00aa",
            "$1,000,000",
            "550e8400-e29b-41d4-a716-446655440000",
            "9fceb02d0ae598e95dc970b74767f19372d61af8",
            "let max_tokens = 1024;",
            "password: string",
            "const password = process.env.PASSWORD",
            "Password: ********",
            "iPhone15",
            "The author: Jane Doe",
            "Use Bearer auth",
        ] {
            assert!(
                kinds(text).is_empty(),
                "flagged {text:?} as {:?}",
                kinds(text)
            );
        }
    }

    #[test]
    fn vendor_keys() {
        for key in [
            concat!("ghp", "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx"),
            concat!(
                "github",
                "_pat_11ABCDEFG0abcdefghijkl_mnopqrstuvwxyz0123456789ABCDEF"
            ),
            concat!("sk-ant", "-api03-abcDEF123456ghiJKL789012mnoPQR345678"),
            concat!("sk-proj", "-Abc123Def456Ghi789Jkl012Mno345"),
            concat!("sk_live", "_4eC39HqLyjWDarjtT1zdp7dc"),
            concat!(
                "xoxb",
                "-123456789012-1234567890123-AbCdEfGhIjKlMnOpQrStUvWx"
            ),
            "AKIAIOSFODNN7EXAMPLE",
            concat!("AIza", "SyDdI0hCZtE6vySjMm-WEfRq3CPzqKqqsHI"),
            "glpat-xxxxxxxxxxxxxxxx2xxx",
            concat!("npm", "_abcdefghijklmnopqrstuvwxyz0123456789"),
        ] {
            let text = format!("my key is {key} ok");
            assert_eq!(redacted(&text), "my key is [redacted: API key] ok", "{key}");
        }
    }

    #[test]
    fn private_key_block() {
        let text = "id:\n-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAA\n-----END OPENSSH PRIVATE KEY-----\nthanks";
        assert_eq!(redacted(text), "id:\n[redacted: private key]\nthanks");
        // A public key or certificate is fine.
        assert!(kinds("-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----").is_empty());
        // An unterminated block is withheld to the end.
        assert_eq!(
            redacted("-----BEGIN RSA PRIVATE KEY-----\nMIIE"),
            "[redacted: private key]"
        );
    }

    #[test]
    fn jwt_and_authorization() {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        assert_eq!(
            redacted(&format!("token {jwt}")),
            "token [redacted: access token]"
        );
        assert_eq!(
            redacted("curl -H 'Authorization: Bearer abc.def-123_xyz'"),
            "curl -H 'Authorization: Bearer [redacted: access token]'"
        );
        assert_eq!(
            redacted("Authorization: token deadbeefcafe"),
            "Authorization: token [redacted: access token]"
        );
    }

    #[test]
    fn labelled_values() {
        assert_eq!(
            redacted("DB_PASSWORD=correct-horse"),
            "DB_PASSWORD=[redacted: password]"
        );
        assert_eq!(
            redacted(r#"{"api_key": "abcd1234", "name": "x"}"#),
            r#"{"api_key": "[redacted: secret]", "name": "x"}"#
        );
        assert_eq!(
            redacted("export GITHUB_TOKEN=abc123xyz"),
            "export GITHUB_TOKEN=[redacted: secret]"
        );
        assert_eq!(
            redacted("Wi-Fi password: purple monkey"),
            "Wi-Fi password: [redacted: password] monkey"
        );
        assert_eq!(
            redacted("The password is: Summer2024"),
            "The password is: [redacted: password]"
        );
        assert_eq!(redacted("API Key: q8w7e6r5"), "API Key: [redacted: secret]");
        assert_eq!(
            redacted("--password=hunter22 --user bob"),
            "--password=[redacted: password] --user bob"
        );
        assert_eq!(redacted("pin: 4821"), "pin: [redacted: password]");
        assert_eq!(
            redacted("client_secret => 'zz9plural'"),
            "client_secret => '[redacted: secret]'"
        );
    }

    #[test]
    fn url_credentials_and_webhooks() {
        assert_eq!(
            redacted("postgres://admin:s3cr3t@db.local:5432/app"),
            "postgres://admin:[redacted: secret]@db.local:5432/app"
        );
        assert!(kinds("https://user@example.com/path").is_empty());
        assert_eq!(
            redacted("hook https://hooks.slack.com/services/T000/B000/XXXXXXXX done"),
            "hook https://hooks.slack.com/services/[redacted: secret] done"
        );
    }

    #[test]
    fn lone_passwords() {
        for pw in [
            "P@ssw0rd!",
            "Tr0ub4dor&3",
            "kezwy4-dyxbeR-mosmac",
            "qT7xLm9vR2bN",
            "a8f3k2m9x7q1z5w4",
        ] {
            let r = redact(pw);
            assert!(r.whole, "{pw} not flagged");
            assert_eq!(r.text, "[redacted: password]");
        }
        // Surrounding whitespace keeps it whole.
        assert!(redact("  P@ssw0rd!\n").whole);
        // A generated key alone is whole too.
        assert!(redact("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY").whole);
    }

    #[test]
    fn random_runs_inside_text() {
        assert_eq!(
            redacted("secret is wJalrXUtnFEMI/K7MDENG/bPxRfiCY3XAMPL9KEY ok"),
            "secret is [redacted: API key] ok"
        );
        assert_eq!(
            redacted("hex 7f3a9c2e5b8d1f4a6c0e9b2d5f8a1c4e"),
            "hex [redacted: API key]"
        );
        // Long code identifiers stay.
        assert!(kinds("fn handle_incoming_connection_request(stream: UnixStream) {}").is_empty());
    }

    #[test]
    fn mixed_text_is_not_whole() {
        let r = redact("password=abcd1234 for the staging box");
        assert!(!r.whole);
        assert_eq!(r.kinds, vec![SecretKind::Password]);
        let r = redact(concat!(
            "sk_live",
            "_4eC39HqLyjWDarjtT1zdp7dc\nghp",
            "_wWPw5k4aXcaT4fNP0UcnZwJUVFk6LO0pINUx"
        ));
        assert!(r.whole);
    }

    #[test]
    fn handles_non_ascii() {
        assert!(kinds("Grüße aus Köln — schöne Woche! 🌿").is_empty());
        assert_eq!(
            redacted("Passwort-Hinweis — password: Zürich#2024"),
            "Passwort-Hinweis — password: [redacted: password]"
        );
    }
}
