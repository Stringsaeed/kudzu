//! TextExpander export → Kudzu library.
//!
//! Reads the two formats TextExpander writes: the XML property list behind `.textexpander`
//! group files (`snippetsTE2`), and CSV. Macros (`%clipboard`, `%|`, `%Y-%m-%d`,
//! `%filltext:…%`, `%snippet:…%`) become Kudzu tags. See the TextExpander mapping table in
//! docs/TEMPLATE_SYNTAX.md.

mod csv;
mod plist;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, SubsecRound, Utc};
use uuid::Uuid;

use super::espanso::tags;
use super::strftime::{escape_icu_literal, map_specifier};
use super::template_out::TemplateOut;
use super::espanso::ImportWarning;
use super::{ImportResult, InterchangeError};
use crate::types::{iso8601, CaseMode, FireMode, GroupRecord, KudzuLibrary, SnippetRecord};

use plist::Value;

/// Reads a TextExpander export (`.textexpander` property list or CSV) from `path`.
pub fn import_file(path: &Path) -> Result<ImportResult, InterchangeError> {
    if path.is_dir() {
        return Err(InterchangeError::InvalidSource(
            "Choose the exported .textexpander or .csv file, not a folder.".into(),
        ));
    }
    let data = fs::read(path)
        .map_err(|e| InterchangeError::io(format!("Couldn't read {}", path.display()), e))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    import_data(&data, &file_name)
}

/// Converts the bytes of a TextExpander export. `file_name` names the group and labels warnings.
pub fn import_data(data: &[u8], file_name: &str) -> Result<ImportResult, InterchangeError> {
    let data = data.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(data);
    if data.starts_with(b"bplist") {
        return Err(InterchangeError::InvalidSource(
            "This is a binary TextExpander file Kudzu can't read. In TextExpander, choose File ▸ Export and pick CSV instead."
                .into(),
        ));
    }
    if data.starts_with(b"PK\x03\x04") {
        return Err(InterchangeError::InvalidSource(
            "This is a compressed TextExpander file Kudzu can't read. In TextExpander, choose File ▸ Export and pick CSV instead."
                .into(),
        ));
    }
    let text = String::from_utf8_lossy(data);
    let stem = Path::new(file_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "TextExpander".into());

    let mut warnings = Vec::new();
    let head = text.trim_start();
    let is_plist = ["<?xml", "<!DOCTYPE", "<plist"]
        .iter()
        .any(|marker| head.starts_with(marker));
    let groups = if is_plist {
        let root = plist::parse(&text).map_err(|e| {
            InterchangeError::InvalidSource(format!("Couldn't read this TextExpander file: {e}."))
        })?;
        read_plist(&root, &stem, file_name, &mut warnings)?
    } else {
        read_csv(&csv::parse(&text), &stem, file_name, &mut warnings)?
    };
    Ok(build(groups, file_name, warnings))
}

// MARK: - Raw structures

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    Rich,
    Script,
}

struct RawSnippet {
    index: usize,
    abbreviation: String,
    content: String,
    label: Option<String>,
    kind: Kind,
    exact_case: bool,
    created: Option<DateTime<Utc>>,
    modified: Option<DateTime<Utc>>,
}

struct RawGroup {
    name: String,
    snippets: Vec<RawSnippet>,
}

fn warning(file: &str, index: usize, trigger: Option<&str>, message: String) -> ImportWarning {
    ImportWarning {
        file: file.to_string(),
        line: None,
        match_index: Some(index),
        trigger: trigger.filter(|t| !t.is_empty()).map(str::to_string),
        message,
    }
}

fn not_textexpander() -> InterchangeError {
    InterchangeError::InvalidSource(
        "No TextExpander snippets were found in this file. Export a group from TextExpander as .textexpander or CSV."
            .into(),
    )
}

// MARK: - Property list

fn read_plist(
    root: &Value,
    stem: &str,
    file: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Result<Vec<RawGroup>, InterchangeError> {
    let mut groups = Vec::new();
    let mut next_index = 0;
    let mut add = |value: &Value, fallback: &str, warnings: &mut Vec<ImportWarning>| {
        let Some(list) = value.get("snippetsTE2").and_then(Value::as_array) else {
            return;
        };
        let name = value
            .get("groupInfo")
            .and_then(|i| i.get("groupName"))
            .or_else(|| value.get("groupName"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .unwrap_or(fallback)
            .to_string();
        let mut snippets = Vec::new();
        for item in list {
            let index = next_index;
            next_index += 1;
            let abbreviation = item.get("abbreviation").and_then(Value::as_str);
            let content = item.get("plainText").and_then(Value::as_str);
            let (Some(abbreviation), Some(content)) = (abbreviation, content) else {
                warnings.push(warning(
                    file,
                    index,
                    abbreviation,
                    "Snippet is missing its abbreviation or text and was skipped.".into(),
                ));
                continue;
            };
            let kind = match item.get("snippetType").and_then(Value::as_int) {
                Some(1) => Kind::Rich,
                Some(n) if n >= 2 => Kind::Script,
                _ => Kind::Plain,
            };
            let date = |key: &str| {
                item.get(key)
                    .and_then(Value::as_date)
                    .and_then(iso8601::parse)
            };
            snippets.push(RawSnippet {
                index,
                abbreviation: abbreviation.to_string(),
                content: normalize_newlines(content),
                label: item
                    .get("label")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                kind,
                exact_case: item.get("abbreviationMode").and_then(Value::as_int) == Some(0),
                created: date("creationDate"),
                modified: date("modificationDate"),
            });
        }
        groups.push(RawGroup { name, snippets });
    };

    match root {
        Value::Dict(_) => {
            if root.get("snippetsTE2").is_some() {
                add(root, stem, warnings);
            } else if let Some(list) = root.get("groups").and_then(Value::as_array) {
                for (i, group) in list.iter().enumerate() {
                    add(group, &format!("{stem} {}", i + 1), warnings);
                }
            }
        }
        Value::Array(_) => {
            // A bare list of snippet dictionaries.
            let wrapped = Value::Dict(vec![("snippetsTE2".into(), root.clone())]);
            add(&wrapped, stem, warnings);
        }
        _ => {}
    }
    if groups.is_empty() {
        return Err(not_textexpander());
    }
    Ok(groups)
}

// MARK: - CSV

fn read_csv(
    rows: &[Vec<String>],
    stem: &str,
    file: &str,
    warnings: &mut Vec<ImportWarning>,
) -> Result<Vec<RawGroup>, InterchangeError> {
    let Some(first) = rows.first() else {
        return Err(not_textexpander());
    };
    let find = |names: &[&str]| {
        first
            .iter()
            .position(|c| names.contains(&c.trim().to_lowercase().as_str()))
    };
    let abbreviation_col = find(&["abbreviation", "abbrev", "trigger", "shortcut"]);
    let content_col = find(&[
        "snippet",
        "content",
        "expansion",
        "text",
        "plain text",
        "plaintext",
        "replacement",
    ]);
    let (abbreviation_col, content_col, label_col, group_col, body) =
        match (abbreviation_col, content_col) {
            (Some(a), Some(c)) => (
                a,
                c,
                find(&["label", "name", "title", "description"]),
                find(&["group", "group name", "folder"]),
                &rows[1..],
            ),
            // No header row: abbreviation, content, optional label.
            _ => (
                0,
                1,
                rows.iter().any(|r| r.len() > 2).then_some(2),
                None,
                rows,
            ),
        };

    let mut groups: Vec<RawGroup> = Vec::new();
    for (index, row) in body.iter().enumerate() {
        let cell = |col: usize| row.get(col).map(String::as_str).unwrap_or("");
        let abbreviation = cell(abbreviation_col);
        let content = cell(content_col);
        if content.is_empty() {
            warnings.push(warning(
                file,
                index,
                Some(abbreviation),
                "Row has no content and was skipped.".into(),
            ));
            continue;
        }
        let group_name = group_col
            .map(cell)
            .map(str::trim)
            .filter(|g| !g.is_empty())
            .unwrap_or(stem);
        let position = match groups.iter().position(|g| g.name == group_name) {
            Some(p) => p,
            None => {
                groups.push(RawGroup {
                    name: group_name.to_string(),
                    snippets: vec![],
                });
                groups.len() - 1
            }
        };
        groups[position].snippets.push(RawSnippet {
            index,
            abbreviation: abbreviation.trim().to_string(),
            content: normalize_newlines(content),
            label: label_col.map(cell).map(str::to_string),
            kind: Kind::Plain,
            exact_case: false,
            created: None,
            modified: None,
        });
    }
    if groups.is_empty() && warnings.is_empty() {
        return Err(not_textexpander());
    }
    Ok(groups)
}

// MARK: - Library

fn build(groups: Vec<RawGroup>, file: &str, mut warnings: Vec<ImportWarning>) -> ImportResult {
    // Which snippets will exist, so `%snippet:abbr%` can point at them.
    let mut ids: HashMap<(usize, usize), Uuid> = HashMap::new();
    let mut by_abbreviation: HashMap<String, Uuid> = HashMap::new();
    for (g, group) in groups.iter().enumerate() {
        for (s, snippet) in group.snippets.iter().enumerate() {
            if snippet.kind == Kind::Script || snippet.content.is_empty() {
                continue;
            }
            let id = Uuid::new_v4();
            ids.insert((g, s), id);
            if !snippet.abbreviation.is_empty() {
                by_abbreviation.entry(snippet.abbreviation.clone()).or_insert(id);
            }
        }
    }

    let timestamp = Utc::now().trunc_subsecs(0);
    let mut out_groups = Vec::new();
    let mut out_snippets = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        let record = GroupRecord {
            sort_index: i64::try_from(out_groups.len()).unwrap_or(0),
            ..GroupRecord::new(group.name.clone())
        };
        let mut any = false;
        for (s, snippet) in group.snippets.iter().enumerate() {
            let trigger = Some(snippet.abbreviation.as_str());
            if snippet.kind == Kind::Script {
                warnings.push(warning(
                    file,
                    snippet.index,
                    trigger,
                    "Script snippet (AppleScript, JavaScript or shell) isn't supported and was skipped."
                        .into(),
                ));
                continue;
            }
            let Some(&id) = ids.get(&(g, s)) else {
                warnings.push(warning(
                    file,
                    snippet.index,
                    trigger,
                    "Snippet has no text and was skipped.".into(),
                ));
                continue;
            };
            let mut problems = Vec::new();
            let content = convert(&snippet.content, &by_abbreviation, &mut problems);
            if snippet.kind == Kind::Rich {
                problems.push("Rich text formatting was dropped; the snippet was imported as plain text.".into());
            }
            for message in problems {
                warnings.push(warning(file, snippet.index, trigger, message));
            }
            let name = snippet
                .label
                .as_deref()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map_or_else(
                    || {
                        if snippet.abbreviation.is_empty() {
                            first_line(&snippet.content)
                        } else {
                            snippet.abbreviation.clone()
                        }
                    },
                    str::to_string,
                );
            let has_trigger = !snippet.abbreviation.is_empty();
            if !has_trigger {
                warnings.push(warning(
                    file,
                    snippet.index,
                    None,
                    format!("'{name}' has no abbreviation, so it was imported turned off. Add a trigger to use it."),
                ));
            }
            out_snippets.push(SnippetRecord {
                id,
                group_id: Some(record.id),
                fire_mode: FireMode::Immediate,
                case_mode: if snippet.exact_case {
                    CaseMode::Exact
                } else {
                    CaseMode::Propagate
                },
                is_enabled: has_trigger,
                created_at: snippet.created.map_or(timestamp, |d| d.trunc_subsecs(0)),
                updated_at: snippet
                    .modified
                    .or(snippet.created)
                    .map_or(timestamp, |d| d.trunc_subsecs(0)),
                ..SnippetRecord::new(
                    name,
                    if has_trigger {
                        vec![snippet.abbreviation.clone()]
                    } else {
                        vec![]
                    },
                    content,
                )
            });
            any = true;
        }
        if any {
            out_groups.push(record);
        }
    }
    ImportResult {
        library: KudzuLibrary {
            groups: out_groups,
            snippets: out_snippets,
            ..KudzuLibrary::default()
        },
        warnings,
    }
}

/// XML and spreadsheets both write line breaks as `\r\n`; Kudzu templates use `\n`.
fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn first_line(text: &str) -> String {
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

fn convert(raw: &str, abbreviations: &HashMap<String, Uuid>, problems: &mut Vec<String>) -> String {
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
                let section_end = (section_start..chars.len())
                    .find(|&k| starts_with(&chars, k, end_marker));
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
        if let Some(name) = ["key", "calc", "mouse"]
            .into_iter()
            .find(|k| starts_with(&chars, i + 1, k) && chars.get(i + 1 + k.len()) == Some(&':'))
        {
            if let Some(close) = next_percent(&chars, i + 2 + name.len()) {
                let source: String = chars[i..=close].iter().collect();
                problems.push(format!("%{name}:…% macros aren't supported; {source} kept as text."));
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
            problems.push("Date math (%@…) isn't supported here; the macro was kept as text.".into());
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
        if let Some(d) = default.as_deref().filter(|d| !options.iter().any(|o| o == d)) {
            options.insert(0, d.to_string());
        }
        return Some(tags::choice_tag(&name, &options, default.as_deref()));
    }
    Some(tags::field_tag(&name, default.as_deref(), kind == "fillarea"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert_plain(raw: &str) -> (String, Vec<String>) {
        let mut problems = Vec::new();
        let out = convert(raw, &HashMap::new(), &mut problems);
        (out, problems)
    }

    #[test]
    fn date_specifiers_merge_into_one_pattern() {
        let (out, problems) = convert_plain("%A, %B %1d, %Y at %H:%M");
        assert!(problems.is_empty());
        assert_eq!(
            out,
            "{{date format=\"EEEE, MMMM d, yyyy\"}} at {{date format=\"HH:mm\"}}"
        );
    }

    #[test]
    fn percent_signs_that_are_not_macros_stay_literal() {
        let (out, problems) = convert_plain("100% sure, 50%, %q, %");
        assert!(problems.is_empty());
        assert_eq!(out, "100% sure, 50%, %q, %");
    }

    #[test]
    fn fill_macros_become_form_tags() {
        let (out, problems) = convert_plain(
            "Hi %filltext:name=Customer:default=Jo%, %fillarea:name=Note:width=40%, %fillpopup:name=Tone:Formal:default=Casual%",
        );
        assert!(problems.is_empty());
        assert_eq!(
            out,
            "Hi {{field name=\"Customer\" default=\"Jo\"}}, {{field name=\"Note\" multiline=true}}, {{choice name=\"Tone\" options=\"Casual|Formal\" default=\"Casual\"}}"
        );
    }

    #[test]
    fn unsupported_macros_warn_and_stay_text() {
        let (out, problems) = convert_plain("%key:return% and %calc:1+1% and %@+1D (no date)");
        assert_eq!(problems.len(), 3);
        assert_eq!(out, "%key:return% and %calc:1+1% and %@+1D (no date)");
    }

    #[test]
    fn date_math_becomes_an_offset() {
        let (out, problems) = convert_plain("%@+1D%A, %@-2M%B %@+3m%H:%M");
        assert!(problems.is_empty());
        assert_eq!(
            out,
            "{{date format=\"EEEE\" offset=\"+1d\"}}, {{date format=\"MMMM\" offset=\"-2m\"}} {{date format=\"HH:mm\" offset=\"+3min\"}}"
        );
    }

    #[test]
    fn optional_sections_are_always_included() {
        let (out, problems) =
            convert_plain("A%fillpart:name=Opt:default=yes% extra %clipboard%fillpartend%B");
        assert_eq!(out, "A extra {{clipboard}}B");
        assert_eq!(problems.len(), 1);
        let (out, _) = convert_plain("A%fillpart:name=Opt%B");
        assert_eq!(out, "AB");
    }
}
