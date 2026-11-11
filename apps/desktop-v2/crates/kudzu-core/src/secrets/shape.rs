//! Shape: character classes and entropy, generated-looking runs, and lone words that look
//! like passwords.

use super::{Finding, SecretKind};

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
pub(super) fn looks_random(s: &str) -> bool {
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
pub(super) fn lone_word(text: &str, out: &mut Vec<Finding>) {
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
