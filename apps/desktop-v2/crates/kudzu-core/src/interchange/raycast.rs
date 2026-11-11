//! Raycast snippet export (JSON) → Kudzu library.
//!
//! Raycast's "Export Snippets" writes a JSON array of `{ "name", "text", "keyword" }` objects.
//! Placeholders (`{clipboard}`, `{date …}`, `{argument …}`, …) become Kudzu tags; the ones Kudzu
//! can't run stay as literal text with a warning. See the Raycast mapping table in
//! docs/TEMPLATE_SYNTAX.md.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{SubsecRound, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::espanso::tags;
use super::espanso::ImportWarning;
use super::template_out::TemplateOut;
use super::{ImportResult, InterchangeError};
use crate::library::model::{CaseMode, FireMode, GroupRecord, KudzuLibrary, SnippetRecord};

/// Name of the group that holds the imported snippets.
pub const GROUP_NAME: &str = "Raycast";

struct Entry {
    index: usize,
    name: String,
    text: String,
    keyword: Option<String>,
}

/// Reads a Raycast snippet export from `path`.
pub fn import_file(path: &Path) -> Result<ImportResult, InterchangeError> {
    let json = fs::read_to_string(path)
        .map_err(|e| InterchangeError::io(format!("Couldn't read {}", path.display()), e))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    import_json(&json, &file_name)
}

/// Converts the text of a Raycast snippet export. `file_name` labels warnings.
pub fn import_json(json: &str, file_name: &str) -> Result<ImportResult, InterchangeError> {
    let json = json.trim_start_matches('\u{feff}');
    let value: Value = serde_json::from_str(json).map_err(|e| {
        InterchangeError::InvalidSource(format!("This isn't a Raycast snippet export: {e}"))
    })?;
    let Value::Array(items) = &value else {
        return Err(not_raycast());
    };

    let mut warnings = Vec::new();
    let mut entries = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let warn = |trigger: Option<&str>, message: String| ImportWarning {
            file: file_name.to_string(),
            line: None,
            match_index: Some(index),
            trigger: trigger.map(str::to_string),
            message,
        };
        let Some(object) = item.as_object() else {
            warnings.push(warn(
                None,
                format!("Entry {index} isn't a snippet and was skipped."),
            ));
            continue;
        };
        let string = |key: &str| object.get(key).and_then(Value::as_str);
        let keyword = string("keyword")
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string);
        let Some(text) = string("text").filter(|t| !t.is_empty()) else {
            warnings.push(warn(
                keyword.as_deref(),
                "Snippet has no text and was skipped.".into(),
            ));
            continue;
        };
        let name = string("name")
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .or_else(|| keyword.clone())
            .unwrap_or_else(|| first_line(text));
        entries.push(Entry {
            index,
            name,
            text: text.to_string(),
            keyword,
        });
    }
    if items.is_empty() {
        return Ok(ImportResult {
            library: KudzuLibrary::default(),
            warnings,
        });
    }
    if entries.is_empty() && warnings.len() == items.len() {
        return Err(not_raycast());
    }

    // `{snippet name="…"}` resolves by name, so every snippet gets its ID first.
    let ids: Vec<Uuid> = entries.iter().map(|_| Uuid::new_v4()).collect();
    let mut by_name: HashMap<String, Uuid> = HashMap::new();
    for (entry, id) in entries.iter().zip(&ids) {
        by_name.entry(entry.name.clone()).or_insert(*id);
    }

    let group = GroupRecord::new(GROUP_NAME);
    let timestamp = Utc::now().trunc_subsecs(0);
    let mut snippets = Vec::new();
    for (entry, id) in entries.iter().zip(ids) {
        let mut problems = Vec::new();
        let content = convert(&entry.text, &by_name, &mut problems);
        for message in problems {
            warnings.push(ImportWarning {
                file: file_name.to_string(),
                line: None,
                match_index: Some(entry.index),
                trigger: entry.keyword.clone(),
                message,
            });
        }
        if entry.keyword.is_none() {
            warnings.push(ImportWarning {
                file: file_name.to_string(),
                line: None,
                match_index: Some(entry.index),
                trigger: None,
                message: format!(
                    "'{}' has no keyword, so it was imported turned off. Add a trigger to use it.",
                    entry.name
                ),
            });
        }
        snippets.push(SnippetRecord {
            id,
            group_id: Some(group.id),
            fire_mode: FireMode::Immediate,
            case_mode: CaseMode::Exact,
            is_enabled: entry.keyword.is_some(),
            created_at: timestamp,
            updated_at: timestamp,
            ..SnippetRecord::new(
                entry.name.clone(),
                entry.keyword.clone().into_iter().collect(),
                content,
            )
        });
    }

    Ok(ImportResult {
        library: KudzuLibrary {
            groups: if snippets.is_empty() {
                vec![]
            } else {
                vec![group]
            },
            snippets,
            ..KudzuLibrary::default()
        },
        warnings,
    })
}

/// Path-free advice for a JSON file that isn't a Raycast export.
pub const NOT_RAYCAST_MESSAGE: &str = "This isn't a Raycast snippet export. In Raycast, run “Export Snippets” and choose the JSON file it saves.";

fn not_raycast() -> InterchangeError {
    InterchangeError::InvalidSource(NOT_RAYCAST_MESSAGE.into())
}

fn first_line(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    let short: String = line.chars().take(40).collect();
    if short.is_empty() {
        "Untitled".into()
    } else {
        short
    }
}

// MARK: - Placeholder conversion

struct Placeholder {
    name: String,
    attrs: Vec<(String, String)>,
    modifiers: Vec<String>,
}

impl Placeholder {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

const NAMES: [&str; 10] = [
    "clipboard",
    "cursor",
    "date",
    "time",
    "datetime",
    "day",
    "selection",
    "uuid",
    "snippet",
    "argument",
];

fn convert(raw: &str, snippets: &HashMap<String, Uuid>, problems: &mut Vec<String>) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = TemplateOut::new();
    let mut unnamed_arguments = 0;
    let mut i = 0;
    while i < chars.len() {
        // `\{` escapes a placeholder; before anything else it is a backslash and a brace.
        if chars[i] == '\\' && chars.get(i + 1) == Some(&'{') {
            if placeholder_span(&chars, i + 1).is_some() {
                out.push_char('{');
            } else {
                out.text("\\{");
            }
            i += 2;
            continue;
        }
        let Some((close, name)) = placeholder_span(&chars, i) else {
            out.push_char(chars[i]);
            i += 1;
            continue;
        };
        let inner: String = chars[i + 1..close].iter().collect();
        let source: String = chars[i..=close].iter().collect();
        match parse_placeholder(&name, &inner[name.len()..]) {
            Some(p) => match placeholder_tag(&p, snippets, &mut unnamed_arguments, problems) {
                Some(tag) => out.tag(&tag),
                None => out.text(&source),
            },
            None => {
                problems.push(format!(
                    "Couldn't read the placeholder {source}; kept as text."
                ));
                out.text(&source);
            }
        }
        i = close + 1;
    }
    out.finish()
}

/// If a Raycast placeholder opens at `open`, its closing brace and name.
fn placeholder_span(chars: &[char], open: usize) -> Option<(usize, String)> {
    if chars.get(open) != Some(&'{') {
        return None;
    }
    let close = find_close(chars, open + 1)?;
    let inner: String = chars[open + 1..close].iter().collect();
    let name: String = inner.chars().take_while(char::is_ascii_lowercase).collect();
    let known = NAMES.contains(&name.as_str())
        && inner[name.len()..]
            .chars()
            .next()
            .is_none_or(|c| c.is_whitespace() || c == '|');
    // Anything else (code, JSON, …) is kept as written.
    known.then_some((close, name))
}

/// The index of the `}` that closes a placeholder opened just before `from`, skipping quoted text.
fn find_close(chars: &[char], from: usize) -> Option<usize> {
    let mut quoted = false;
    let mut j = from;
    while j < chars.len() {
        match chars[j] {
            '\\' if quoted => j += 1,
            '"' => quoted = !quoted,
            '}' if !quoted => return Some(j),
            '\n' if !quoted => return None,
            _ => {}
        }
        j += 1;
    }
    None
}

/// Parses ` key="value" key=bare | modifier | modifier`.
fn parse_placeholder(name: &str, rest: &str) -> Option<Placeholder> {
    let chars: Vec<char> = rest.chars().collect();
    let mut attrs = Vec::new();
    let mut i = 0;
    let mut modifiers = Vec::new();
    loop {
        while chars.get(i).is_some_and(|c| c.is_whitespace()) {
            i += 1;
        }
        match chars.get(i) {
            None => break,
            Some('|') => {
                let tail: String = chars[i + 1..].iter().collect();
                modifiers = tail
                    .split('|')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect();
                break;
            }
            Some(_) => {}
        }
        let start = i;
        while chars
            .get(i)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        {
            i += 1;
        }
        if i == start || chars.get(i) != Some(&'=') {
            return None;
        }
        let key: String = chars[start..i].iter().collect();
        i += 1;
        let mut value = String::new();
        if chars.get(i) == Some(&'"') {
            i += 1;
            loop {
                match chars.get(i)? {
                    '\\' if matches!(chars.get(i + 1), Some('"' | '\\')) => {
                        value.push(chars[i + 1]);
                        i += 2;
                    }
                    '"' => {
                        i += 1;
                        break;
                    }
                    c => {
                        value.push(*c);
                        i += 1;
                    }
                }
            }
        } else {
            while chars
                .get(i)
                .is_some_and(|c| !c.is_whitespace() && *c != '|')
            {
                value.push(chars[i]);
                i += 1;
            }
        }
        attrs.push((key, value));
    }
    Some(Placeholder {
        name: name.to_string(),
        attrs,
        modifiers,
    })
}

/// `None` keeps the placeholder as literal text (after recording why).
fn placeholder_tag(
    p: &Placeholder,
    snippets: &HashMap<String, Uuid>,
    unnamed_arguments: &mut usize,
    problems: &mut Vec<String>,
) -> Option<String> {
    let label = format!("{{{}}}", p.name);
    let modifiers_used: bool;
    let tag = match p.name.as_str() {
        "cursor" => {
            modifiers_used = !p.modifiers.is_empty();
            tags::cursor_tag()
        }
        "clipboard" => {
            let trim = p.modifiers.iter().any(|m| m == "trim");
            modifiers_used = p.modifiers.iter().any(|m| m != "trim");
            match p.attr("offset").map(str::trim).filter(|o| *o != "0") {
                None => tags::clipboard_tag(trim),
                // Raycast's offset counts copies back from the current clipboard, as `history` does.
                Some(offset) => match offset
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=crate::template::MAX_CLIPBOARD_HISTORY).contains(n))
                {
                    Some(history) => tags::clipboard_history_tag(history, trim),
                    None => {
                        problems.push(format!("{label} has offset={offset}, which isn't a clipboard history position Kudzu can read; kept as text."));
                        return None;
                    }
                },
            }
        }
        "selection" => {
            modifiers_used = !p.modifiers.is_empty();
            tags::selection_tag()
        }
        "uuid" => {
            modifiers_used = !p.modifiers.is_empty();
            // Raycast writes UUIDs in capitals.
            tags::uuid_tag(true)
        }
        "date" | "time" | "datetime" | "day" => {
            modifiers_used = !p.modifiers.is_empty();
            let default = match p.name.as_str() {
                "date" => "d MMM yyyy",
                "time" => "HH:mm",
                "datetime" => "d MMM yyyy 'at' HH:mm",
                _ => "EEEE",
            };
            let format = p
                .attr("format")
                .filter(|f| !f.is_empty())
                .unwrap_or(default);
            let offset = match p.attr("offset").filter(|o| !o.trim().is_empty()) {
                None => None,
                Some(raw) => match convert_offset(raw) {
                    Some(offset) => Some(offset),
                    None => {
                        problems.push(format!(
                            "Date offset \"{raw}\" in {label} isn't supported and was ignored."
                        ));
                        None
                    }
                },
            };
            let locale = p.attr("locale").map(|l| l.trim().replace('-', "_"));
            tags::date_tag(Some(format), offset.as_deref(), locale.as_deref())
        }
        "snippet" => {
            let target = p.attr("name").and_then(|n| snippets.get(n));
            match target {
                Some(&id) => {
                    modifiers_used = !p.modifiers.is_empty();
                    tags::snippet_tag(id)
                }
                None => {
                    problems.push(match p.attr("name") {
                        Some(n) => format!(
                            "{label} refers to \"{n}\", which isn't in this export; kept as text."
                        ),
                        None => format!("{label} has no name; kept as text."),
                    });
                    return None;
                }
            }
        }
        "argument" => {
            modifiers_used = !p.modifiers.is_empty();
            // Raycast shares a value only between arguments of the same name.
            let generated;
            let name = match p.attr("name").filter(|n| !n.is_empty()) {
                Some(name) => name,
                None => {
                    *unnamed_arguments += 1;
                    generated = format!("argument {unnamed_arguments}");
                    &generated
                }
            };
            let default = p.attr("default");
            let options: Vec<String> = p
                .attr("options")
                .map(|o| {
                    o.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            if options.is_empty() {
                tags::field_tag(name, default, false)
            } else {
                tags::choice_tag(name, &options, default)
            }
        }
        other => {
            problems.push(format!(
                "{{{other}}} has no Kudzu equivalent; kept as text."
            ));
            return None;
        }
    };
    if modifiers_used {
        problems.push(format!(
            "Modifiers on {label} (| {}) aren't supported and were ignored.",
            p.modifiers.join(" | ")
        ));
    }
    Some(tag)
}

/// Raycast offsets use `m` for minutes and `M` for months; Kudzu uses `min` and `m`.
///
/// Parts may be separated by spaces (`+2y +5M`). Kudzu applies one leading sign to every
/// component, so parts with different signs can't be expressed.
fn convert_offset(raw: &str) -> Option<String> {
    let mut sign = None;
    let mut out = String::new();
    for part in raw.split_whitespace() {
        let (part_sign, body) = match part.chars().next()? {
            '+' => ('+', &part[1..]),
            '-' => ('-', &part[1..]),
            _ => ('+', part),
        };
        if *sign.get_or_insert(part_sign) != part_sign {
            return None;
        }
        let chars: Vec<char> = body.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let start = i;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            let digits: String = chars[start..i].iter().collect();
            if digits.is_empty() || i >= chars.len() {
                return None;
            }
            let unit = match chars[i] {
                'm' => "min",
                'M' => "m",
                'h' | 'H' => "h",
                'd' | 'D' => "d",
                'w' | 'W' => "w",
                'y' | 'Y' => "y",
                _ => return None,
            };
            i += 1;
            out.push_str(&digits);
            out.push_str(unit);
        }
    }
    (!out.is_empty()).then(|| format!("{}{out}", sign.unwrap_or('+')))
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
    fn offsets_swap_minute_and_month_units() {
        assert_eq!(convert_offset("+1d").as_deref(), Some("+1d"));
        assert_eq!(convert_offset("-2M").as_deref(), Some("-2m"));
        assert_eq!(convert_offset("+30m").as_deref(), Some("+30min"));
        assert_eq!(convert_offset("1y2d").as_deref(), Some("+1y2d"));
        assert_eq!(convert_offset("+2y +5M").as_deref(), Some("+2y5m"));
        assert_eq!(convert_offset("-1d -2h").as_deref(), Some("-1d2h"));
        assert_eq!(convert_offset("+1d -2h"), None);
        assert_eq!(convert_offset("+x"), None);
        assert_eq!(convert_offset("+5"), None);
        assert_eq!(convert_offset(""), None);
    }

    #[test]
    fn plain_braces_are_not_placeholders() {
        let (out, problems) = convert_plain("fn main() { println!(\"{x}\") } {clipboardy}");
        assert_eq!(out, "fn main() { println!(\"{x}\") } {clipboardy}");
        assert!(problems.is_empty());
    }

    #[test]
    fn backslash_escape_applies_only_before_placeholders() {
        let (out, _) = convert_plain("\\{clipboard}");
        assert_eq!(out, "{clipboard}");
        // LaTeX and Windows paths keep their backslashes.
        let (out, problems) = convert_plain("\\frac\\{a\\}{b} C:\\path\\{x}");
        assert!(problems.is_empty());
        assert_eq!(out, "\\\\frac\\\\{a\\\\}{b} C:\\\\path\\\\{x}");
    }

    #[test]
    fn unnamed_arguments_get_separate_fields() {
        let (out, _) = convert_plain("{argument} and {argument} and {argument name=\"Who\"}");
        assert_eq!(
            out,
            "{{field name=\"argument 1\"}} and {{field name=\"argument 2\"}} and {{field name=\"Who\"}}"
        );
    }

    #[test]
    fn quoted_values_may_contain_braces_and_pipes() {
        let (out, problems) = convert_plain("{argument name=\"a}b\" default=\"x|y\"}");
        assert!(problems.is_empty());
        assert_eq!(out, "{{field name=\"a}b\" default=\"x|y\"}}");
    }
}
