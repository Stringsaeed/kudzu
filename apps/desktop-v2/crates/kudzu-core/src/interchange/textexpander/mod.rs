//! TextExpander export → Kudzu library.
//!
//! Reads the two formats TextExpander writes: the XML property list behind `.textexpander`
//! group files (`snippetsTE2`), and CSV. Macros (`%clipboard`, `%|`, `%Y-%m-%d`,
//! `%filltext:…%`, `%snippet:…%`) become Kudzu tags. See the TextExpander mapping table in
//! docs/TEMPLATE_SYNTAX.md.

mod csv;
mod macros;
mod plist;

use macros::{convert, first_line, normalize_newlines};

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, SubsecRound, Utc};
use uuid::Uuid;

use super::espanso::ImportWarning;
use super::rich::{self, RichText};
use super::{ImportResult, InterchangeError};
use crate::library::model::{
    iso8601, CaseMode, FireMode, GroupRecord, KudzuLibrary, SnippetRecord,
};

use plist::Value;

/// Path-free advice shown for a folder, a binary or compressed file, or a file without snippets.
/// Public so callers can show these sentences while dropping parser-built messages.
pub const FOLDER_MESSAGE: &str = "Choose the exported .textexpander or .csv file, not a folder.";
pub const BINARY_MESSAGE: &str = "This is a binary TextExpander file Kudzu can't read. In TextExpander, choose File ▸ Export and pick CSV instead.";
pub const COMPRESSED_MESSAGE: &str = "This is a compressed TextExpander file Kudzu can't read. In TextExpander, choose File ▸ Export and pick CSV instead.";
pub const NO_SNIPPETS_MESSAGE: &str = "No TextExpander snippets were found in this file. Export a group from TextExpander as .textexpander or CSV.";

/// Reads a TextExpander export (`.textexpander` property list or CSV) from `path`.
pub fn import_file(path: &Path) -> Result<ImportResult, InterchangeError> {
    if path.is_dir() {
        return Err(InterchangeError::InvalidSource(FOLDER_MESSAGE.into()));
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
        return Err(InterchangeError::InvalidSource(BINARY_MESSAGE.into()));
    }
    if data.starts_with(b"PK\x03\x04") {
        return Err(InterchangeError::InvalidSource(COMPRESSED_MESSAGE.into()));
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
    /// A rich snippet's formatting: Cocoa RTF from `extraInfo.RTF`, or `Err` with why it can't be read.
    rich: Option<Result<RichText, String>>,
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
    InterchangeError::InvalidSource(NO_SNIPPETS_MESSAGE.into())
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
                rich: (kind == Kind::Rich).then(|| rich_text(item)),
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
            rich: None,
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
                by_abbreviation
                    .entry(snippet.abbreviation.clone())
                    .or_insert(id);
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
            let mut formatting = None;
            let content = match &snippet.rich {
                Some(Ok(text)) => {
                    let atoms = rich::textexpander_atoms(&text.text);
                    let converted = rich::to_template(text, &atoms, |segment| {
                        convert(segment, &by_abbreviation, &mut problems)
                    });
                    problems.extend(text.warning());
                    formatting = Some(converted.formatting);
                    converted.content
                }
                Some(Err(reason)) => {
                    problems.push(format!(
                        "Rich text snippet imported as plain text: {reason}."
                    ));
                    convert(&snippet.content, &by_abbreviation, &mut problems)
                }
                None => convert(&snippet.content, &by_abbreviation, &mut problems),
            };
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
                formatting,
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

/// Reads a rich snippet's formatting. TextExpander stores it twice: `extraInfo.RTF` (Cocoa RTF,
/// read here) and `richText` (an `NSArchiver` typedstream of an attributed string, which Kudzu
/// doesn't decode). RTF whose text disagrees with `plainText` isn't trusted.
fn rich_text(item: &Value) -> Result<RichText, String> {
    let rtf = item
        .get("extraInfo")
        .and_then(|info| info.get("RTF"))
        .and_then(Value::as_str);
    let Some(rtf) = rtf else {
        return Err(if item.get("richText").is_some() {
            "its formatting is stored only as an Apple archive Kudzu can't read".into()
        } else {
            "the export has no formatting for it".into()
        });
    };
    let text = rich::read_rtf(rtf)?;
    let plain = item.get("plainText").and_then(Value::as_str).unwrap_or("");
    if !text.agrees_with(plain) {
        return Err("its rich text doesn't match its plain text".into());
    }
    Ok(text)
}

#[cfg(test)]
mod tests;
