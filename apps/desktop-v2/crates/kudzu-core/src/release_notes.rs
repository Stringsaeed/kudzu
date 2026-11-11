//! Release-note clean-up for the update toast and the About pane.
//!
//! release-please notes are Markdown with commit links; this reduces them to headings
//! ("Features") and bullet items without scopes' bold markers or commit hashes.

/// One section of cleaned release notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotesSection {
    pub heading: Option<String>,
    pub items: Vec<String>,
}

/// Parse Markdown release notes into headings and cleaned bullet items.
pub fn parse(md: Option<&str>) -> Vec<NotesSection> {
    let Some(md) = md else {
        return Vec::new();
    };
    let mut sections = Vec::new();
    let mut cur = NotesSection {
        heading: None,
        items: Vec::new(),
    };
    for raw in md.split('\n') {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(heading) = markdown_heading(line) {
            if is_version_title(heading) {
                continue;
            }
            if !cur.items.is_empty() || cur.heading.is_some() {
                sections.push(cur);
            }
            cur = NotesSection {
                heading: Some(heading.to_string()),
                items: Vec::new(),
            };
            continue;
        }
        let item = strip_bullet(line);
        let clean = strip_bold(&replace_md_links(&strip_commit_links(item)))
            .trim()
            .to_string();
        if !clean.is_empty() {
            cur.items.push(clean);
        }
    }
    if !cur.items.is_empty() || cur.heading.is_some() {
        sections.push(cur);
    }
    sections
}

fn markdown_heading(line: &str) -> Option<&str> {
    if !line.starts_with('#') {
        return None;
    }
    let hashes = line.find(|c: char| c != '#').unwrap_or(line.len());
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let after = &line[hashes..];
    let rest = after.trim_start();
    if rest.len() == after.len() {
        return None;
    }
    Some(rest)
}

fn is_version_title(heading: &str) -> bool {
    let s = heading.strip_prefix('[').unwrap_or(heading);
    let Some(rest) = take_digits(s) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix('.') else {
        return false;
    };
    let Some(rest) = take_digits(rest) else {
        return false;
    };
    let Some(rest) = rest.strip_prefix('.') else {
        return false;
    };
    take_digits(rest).is_some()
}

fn take_digits(s: &str) -> Option<&str> {
    let n = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    (n > 0).then_some(&s[n..])
}

fn strip_bullet(line: &str) -> &str {
    line.strip_prefix("* ")
        .or_else(|| line.strip_prefix("- "))
        .unwrap_or(line)
}

/// `\s*\(\[[0-9a-f]{7,}\]\([^)]*\)\)`
fn strip_commit_links(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut i = 0;
    let mut out = String::new();
    while i < bytes.len() {
        if let Some(end) = commit_link_at(bytes, i) {
            i = end;
            continue;
        }
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn commit_link_at(bytes: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
        j += 1;
    }
    if j + 2 > bytes.len() || bytes[j] != b'(' || bytes[j + 1] != b'[' {
        return None;
    }
    j += 2;
    let hex_start = j;
    while j < bytes.len() && is_lower_hex(bytes[j]) {
        j += 1;
    }
    if j - hex_start < 7 {
        return None;
    }
    if j + 2 > bytes.len() || bytes[j] != b']' || bytes[j + 1] != b'(' {
        return None;
    }
    j += 2;
    while j < bytes.len() && bytes[j] != b')' {
        j += 1;
    }
    if j + 1 >= bytes.len() || bytes[j] != b')' || bytes[j + 1] != b')' {
        return None;
    }
    Some(j + 2)
}

fn is_lower_hex(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f')
}

/// `\[([^\]]+)\]\([^)]*\)` → capture 1
fn replace_md_links(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut out = String::new();
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some((text, end)) = md_link_at(s, i) {
                out.push_str(text);
                i = end;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn md_link_at(s: &str, i: usize) -> Option<(&str, usize)> {
    let rest = &s[i + 1..];
    let close = rest.find(']')?;
    if close == 0 {
        return None;
    }
    let after = &rest[close + 1..];
    let after_bytes = after.as_bytes();
    if after_bytes.first().copied() != Some(b'(') {
        return None;
    }
    let url = &after[1..];
    let url_end = url.find(')')?;
    let text = &rest[..close];
    let end = i + 1 + close + 1 + 1 + url_end + 1;
    Some((text, end))
}

/// `\*\*([^*]+)\*\*` → capture 1
fn strip_bold(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("**") {
        let after = &rest[start + 2..];
        if let Some(end) = after.find("**") {
            if end > 0 && !after[..end].contains('*') {
                out.push_str(&rest[..start]);
                out.push_str(&after[..end]);
                rest = &after[end + 2..];
                continue;
            }
        }
        out.push_str(&rest[..start + 2]);
        rest = &rest[start + 2..];
    }
    out.push_str(rest);
    out
}
