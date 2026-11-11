//! TextExpander `%macro%` text → Kudzu template source: dates with date math, fill-ins,
//! nested snippets, keys and the clipboard.

use std::collections::HashMap;

use uuid::Uuid;

use super::super::espanso::tags;
use super::super::strftime::{escape_icu_literal, map_specifier};
use super::super::template_out::TemplateOut;

/// XML and spreadsheets both write line breaks as `\r\n`; Kudzu templates use `\n`.
pub(super) fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn first_line(text: &str) -> String {
    let line: String = text
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .chars()
        .take(40)
        .collect();
    if line.is_empty() {
        "Untitled".into()
    } else {
        line
    }
}

// MARK: - Macro conversion

fn starts_with(chars: &[char], at: usize, pattern: &str) -> bool {
    pattern
        .chars()
        .enumerate()
        .all(|(k, c)| chars.get(at + k) == Some(&c))
}

/// The index of the next `%` at or after `from`.
fn next_percent(chars: &[char], from: usize) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == '%')
}

/// ICU token and length of the date specifier at `at` (`%Y`, `%1d`, …).
fn date_spec(chars: &[char], at: usize) -> Option<(&'static str, usize)> {
    if chars.get(at) != Some(&'%') {
        return None;
    }
    let next = *chars.get(at + 1)?;
    if next == '1' {
        // `%1m`, `%1d`, `%1H` and `%1I` skip the leading zero.
        let token = match chars.get(at + 2)? {
            'm' => "M",
            'd' => "d",
            'H' => "H",
            'I' => "h",
            _ => return None,
        };
        return Some((token, 3));
    }
    map_specifier(next).map(|t| (t, 2))
}

/// `+1D`, `-2M`, `+3h` after `%@`: a Kudzu offset and the index after it.
fn date_math(chars: &[char], from: usize) -> Option<(String, usize)> {
    let sign = match chars.get(from)? {
        '+' => '+',
        '-' => '-',
        _ => return None,
    };
    let mut i = from + 1;
    let start = i;
    while chars.get(i).is_some_and(char::is_ascii_digit) {
        i += 1;
    }
    if i == start {
        return None;
    }
    let digits: String = chars[start..i].iter().collect();
    let unit = match chars.get(i)? {
        'Y' => "y",
        'M' => "m",
        'W' => "w",
        'D' => "d",
        'h' => "h",
        'm' => "min",
        _ => return None,
    };
    Some((format!("{sign}{digits}{unit}"), i + 1))
}

/// A run of date specifiers joined by plain separators (`%m/%d/%Y`) as one ICU pattern.
fn date_run(chars: &[char], from: usize) -> Option<(String, usize)> {
    let (first, len) = date_spec(chars, from)?;
    let mut icu = first.to_string();
    let mut end = from + len;
    loop {
        let mut k = end;
        let mut separator = String::new();
        while separator.chars().count() < 2
            && chars
                .get(k)
                .is_some_and(|c| matches!(c, ' ' | '-' | '/' | ':' | ',' | '.'))
        {
            separator.push(chars[k]);
            k += 1;
        }
        match date_spec(chars, k) {
            Some((token, len)) => {
                icu.push_str(&escape_icu_literal(&separator));
                icu.push_str(token);
                end = k + len;
            }
            None => break,
        }
    }
    Some((icu, end))
}

pub(super) fn convert(
    raw: &str,
    abbreviations: &HashMap<String, Uuid>,
    problems: &mut Vec<String>,
) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = TemplateOut::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            out.push_char(chars[i]);
            i += 1;
            continue;
        }
        if chars.get(i + 1) == Some(&'|') {
            out.tag(&tags::cursor_tag());
            i += 2;
            continue;
        }
        if starts_with(&chars, i, "%clipboard")
            && !chars
                .get(i + 10)
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
        {
            out.tag(&tags::clipboard_tag(false));
            i += 10;
            continue;
        }
        if starts_with(&chars, i, "%snippet:") {
            if let Some(close) = next_percent(&chars, i + 9) {
                let name: String = chars[i + 9..close].iter().collect();
                let source: String = chars[i..=close].iter().collect();
                match abbreviations.get(name.trim()) {
                    Some(&id) => out.tag(&tags::snippet_tag(id)),
                    None => {
                        problems.push(format!(
                            "{source} refers to a snippet that isn't in this export; kept as text."
                        ));
                        out.text(&source);
                    }
                }
                i = close + 1;
                continue;
            }
        }
        if starts_with(&chars, i, "%fillpart:") {
            // An optional section: `%fillpart:name=…%content%fillpartend%`.
            if let Some(close) = next_percent(&chars, i + 10) {
                let end_marker = "%fillpartend%";
                let section_start = close + 1;
                let section_end =
                    (section_start..chars.len()).find(|&k| starts_with(&chars, k, end_marker));
                problems.push("Optional sections (%fillpart…%) aren't supported; the section is always included.".into());
                match section_end {
                    Some(end) => {
                        let inner: String = chars[section_start..end].iter().collect();
                        out.tag(&convert(&inner, abbreviations, problems));
                        i = end + end_marker.len();
                    }
                    None => i = section_start,
                }
                continue;
            }
        }
        if starts_with(&chars, i, "%fillpartend%") {
            i += 13;
            continue;
        }
        if let Some(kind) = ["filltext", "fillarea", "fillpopup"]
            .into_iter()
            .find(|k| starts_with(&chars, i + 1, k) && chars.get(i + 1 + k.len()) == Some(&':'))
        {
            if let Some(close) = next_percent(&chars, i + 2 + kind.len()) {
                let body: String = chars[i + 2 + kind.len()..close].iter().collect();
                let source: String = chars[i..=close].iter().collect();
                match fill_tag(kind, &body) {
                    Some(tag) => out.tag(&tag),
                    None => {
                        problems.push(format!("{source} has no name; kept as text."));
                        out.text(&source);
                    }
                }
                i = close + 1;
                continue;
            }
        }
        if starts_with(&chars, i, "%key:") {
            if let Some(close) = next_percent(&chars, i + 5) {
                let name: String = chars[i + 5..close].iter().collect();
                // TextExpander's Enter and Return both become Return.
                if let Some(key) = crate::template::Key::parse(&name.trim().to_lowercase()) {
                    out.tag(&tags::key_tag(key, 1));
                    i = close + 1;
                    continue;
                }
            }
        }
        if let Some(name) = ["key", "calc", "mouse"]
            .into_iter()
            .find(|k| starts_with(&chars, i + 1, k) && chars.get(i + 1 + k.len()) == Some(&':'))
        {
            if let Some(close) = next_percent(&chars, i + 2 + name.len()) {
                let source: String = chars[i..=close].iter().collect();
                problems.push(format!(
                    "%{name}:…% macros aren't supported; {source} kept as text."
                ));
                out.text(&source);
                i = close + 1;
                continue;
            }
        }
        if chars.get(i + 1) == Some(&'@') {
            // Date math: `%@+1D` shifts the date macros that follow it.
            if let Some((offset, after)) = date_math(&chars, i + 2) {
                if let Some((icu, end)) = date_run(&chars, after) {
                    out.tag(&tags::date_tag(Some(&icu), Some(&offset), None));
                    i = end;
                    continue;
                }
            }
            problems
                .push("Date math (%@…) isn't supported here; the macro was kept as text.".into());
        }
        if let Some((icu, end)) = date_run(&chars, i) {
            out.tag(&tags::date_tag(Some(&icu), None, None));
            i = end;
            continue;
        }
        out.push_char('%');
        i += 1;
    }
    out.finish()
}

/// `name=Customer:default=Jo` (and `:`-separated popup items) → a field or choice tag.
fn fill_tag(kind: &str, body: &str) -> Option<String> {
    let mut name = None;
    let mut default = None;
    let mut options: Vec<String> = Vec::new();
    for part in body.split(':') {
        if let Some(v) = part.strip_prefix("name=") {
            name = Some(v.trim().to_string());
        } else if let Some(v) = part.strip_prefix("default=") {
            default = Some(v.to_string());
        } else if ["width=", "height=", "scrollbar=", "hscroll="]
            .iter()
            .any(|k| part.starts_with(k))
        {
            // Layout hints for TextExpander's own form; nothing to carry over.
        } else if kind == "fillpopup" && !part.is_empty() {
            options.push(part.to_string());
        }
    }
    let name = name.filter(|n| !n.is_empty())?;
    if kind == "fillpopup" && !options.is_empty() {
        if let Some(d) = default
            .as_deref()
            .filter(|d| !options.iter().any(|o| o == d))
        {
            options.insert(0, d.to_string());
        }
        return Some(tags::choice_tag(&name, &options, default.as_deref()));
    }
    Some(tags::field_tag(
        &name,
        default.as_deref(),
        kind == "fillarea",
    ))
}
