//! Labels and URLs: values after `password=`-style keys, credentials inside URLs, webhook
//! secrets, and `Authorization` header values.

use std::ops::Range;

use super::formats::is_b64_byte;
use super::{Finding, SecretKind};

// MARK: Labels and URLs

pub(super) fn url_credentials(text: &str, out: &mut Vec<Finding>) {
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

pub(super) fn authorization(text: &str, out: &mut Vec<Finding>) {
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

pub(super) fn labelled(text: &str, out: &mut Vec<Finding>) {
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
