//! Update notes (`lib/updates.ts`): release-please Markdown reduced to readable lines.

use super::js;

/// Public releases page (changelog and manual downloads).
pub const RELEASES_URL: &str = "https://github.com/Stringsaeed/kudzu/releases";

/// A notes section: a heading ("Features") and its items.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NoteSection {
    pub heading: Option<String>,
    pub items: Vec<String>,
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `/^#{1,6}\s+(.*)$/` → the heading text.
fn heading(line: &[char]) -> Option<String> {
    let hashes = line.iter().take_while(|&&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let spaces = line[hashes..].iter().take_while(|&&c| js::is_space(c)).count();
    let rest = &line[hashes + spaces..];
    (spaces > 0 && !rest.iter().copied().any(is_line_terminator)).then(|| rest.iter().collect())
}

/// `/^\[?\d+\.\d+\.\d+/`: the "## [0.2.0](compare…) (date)" title line.
fn is_version_title(h: &str) -> bool {
    let s = h.strip_prefix('[').unwrap_or(h);
    let mut parts = s.splitn(3, '.');
    let digits = |p: Option<&str>, whole: bool| {
        p.is_some_and(|p| {
            let n = p.bytes().take_while(u8::is_ascii_digit).count();
            n > 0 && (!whole || n == p.len())
        })
    };
    digits(parts.next(), true) && digits(parts.next(), true) && digits(parts.next(), false)
}

/// Global replace with a matcher tried at each position (leftmost, non-overlapping).
fn replace_all(s: &str, try_at: impl Fn(&[char], usize) -> Option<(usize, String)>) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        match try_at(&chars, i) {
            Some((end, replacement)) => {
                out.push_str(&replacement);
                i = end;
            }
            None => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

/// `[^c]*` from `i`, then `c`: the index after `c`.
fn until(chars: &[char], i: usize, c: char) -> Option<usize> {
    chars[i..].iter().position(|&x| x == c).map(|p| i + p + 1)
}

/// `\s*\(\[[0-9a-f]{7,}\]\([^)]*\)\)` → "" (commit links).
fn commit_link(chars: &[char], i: usize) -> Option<(usize, String)> {
    let mut j = i + chars[i..].iter().take_while(|&&c| js::is_space(c)).count();
    if chars.get(j) != Some(&'(') || chars.get(j + 1) != Some(&'[') {
        return None;
    }
    j += 2;
    let hex = chars[j..].iter().take_while(|c| matches!(c, '0'..='9' | 'a'..='f')).count();
    if hex < 7 {
        return None;
    }
    j += hex;
    if chars.get(j) != Some(&']') || chars.get(j + 1) != Some(&'(') {
        return None;
    }
    let j = until(chars, j + 2, ')')?;
    (chars.get(j) == Some(&')')).then(|| (j + 1, String::new()))
}

/// `\[([^\]]+)\]\([^)]*\)` → `$1` (links to their text).
fn link(chars: &[char], i: usize) -> Option<(usize, String)> {
    if chars[i] != '[' {
        return None;
    }
    let close = until(chars, i + 1, ']')?;
    if close - 1 == i + 1 || chars.get(close) != Some(&'(') {
        return None;
    }
    let end = until(chars, close + 1, ')')?;
    Some((end, chars[i + 1..close - 1].iter().collect()))
}

/// `\*\*([^*]+)\*\*` → `$1` (bold scopes).
fn bold(chars: &[char], i: usize) -> Option<(usize, String)> {
    if chars[i] != '*' || chars.get(i + 1) != Some(&'*') {
        return None;
    }
    let star = until(chars, i + 2, '*')? - 1;
    if star == i + 2 || chars.get(star + 1) != Some(&'*') {
        return None;
    }
    Some((star + 2, chars[i + 2..star].iter().collect()))
}

/// release-please notes reduced to sections: headings ("Features"), and bullet items without
/// bold scope markers, links or commit hashes. The version title line is dropped.
pub fn parse_notes(md: Option<&str>) -> Vec<NoteSection> {
    let Some(md) = md.filter(|m| !m.is_empty()) else {
        return Vec::new();
    };
    let mut sections = Vec::new();
    let mut cur = NoteSection::default();
    for raw in md.split('\n') {
        let line = js::trim(raw);
        if line.is_empty() {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        if let Some(h) = heading(&chars) {
            if is_version_title(&h) {
                continue;
            }
            if !cur.items.is_empty() || cur.heading.is_some() {
                sections.push(std::mem::take(&mut cur));
            }
            cur = NoteSection { heading: Some(h), items: Vec::new() };
            continue;
        }
        let item = match line.strip_prefix(['*', '-']) {
            Some(rest) if rest.starts_with(js::is_space) => rest.trim_start_matches(js::is_space),
            _ => line,
        };
        let clean = replace_all(&replace_all(&replace_all(item, commit_link), link), bold);
        let clean = js::trim(&clean);
        if !clean.is_empty() {
            cur.items.push(clean.to_string());
        }
    }
    if !cur.items.is_empty() || cur.heading.is_some() {
        sections.push(cur);
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(heading: Option<&str>, items: &[&str]) -> NoteSection {
        NoteSection { heading: heading.map(str::to_string), items: items.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn parses_release_please_notes() {
        let md = "## [0.2.0](https://github.com/x/y/compare/v0.1.0...v0.2.0) (2026-01-01)\n\n\n### Features\n\n* **desktop:** import from Raycast ([b8b1f36](https://github.com/x/y/commit/b8b1f36d)) ([#12](https://github.com/x/y/issues/12))\n* plain item\n\n### Bug Fixes\n\n- fix **a** and **b** ([abcdef0](u))\n";
        assert_eq!(parse_notes(Some(md)), [
            section(Some("Features"), &["desktop: import from Raycast (#12)", "plain item"]),
            section(Some("Bug Fixes"), &["fix a and b"]),
        ]);
    }

    #[test]
    fn edge_cases() {
        assert!(parse_notes(None).is_empty());
        assert!(parse_notes(Some("")).is_empty());
        assert_eq!(parse_notes(Some("intro line\n# Heading")), [section(None, &["intro line"]), section(Some("Heading"), &[])]);
        // Not headings: seven hashes, or no space.
        assert_eq!(parse_notes(Some("####### x\n#y")), [section(None, &["####### x", "#y"])]);
        // Short hashes are links, empty link text and empty bold stay.
        assert_eq!(parse_notes(Some("* a ([abc12](u)) [](v) ****")), [section(None, &["a (abc12) [](v) ****"])]);
        assert_eq!(parse_notes(Some("*no space")), [section(None, &["*no space"])]);
        assert_eq!(parse_notes(Some("## 1.2.3 beta\n### v1.2")), [section(Some("v1.2"), &[])]);
    }
}
